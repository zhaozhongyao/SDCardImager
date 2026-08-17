use std::path::Path;
use std::process::Command;

/// 守护进程 pid 文件路径（macOS 固定 /tmp，与其他平台共享一致约定）
fn pid_file() -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        std::path::PathBuf::from("/tmp/flash-helper.pid")
    } else {
        std::env::temp_dir().join("flash-helper.pid")
    }
}

/// 检测守护进程是否存活。
/// macOS：ps 检测（kill -0 对 root 进程会 EPERM）
/// Windows：OpenProcess 检测进程存在性
pub fn daemon_alive() -> bool {
    if let Ok(content) = std::fs::read_to_string(pid_file()) {
        if let Ok(pid) = content.trim().parse::<u32>() {
            #[cfg(target_os = "macos")]
            {
                if let Ok(out) = Command::new("ps")
                    .args(["-p", &pid.to_string(), "-o", "pid="])
                    .output()
                {
                    return !String::from_utf8_lossy(&out.stdout).trim().is_empty();
                }
            }
            #[cfg(target_os = "windows")]
            {
                use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
                use windows_sys::Win32::System::Threading::{
                    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
                };
                unsafe {
                    let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                    if h != INVALID_HANDLE_VALUE {
                        let _ = CloseHandle(h);
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// 确保守护进程在运行。
/// macOS：引导用户从已获 FDA 的终端手动启动（写入块设备需要完全磁盘访问权限）
/// Windows：通过 ShellExecuteW(runas) 弹 UAC 提权启动（管理员权限即可写设备）
pub fn ensure_helper_daemon() -> Result<(), String> {
    if daemon_alive() {
        return Ok(());
    }
    let helper = helper_path()?;

    #[cfg(target_os = "macos")]
    {
        return Err(format!(
            "权限守护进程未运行。\n\n请在终端中执行以下命令启动它（需要输入一次密码）：\n\nsudo nohup {helper} serve >/dev/null 2>&1 &\n\n启动完成后返回本窗口重新操作。"
        ));
    }

    #[cfg(target_os = "windows")]
    {
        spawn_windows_daemon(&helper)?;
        for _ in 0..40 {
            if daemon_alive() {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        return Err("等待管理员授权超时，请检查 UAC 对话框".to_string());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = &helper;
        Err("此平台暂不支持权限守护进程".to_string())
    }
}

/// Windows：通过 ShellExecuteW 以 runas（UAC 提权）启动 flash-helper serve
#[cfg(target_os = "windows")]
fn spawn_windows_daemon(helper: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::{ShellExecuteW, SW_HIDE};
    use windows_sys::Win32::UI::WindowsAndMessaging::SE_ERR_ACCESSDENIED;

    let _ = std::fs::remove_file(pid_file());
    let exe: Vec<u16> = helper.encode_utf16().chain(std::iter::once(0)).collect();
    let args: Vec<u16> = "serve".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let r = ShellExecuteW(
            std::ptr::null_mut(),
            wide_str("runas").as_ptr(),
            exe.as_ptr(),
            args.as_ptr(),
            std::ptr::null(),
            SW_HIDE as i32,
        );
        if r as isize <= 32 {
            let _ = SE_ERR_ACCESSDENIED;
            return Err("无法启动提权进程（UAC 被拒绝或 ShellExecute 失败）".to_string());
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn wide_str(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn helper_path() -> Result<String, String> {
    if let Ok(exe) = std::env::current_exe() {
        let p = exe
            .parent()
            .unwrap_or_else(|| Path::new("/"))
            .join("flash-helper");
        if p.exists() {
            return Ok(p.to_string_lossy().into_owned());
        }
    }
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(if cfg!(debug_assertions) { "debug" } else { "release" })
        .join("flash-helper");
    if p.exists() {
        return Ok(p.to_string_lossy().into_owned());
    }
    Err("未找到 flash-helper 可执行文件，请先执行 cargo build 构建 helper".to_string())
}
