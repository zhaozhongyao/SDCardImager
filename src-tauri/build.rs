fn main() {
    // 注入编译时间（UNIX 秒），用于"关于"弹窗显示
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    println!("cargo:rustc-env=BUILD_UNIX={}", secs);
    tauri_build::build()
}
