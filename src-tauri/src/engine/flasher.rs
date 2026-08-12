use super::{poll_log, PollOutcome, QueuedTask};
use crate::permission;
use std::path::PathBuf;
use tauri::AppHandle;

/// 烧录流程：
/// 1. 确保 root 守护进程运行（已具备完全磁盘访问权限，可直接读取任意位置的镜像）
/// 2. 写任务文件，daemon 直接读取镜像并写入块设备 + 回读校验
/// 3. 轮询进度日志并转发到前端
pub fn flash(handle: &AppHandle, task: &QueuedTask) -> Result<PollOutcome, String> {
    let image_path = task
        .image_path
        .clone()
        .ok_or_else(|| "缺少镜像路径".to_string())?;
    eprintln!("flash: start task_id={} image={}", task.id, image_path);

    permission::ensure_helper_daemon()?;

    let log_path: PathBuf = PathBuf::from("/tmp").join(format!("flash-helper-{}.jsonl", task.id));
    let task_path: PathBuf = PathBuf::from("/tmp").join(format!("flash-task-{}.json", task.id));
    let _ = std::fs::remove_file(&log_path);
    let _ = std::fs::write(&log_path, "");

    let task_json = serde_json::json!({
        "mode": "flash",
        "id": task.id.to_string(),
        "image": image_path,
        "device": task.device_path,
        "log": log_path.to_string_lossy(),
    });
    eprintln!("flash: submitting task file {}", task_path.display());
    std::fs::write(&task_path, task_json.to_string())
        .map_err(|e| format!("无法提交任务: {e}"))?;
    eprintln!("flash: task submitted, polling");

    let result = poll_log(handle, task, &log_path);
    eprintln!("flash: poll result: {:?}", result);

    let _ = std::fs::remove_file(&task_path);
    let _ = std::fs::remove_file(format!("/tmp/flash-cancel-{}", task.id));
    result
}
