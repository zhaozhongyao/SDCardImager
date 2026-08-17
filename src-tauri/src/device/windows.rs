use super::{Device, Partition};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;

use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, INVALID_HANDLE_VALUE, HANDLE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, OPEN_EXISTING, FILE_SHARE_READ, FILE_SHARE_WRITE,
};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::{
    IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_PROPERTY_QUERY, STORAGE_DEVICE_DESCRIPTOR,
};

const MAX_DISKS: u32 = 32;
const SECTOR: u64 = 512;

unsafe fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 通过 DeviceIoControl 获取磁盘属性（可移动性/厂商/型号/总线类型）
struct DiskInfo {
    removable: bool,
    vendor: String,
    product: String,
    bus: u32,
}

unsafe fn query_disk_info(handle: HANDLE) -> Option<DiskInfo> {
    let mut query: STORAGE_PROPERTY_QUERY = std::mem::zeroed();
    query.PropertyId = 0; // StorageDeviceProperty
    query.QueryType = 0; // PropertyStandardQuery
    let mut buffer = [0u8; 1024];
    let mut returned: u32 = 0;
    let ok = DeviceIoControl(
        handle,
        IOCTL_STORAGE_QUERY_PROPERTY,
        &query as *const _ as *const _,
        std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
        buffer.as_mut_ptr() as *mut _,
        buffer.len() as u32,
        &mut returned,
        std::ptr::null_mut(),
    );
    if ok == 0 {
        return None;
    }
    let desc = &*(buffer.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR);
    let read_str = |off: u32, len: u32| -> String {
        if off == u32::MAX || len == 0 || off as usize >= buffer.len() {
            return String::new();
        }
        let start = off as usize;
        let end = (start + len as usize).min(buffer.len());
        let bytes = &buffer[start..end];
        let text = String::from_utf8_lossy(bytes);
        text.trim_end_matches('\0').trim().to_string()
    };
    let vendor = read_str(desc.VendorIdOffset, desc.VendorIdLength);
    let product = read_str(desc.ProductIdOffset, desc.ProductIdLength);
    Some(DiskInfo {
        removable: desc.RemovableMediaOffset != u32::MAX,
        vendor,
        product,
        bus: desc.BusType,
    })
}

/// 通过 IOCTL_DISK_GET_LENGTH_INFO 获取磁盘大小
unsafe fn disk_length(handle: HANDLE) -> Option<u64> {
    let mut length: i64 = 0;
    let mut returned: u32 = 0;
    let ok = DeviceIoControl(
        handle,
        0x0007405C, // IOCTL_DISK_GET_LENGTH_INFO
        std::ptr::null(),
        0,
        &mut length as *mut _ as *mut _,
        std::mem::size_of::<i64>() as u32,
        &mut returned,
        std::ptr::null_mut(),
    );
    if ok == 0 {
        return None;
    }
    Some(length as u64)
}

/// 解析 MBR 分区表（每分区 16 字节，最多 4 个主分区）
fn parse_mbr_partitions(buf: &[u8]) -> Vec<Partition> {
    let mut parts = Vec::new();
    if buf.len() < 512 || buf[510] != 0x55 || buf[511] != 0xAA {
        return parts;
    }
    for i in 0..4 {
        let off = 446 + i * 16;
        let entry = &buf[off..off + 16];
        let ptype = entry[4];
        if ptype == 0 {
            continue;
        }
        let start = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as u64 * SECTOR;
        let size = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as u64 * SECTOR;
        if size > 0 {
            parts.push(Partition {
                name: format!("Partition {}", i + 1),
                start,
                size,
                content: mbr_type_name(ptype).map(|s| s.to_string()),
            });
        }
    }
    parts
}

fn mbr_type_name(ptype: u8) -> Option<&'static str> {
    Some(match ptype {
        0x0B | 0x0C | 0x1C => "FAT32",
        0x0E | 0x06 => "FAT16",
        0x07 => "NTFS",
        0x83 => "Linux",
        0x82 => "Linux Swap",
        0x05 | 0x0F => "Extended",
        0xEE => "GPT Protective",
        0xEF => "EFI System",
        0xAB => "APFS",
        0xAF => "HFS+",
        _ => return None,
    })
}

/// 解析 GPT 分区表（从 LBA1 的 GPT Header + 分区项数组）
fn parse_gpt_partitions(device_path: &str, disk_size: u64) -> Vec<Partition> {
    let mut parts = Vec::new();
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(device_path)
    {
        Ok(f) => f,
        Err(_) => return parts,
    };
    // 读取 LBA1（GPT Header，512 字节）确认 GPT 签名
    let _ = file.seek(SeekFrom::Start(512));
    let mut header = [0u8; 512];
    if file.read_exact(&mut header).is_err() {
        return parts;
    }
    if &header[0..8] != b"EFI PART" {
        return parts;
    }
    let entry_count = u32::from_le_bytes([header[80], header[81], header[82], header[83]]) as u64;
    let entry_size = u32::from_le_bytes([header[84], header[85], header[86], header[87]]) as u64;
    let entries_lba = u64::from_le_bytes(header[72..80].try_into().unwrap_or([0; 8]));
    if entry_size == 0 || entry_count == 0 {
        return parts;
    }
    // 最多读 1024 个分区项
    let read_count = entry_count.min(1024);
    let read_bytes = (read_count * entry_size).min(disk_size.saturating_sub(entries_lba * SECTOR));
    let _ = file.seek(SeekFrom::Start(entries_lba * SECTOR));
    let mut buf = vec![0u8; read_bytes as usize];
    if file.read_exact(&mut buf).is_err() {
        return parts;
    }
    for i in 0..read_count {
        let off = (i * entry_size) as usize;
        if off + 128 > buf.len() {
            break;
        }
        let entry = &buf[off..off + 128];
        let ptype = &entry[0..16];
        if ptype.iter().all(|&b| b == 0) {
            continue;
        }
        let start = u64::from_le_bytes(entry[32..40].try_into().unwrap_or([0; 8])) * SECTOR;
        let size = u64::from_le_bytes(entry[40..48].try_into().unwrap_or([0; 8])) * SECTOR;
        // 分区名（UTF-16LE，72 字节 = 36 字符）
        let name_utf16: Vec<u16> = entry[56..128]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        let name = String::from_utf16_lossy(&name_utf16);
        if size > 0 {
            parts.push(Partition {
                name: if name.is_empty() {
                    format!("Partition {}", i + 1)
                } else {
                    name
                },
                start,
                size,
                content: gpt_type_name(ptype).map(|s| s.to_string()),
            });
        }
    }
    parts
}

fn gpt_type_name(ptype: &[u8]) -> Option<&'static str> {
    let guid = ptype;
    let basic_data = hex_guid("EBD0A0A2-B9E5-4433-87C0-68B6B72699C7");
    let efi = hex_guid("C12A7328-F81F-11D2-BA4B-00A0C93EC93B");
    let msr = hex_guid("E3C9E316-0B5C-4DB8-817D-F92DF00215AE");
    let linux = hex_guid("0FC63DAF-8483-4772-8E79-3D69D8477DE4");
    let _ = (basic_data, efi, msr, linux);
    Some(match guid {
        g if g == &hex_guid("EBD0A0A2-B9E5-4433-87C0-68B6B72699C7") => "Microsoft Basic Data",
        g if g == &hex_guid("C12A7328-F81F-11D2-BA4B-00A0C93EC93B") => "EFI System",
        g if g == &hex_guid("E3C9E316-0B5C-4DB8-817D-F92DF00215AE") => "Microsoft Reserved",
        g if g == &hex_guid("0FC63DAF-8483-4772-8E79-3D69D8477DE4") => "Linux",
        g if g == &hex_guid("7C3457EF-0000-11AA-AA11-00306543ECAC") => "APFS",
        g if g == &hex_guid("48465300-0000-11AA-AA11-00306543ECAC") => "HFS+",
        _ => return None,
    })
}

fn hex_guid(s: &str) -> [u8; 16] {
    // GUID 字符串（大端组序）转 16 字节（小端存储按 GPT 规范：Data1-3 小端）
    let mut out = [0u8; 16];
    let hex: Vec<u8> = s
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_digit(16).unwrap() as u8)
        .collect();
    let bytes: Vec<u8> = hex.chunks_exact(2).map(|p| p[0] << 4 | p[1]).collect();
    if bytes.len() != 16 {
        return out;
    }
    // GPT 中 GUID 的 Data1(4)/Data2(2)/Data3(2) 为小端
    out[0] = bytes[3];
    out[1] = bytes[2];
    out[2] = bytes[1];
    out[3] = bytes[0];
    out[4] = bytes[5];
    out[5] = bytes[4];
    out[6] = bytes[7];
    out[7] = bytes[6];
    out[8..16].copy_from_slice(&bytes[8..16]);
    out
}

pub fn list_devices() -> Result<Vec<Device>, String> {
    let mut devices = Vec::new();
    for n in 0..MAX_DISKS {
        let path = format!("\\\\.\\PhysicalDrive{}", n);
        let wide_path = wide(&path);
        unsafe {
            let handle = CreateFileW(
                wide_path.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            );
            if handle == INVALID_HANDLE_VALUE {
                continue;
            }
            let info = query_disk_info(handle);
            let size = disk_length(handle);
            let _ = CloseHandle(handle);
            let (Some(info), Some(size)) = (info, size) else {
                continue;
            };
            // 只显示可移动设备或外部总线设备
            let external = info.bus == 7 // USB
                || info.bus == 8 // SD
                || info.bus == 12; // UFS
            if !info.removable && !external {
                continue;
            }
            let device_path = format!("\\\\.\\PhysicalDrive{}", n);
            let display_name = if !info.product.is_empty() {
                format!("{} {}", info.vendor, info.product).trim().to_string()
            } else {
                format!("PhysicalDrive{}", n)
            };
            let vendor = if info.vendor.is_empty() {
                None
            } else {
                Some(info.vendor)
            };

            // 分区表解析（读 MBR/GPT）
            let mut partitions = Vec::new();
            if let Ok(mut f) = OpenOptions::new()
                .read(true)
                .custom_flags(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .open(&device_path)
            {
                let mut mbr = [0u8; 512];
                if f.read_exact(&mut mbr).is_ok() {
                    partitions = parse_mbr_partitions(&mbr);
                    if partitions.is_empty() {
                        // 可能是 GPT（MBR 为保护性分区），尝试解析 GPT
                        partitions = parse_gpt_partitions(&device_path, size);
                    }
                }
            }

            devices.push(Device {
                device_path,
                display_name,
                vendor,
                usb_product_name: if info.product.is_empty() {
                    None
                } else {
                    Some(info.product)
                },
                size_bytes: size,
                removable: info.removable,
                is_system: false,
                partitions,
            });
        }
    }
    devices.sort_by_key(|d| d.device_path.clone());
    Ok(devices)
}
