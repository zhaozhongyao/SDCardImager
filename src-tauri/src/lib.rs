mod device;
mod engine;
mod permission;

use device::Device;
use engine::{FlashStartRequest, TaskQueue};
use tauri::State;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(TaskQueue::default())
        .invoke_handler(tauri::generate_handler![
            list_devices,
            file_size,
            start_task,
            ensure_privileges,
            cancel_tasks,
            app_info
        ])
        .setup(|app| {
            // 清理上次运行遗留的取消信号文件
            if let Ok(entries) = std::fs::read_dir(engine::work_dir()) {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if name.starts_with("flash-cancel-") {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn list_devices() -> Result<Vec<Device>, String> {
    device::list_devices().map_err(|e| e.to_string())
}

#[tauri::command]
fn file_size(path: String) -> Result<u64, String> {
    std::fs::metadata(&path)
        .map(|m| m.len())
        .map_err(|e| format!("无法读取文件信息: {e}"))
}

#[tauri::command]
fn start_task(
    state: State<'_, TaskQueue>,
    handle: tauri::AppHandle,
    request: FlashStartRequest,
) -> Result<Vec<u64>, String> {
    eprintln!("start_task called: mode={} devices={:?} range={:?}", request.mode, request.device_paths, request.export_range);
    state.enqueue(handle, request)
}

/// 供前端主动预检权限：返回是否需要弹授权框（false 表示守护进程已在运行）
#[tauri::command]
fn ensure_privileges() -> Result<bool, String> {
    if permission::daemon_alive() {
        return Ok(false);
    }
    permission::ensure_helper_daemon()?;
    Ok(false)
}

/// 取消指定任务：写取消信号文件，守护进程会在下一个数据块处中断
#[tauri::command]
fn cancel_tasks(task_ids: Vec<u64>) -> Result<(), String> {
    for id in task_ids {
        let _ = std::fs::write(engine::work_dir().join(format!("flash-cancel-{id}")), "1");
    }
    Ok(())
}

/// 应用版本信息（用于"关于"弹窗）
#[tauri::command]
fn app_info() -> serde_json::Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "build_unix": option_env!("BUILD_UNIX").and_then(|s| s.parse::<u64>().ok()),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    })
}
