use super::{Device, Partition};
use std::path::{Path, PathBuf};

fn read_str(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_u64(path: &Path) -> Option<u64> {
    read_str(path).and_then(|s| s.parse().ok())
}

/// 通过 lsblk 获取分区文件系统类型（失败返回 None）
fn fs_type(partition_path: &str) -> Option<String> {
    let out = std::process::Command::new("lsblk")
        .args(["-no", "FSTYPE"])
        .arg(partition_path)
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout);
        let t = s.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    None
}

pub fn list_devices() -> Result<Vec<Device>, String> {
    let mut devices = Vec::new();
    let sysblock = Path::new("/sys/block");

    for entry in std::fs::read_dir(sysblock).map_err(|e| format!("无法读取 /sys/block: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // sdX=SATA/USB, mmcblkX=SD/eMMC, nvmeX=NVMe
        if !name.starts_with("sd") && !name.starts_with("mmcblk") {
            continue;
        }
        let base = sysblock.join(&name);

        // 只显示可移动设备（removable=1），排除内置盘
        let removable = read_str(&base.join("removable")).map(|s| s == "1").unwrap_or(false);
        if !removable {
            continue;
        }

        let size_sectors = read_u64(&base.join("size")).unwrap_or(0);
        let size_bytes = size_sectors.saturating_mul(512);
        if size_bytes == 0 {
            continue; // 空读卡器（无媒体）
        }

        let vendor = read_str(&base.join("device/vendor"));
        let model = read_str(&base.join("device/model"));

        // 分区：/sys/block/sdX/sdX1 或 /sys/block/mmcblk0/mmcblk0p1
        let mut partitions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&base) {
            for pe in entries.flatten() {
                let pname = pe.file_name().to_string_lossy().into_owned();
                if pname.len() <= name.len() || !pname.starts_with(&name) {
                    continue;
                }
                let pbase = base.join(&pname);
                let start = read_u64(&pbase.join("start"));
                let psize_sectors = read_u64(&pbase.join("size"));
                let (Some(start), Some(psize_sectors)) = (start, psize_sectors) else {
                    continue;
                };
                let psize = psize_sectors.saturating_mul(512);
                if psize == 0 {
                    continue;
                }
                let dev_path = format!("/dev/{}", pname);
                partitions.push(Partition {
                    name: format!("/dev/{}", pname),
                    start: start.saturating_mul(512),
                    size: psize,
                    content: fs_type(&dev_path),
                });
            }
        }
        partitions.sort_by_key(|p| p.start);

        let device_path = format!("/dev/{}", name);
        let display_name = model
            .clone()
            .filter(|m| !m.is_empty() && m != "Unknown")
            .unwrap_or_else(|| device_path.clone());
        devices.push(Device {
            device_path,
            display_name,
            vendor,
            usb_product_name: model,
            size_bytes,
            removable: true,
            is_system: false,
            partitions,
        });
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
}
