use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Partition {
    pub name: String,
    pub start: u64,
    pub size: u64,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Device {
    pub device_path: String,
    pub display_name: String,
    pub vendor: Option<String>,
    pub usb_product_name: Option<String>,
    pub size_bytes: u64,
    pub removable: bool,
    pub is_system: bool,
    pub partitions: Vec<Partition>,
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub fn list_devices() -> Result<Vec<Device>, String> {
    macos::list_devices()
}

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub fn list_devices() -> Result<Vec<Device>, String> {
    windows::list_devices()
}

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub fn list_devices() -> Result<Vec<Device>, String> {
    linux::list_devices()
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn list_devices() -> Result<Vec<Device>, String> {
    Err("设备发现暂未在此平台实现".to_string())
}
