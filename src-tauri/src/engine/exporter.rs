use super::{poll_log, PollOutcome, QueuedTask};
use crate::permission;
use std::path::PathBuf;
use tauri::AppHandle;

/// 导出流程：
/// 1. 确保 root 守护进程运行
/// 2. 写任务文件，daemon 直接读取块设备写入用户指定的目标文件
/// 3. 轮询进度日志并转发到前端
pub fn export(handle: &AppHandle, task: &QueuedTask) -> Result<PollOutcome, String> {
    let target_path = task
        .image_path
        .clone()
        .ok_or_else(|| "缺少导出目标路径".to_string())?;

    permission::ensure_helper_daemon()?;

    let log_path: PathBuf = PathBuf::from("/tmp").join(format!("flash-helper-{}.jsonl", task.id));
    let task_path: PathBuf = PathBuf::from("/tmp").join(format!("flash-task-{}.json", task.id));
    let _ = std::fs::remove_file(&log_path);
    let _ = std::fs::write(&log_path, "");

    let mut task_json = serde_json::json!({
        "mode": "export",
        "id": task.id.to_string(),
        "image": target_path,
        "device": task.device_path,
        "log": log_path.to_string_lossy(),
    });
    if let Some(range) = &task.export_range {
        task_json["export_start"] = serde_json::json!(range.start);
        task_json["export_length"] = serde_json::json!(range.length);
    }
    std::fs::write(&task_path, task_json.to_string())
        .map_err(|e| format!("无法提交任务: {e}"))?;

    let result = poll_log(handle, task, &log_path);

    let _ = std::fs::remove_file(&task_path);
    let _ = std::fs::remove_file(format!("/tmp/flash-cancel-{}", task.id));
    result
}
