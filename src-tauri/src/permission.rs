use std::path::Path;
use std::process::Command;

const DAEMON_PID_FILE: &str = "/tmp/flash-helper.pid";

/// 架构说明（macOS 15）：
/// 写入块设备需要"完全磁盘访问权限"(FDA)。osascript 提权与 launchd daemon
/// 启动的 root 进程都没有 FDA，无法访问 /dev/rdisk*。
/// 可靠方案：守护进程从已获 FDA 的终端会话启动（sudo nohup ... serve），
/// 继承终端的 FDA。GUI 检测守护进程不在时，引导用户手动运行启动命令。

/// 检测守护进程是否存活。
/// 注意：不能用 kill(pid, 0)，macOS 上普通用户对 root 进程调用会返回 EPERM；
/// 用 ps 检测进程存在性（普通用户可执行）。
pub fn daemon_alive() -> bool {
    if let Ok(content) = std::fs::read_to_string(DAEMON_PID_FILE) {
        if let Ok(pid) = content.trim().parse::<i32>() {
            if let Ok(out) = Command::new("ps")
                .args(["-p", &pid.to_string(), "-o", "pid="])
                .output()
            {
                return !String::from_utf8_lossy(&out.stdout).trim().is_empty();
            }
        }
    }
    false
}

/// 确保守护进程在运行；不在时返回带启动指引的错误信息
pub fn ensure_helper_daemon() -> Result<(), String> {
    if daemon_alive() {
        return Ok(());
    }
    let helper = helper_path()?;
    Err(format!(
        "权限守护进程未运行。\n\n请在终端中执行以下命令启动它（需要输入一次密码）：\n\nsudo nohup {helper} serve >/dev/null 2>&1 &\n\n启动完成后返回本窗口重新操作。"
    ))
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
