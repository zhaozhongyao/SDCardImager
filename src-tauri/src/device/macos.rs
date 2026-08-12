use super::Device;
use std::ffi::{c_char, c_void, CStr, CString};
use std::os::raw::c_uint;

type CFTypeRef = *const c_void;
type CFMutableDictionaryRef = CFTypeRef;
type CFAllocatorRef = *const c_void;
#[allow(non_camel_case_types)]
type CFIndex = isize;
#[allow(non_camel_case_types)]
type mach_port_t = c_uint;
#[allow(non_camel_case_types)]
type kern_return_t = i32;
#[allow(non_camel_case_types)]
type io_iterator_t = c_uint;
#[allow(non_camel_case_types)]
type io_object_t = c_uint;
#[allow(non_camel_case_types)]
type io_registry_entry_t = c_uint;

const KCF_STRING_ENCODING_UTF8: CFIndex = 0x08000100;
const KCF_NUMBER_SINT64_TYPE: u32 = 4;
const KCF_NUMBER_SINT32_TYPE: u32 = 3;
const K_IO_REGISTRY_ITERATE_RECURSIVELY: u32 = 1;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOServiceMatching(name: *const c_char) -> CFMutableDictionaryRef;
    fn IOServiceGetMatchingServices(
        main_port: mach_port_t,
        matching: CFMutableDictionaryRef,
        existing: *mut io_iterator_t,
    ) -> kern_return_t;
    fn IOIteratorNext(iterator: io_iterator_t) -> io_object_t;
    fn IOObjectRelease(object: io_object_t) -> kern_return_t;
    fn IORegistryEntryCreateCFProperties(
        entry: io_registry_entry_t,
        properties: *mut CFMutableDictionaryRef,
        allocator: CFAllocatorRef,
        options: u32,
    ) -> kern_return_t;
    fn IORegistryEntrySearchCFProperty(
        entry: io_registry_entry_t,
        plane: *const c_char,
        key: CFTypeRef,
        allocator: CFAllocatorRef,
        options: u32,
    ) -> CFTypeRef;
    fn IORegistryEntryGetParentEntry(
        entry: io_registry_entry_t,
        plane: *const c_char,
        parent: *mut io_registry_entry_t,
    ) -> kern_return_t;
    fn IORegistryEntryGetChildIterator(
        entry: io_registry_entry_t,
        plane: *const c_char,
        iterator: *mut io_iterator_t,
    ) -> kern_return_t;
    fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
    fn CFNumberGetValue(number: CFTypeRef, the_type: u32, value_ptr: *mut c_void) -> u8;
    fn CFStringGetCString(
        string: CFTypeRef,
        buffer: *mut c_char,
        buffer_size: CFIndex,
        encoding: CFIndex,
    ) -> u8;
    fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        c_str: *const c_char,
        encoding: CFIndex,
    ) -> CFTypeRef;
    fn CFRelease(cf: CFTypeRef);
}

unsafe fn cf_string(key: &str) -> CFTypeRef {
    let c = CString::new(key).unwrap();
    CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), KCF_STRING_ENCODING_UTF8)
}

unsafe fn get_dict_value(props: CFTypeRef, key: &str) -> CFTypeRef {
    let k = cf_string(key);
    let v = CFDictionaryGetValue(props, k);
    CFRelease(k);
    v
}

unsafe fn get_dict_bool(props: CFTypeRef, key: &str) -> bool {
    let v = get_dict_value(props, key);
    if v.is_null() {
        return false;
    }
    let mut b: u8 = 0;
    CFNumberGetValue(v, KCF_NUMBER_SINT32_TYPE, &mut b as *mut u8 as *mut c_void);
    b != 0
}

unsafe fn get_dict_u64(props: CFTypeRef, key: &str) -> u64 {
    let v = get_dict_value(props, key);
    if v.is_null() {
        return 0;
    }
    let mut n: u64 = 0;
    CFNumberGetValue(v, KCF_NUMBER_SINT64_TYPE, &mut n as *mut u64 as *mut c_void);
    n
}

unsafe fn get_dict_string(props: CFTypeRef, key: &str) -> Option<String> {
    let v = get_dict_value(props, key);
    if v.is_null() {
        return None;
    }
    let mut buf = [0i8; 256];
    CFStringGetCString(v, buf.as_mut_ptr(), 256, KCF_STRING_ENCODING_UTF8);
    Some(CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
}

/// 沿 IOService 平面向上搜索指定属性（如 "Product Name"、"Vendor Name"、"USB Product Name"）
unsafe fn search_property(entry: io_registry_entry_t, key: &str) -> Option<String> {
    let plane = CString::new("IOService").unwrap();
    let key_cf = cf_string(key);
    let v = IORegistryEntrySearchCFProperty(
        entry,
        plane.as_ptr(),
        key_cf,
        std::ptr::null(),
        K_IO_REGISTRY_ITERATE_RECURSIVELY,
    );
    CFRelease(key_cf);
    if v.is_null() {
        return None;
    }
    let mut buf = [0i8; 256];
    CFStringGetCString(v, buf.as_mut_ptr(), 256, KCF_STRING_ENCODING_UTF8);
    let s = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
    CFRelease(v);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 沿父节点链逐层提取设备信息：
/// - USB 层: "USB Product Name" / "USB Vendor Name"（顶层属性）
/// - 存储层: "Device Characteristics" 字典中的 "Product Name" / "Vendor Name"
/// 返回 (usb_product, usb_vendor, block_product, block_vendor)
unsafe fn collect_device_info(
    entry: io_registry_entry_t,
) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    let plane = CString::new("IOService").unwrap();
    let mut chain: Vec<io_registry_entry_t> = vec![entry];
    let mut cur = entry;
    for _ in 0..8 {
        let mut parent: io_registry_entry_t = 0;
        if IORegistryEntryGetParentEntry(cur, plane.as_ptr(), &mut parent) != 0 {
            break;
        }
        chain.push(parent);
        cur = parent;
    }

    let mut usb_product: Option<String> = None;
    let mut usb_vendor: Option<String> = None;
    let mut block_product: Option<String> = None;
    let mut block_vendor: Option<String> = None;

    for e in &chain {
        let mut props: CFMutableDictionaryRef = std::ptr::null_mut();
        if IORegistryEntryCreateCFProperties(*e, &mut props, std::ptr::null(), 0) != 0
            || props.is_null()
        {
            continue;
        }
        if usb_product.is_none() {
            usb_product = get_dict_string(props, "USB Product Name");
        }
        if usb_vendor.is_none() {
            usb_vendor = get_dict_string(props, "USB Vendor Name");
        }
        let dc = get_dict_value(props, "Device Characteristics");
        if !dc.is_null() {
            if block_product.is_none() {
                block_product = get_dict_string(dc, "Product Name").filter(|s| !s.is_empty());
            }
            if block_vendor.is_none() {
                block_vendor = get_dict_string(dc, "Vendor Name").filter(|s| !s.is_empty());
            }
        }
        CFRelease(props);
    }

    (usb_product, usb_vendor, block_product, block_vendor)
}

/// 递归遍历整盘设备的后代 IOMedia（分区），中间可能隔着 IOMediaBSDClient 等节点。
/// 只收集直接分区（深度 2 以内，entry → IOMediaBSDClient → 分区），
/// 避免把 APFS 容器内部子分卷（其 Base 为容器内偏移）混入。
unsafe fn collect_partitions(entry: io_registry_entry_t, depth: u32, out: &mut Vec<super::Partition>) {
    if depth > 2 {
        return;
    }
    let plane = CString::new("IOService").unwrap();
    let mut it: io_iterator_t = 0;
    if IORegistryEntryGetChildIterator(entry, plane.as_ptr(), &mut it) != 0 {
        return;
    }
    loop {
        let child = IOIteratorNext(it);
        if child == 0 {
            break;
        }
        let mut props: CFMutableDictionaryRef = std::ptr::null_mut();
        if IORegistryEntryCreateCFProperties(child, &mut props, std::ptr::null(), 0) == 0
            && !props.is_null()
        {
            let whole = get_dict_bool(props, "Whole");
            let size = get_dict_u64(props, "Size");
            if !whole && size > 0 {
                let start = get_dict_u64(props, "Base");
                let name = get_dict_string(props, "BSD Name")
                    .unwrap_or_else(|| format!("part{}", out.len()));
                let content = get_dict_string(props, "Content").filter(|s| !s.is_empty());
                out.push(super::Partition {
                    name,
                    start,
                    size,
                    content,
                });
            }
            CFRelease(props);
        }
        collect_partitions(child, depth + 1, out);
        IOObjectRelease(child);
    }
    IOObjectRelease(it);
}

/// 遍历整盘设备的子 IOMedia（分区），返回 (name, start, size)
unsafe fn get_partitions(entry: io_registry_entry_t) -> Vec<super::Partition> {
    let mut partitions = Vec::new();
    collect_partitions(entry, 0, &mut partitions);
    partitions.sort_by_key(|p| p.start);
    partitions
}

pub fn list_devices() -> Result<Vec<Device>, String> {
    let mut devices: Vec<Device> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    unsafe {
        let matching = IOServiceMatching(c"IOMedia".as_ptr());
        if matching.is_null() {
            return Ok(devices);
        }
        let mut it: io_iterator_t = 0;
        if IOServiceGetMatchingServices(0, matching, &mut it) != 0 {
            return Ok(devices);
        }
        loop {
            let entry = IOIteratorNext(it);
            if entry == 0 {
                break;
            }
            let mut props: CFMutableDictionaryRef = std::ptr::null_mut();
            if IORegistryEntryCreateCFProperties(entry, &mut props, std::ptr::null(), 0) == 0
                && !props.is_null()
            {
                let whole = get_dict_bool(props, "Whole");
                let removable = get_dict_bool(props, "Removable");
                let ejectable = get_dict_bool(props, "Ejectable");
                let size = get_dict_u64(props, "Size");
                let bsd_name = get_dict_string(props, "BSD Name").unwrap_or_default();
                let is_system = get_dict_bool(props, "Internal");

                if whole && (removable || ejectable) && !bsd_name.is_empty() {
                    let device_path = format!("/dev/{}", bsd_name);
                    if !seen.contains(&device_path) {
                        seen.push(device_path.clone());
                        let (usb_product, usb_vendor, block_product, block_vendor) =
                            collect_device_info(entry);
                        let display_name = usb_product
                            .clone()
                            .or(block_product)
                            .unwrap_or_else(|| bsd_name.clone());
                        let vendor = usb_vendor.or(block_vendor);
                        devices.push(Device {
                            device_path,
                            display_name,
                            vendor,
                            usb_product_name: usb_product,
                            size_bytes: size,
                            removable: true,
                            is_system,
                            partitions: get_partitions(entry),
                        });
                    }
                }
                CFRelease(props);
            }
            IOObjectRelease(entry);
        }
        IOObjectRelease(it);
    }

    devices.sort_by_key(|d| d.device_path.clone());
    Ok(devices)
}

#[cfg(test)]
mod tests {
    #[test]
    fn enumerate_devices() {
        let devices = super::list_devices().unwrap();
        for d in &devices {
            println!("{:?}", d);
        }
    }

    #[test]
    fn partitions_of_internal() {
        unsafe {
            let matching = super::IOServiceMatching(c"IOMedia".as_ptr());
            let mut it: super::io_iterator_t = 0;
            if super::IOServiceGetMatchingServices(0, matching, &mut it) == 0 {
                loop {
                    let entry = super::IOIteratorNext(it);
                    if entry == 0 {
                        break;
                    }
                    let mut props: super::CFMutableDictionaryRef = std::ptr::null_mut();
                    if super::IORegistryEntryCreateCFProperties(
                        entry,
                        &mut props,
                        std::ptr::null(),
                        0,
                    ) == 0
                        && !props.is_null()
                    {
                        let whole = super::get_dict_bool(props, "Whole");
                        let name =
                            super::get_dict_string(props, "BSD Name").unwrap_or_default();
                        if whole && !name.is_empty() && !name.starts_with("disk0s") {
                            let parts = super::get_partitions(entry);
                            println!("{} whole={} parts={}", name, whole, parts.len());
                            for p in &parts {
                                println!("   {}", p.name);
                            }
                        }
                        super::CFRelease(props);
                    }
                    super::IOObjectRelease(entry);
                }
                super::IOObjectRelease(it);
            }
        }
    }
}
