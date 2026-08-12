pub mod exporter;
pub mod flasher;

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Debug, Serialize)]
pub struct ProgressPayload {
    pub task_id: u64,
    pub stage: String,
    pub percent: f64,
    pub speed_mbps: f64,
    pub eta_seconds: f64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRange {
    pub start: u64,
    pub length: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashStartRequest {
    pub mode: String,
    pub image_path: Option<String>,
    pub device_paths: Vec<String>,
    pub export_range: Option<ExportRange>,
    pub concurrency: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct QueuedTask {
    pub id: u64,
    pub mode: String,
    pub image_path: Option<String>,
    pub device_path: String,
    pub export_range: Option<ExportRange>,
}

#[derive(Default)]
pub struct TaskQueue {
    inner: Arc<Mutex<QueueInner>>,
}


#[derive(Default)]
struct QueueInner {
    queue: VecDeque<QueuedTask>,
    next_id: u64,
    workers_started: bool,
    worker_count: usize,
}

impl TaskQueue {
    pub fn enqueue(&self, handle: AppHandle, req: FlashStartRequest) -> Result<Vec<u64>, String> {
        if req.device_paths.is_empty() {
            return Err("未选择任何目标设备".to_string());
        }
        if req.mode != "flash" && req.mode != "export" {
            return Err(format!("未知模式: {}", req.mode));
        }
        if req.mode == "flash" && req.image_path.is_none() {
            return Err("未选择镜像文件".to_string());
        }

        // 并发数（1-8，默认 3）：写入 /tmp/flash-concurrency 供守护进程读取
        let concurrency = req.concurrency.unwrap_or(3).clamp(1, 8) as usize;
        let _ = std::fs::write("/tmp/flash-concurrency", concurrency.to_string());

        let mut inner = self.inner.lock().unwrap();
        let mut ids = Vec::with_capacity(req.device_paths.len());
        for path in &req.device_paths {
            inner.next_id += 1;
            let id = inner.next_id;
            inner.queue.push_back(QueuedTask {
                id,
                mode: req.mode.clone(),
                image_path: req.image_path.clone(),
                device_path: path.clone(),
                export_range: req.export_range.clone(),
            });
            ids.push(id);
        }

        // 确保有足够 worker（常驻，只增不减）
        if !inner.workers_started {
            inner.workers_started = true;
            inner.worker_count = concurrency;
            for _ in 0..concurrency {
                let queue = self.inner.clone();
                let h = handle.clone();
                std::thread::spawn(move || worker_loop(queue, h));
            }
            eprintln!("task_queue: spawned {} workers", concurrency);
        } else if inner.worker_count < concurrency {
            let extra = concurrency - inner.worker_count;
            inner.worker_count = concurrency;
            for _ in 0..extra {
                let queue = self.inner.clone();
                let h = handle.clone();
                std::thread::spawn(move || worker_loop(queue, h));
            }
            eprintln!("task_queue: spawned {} extra workers", extra);
        }
        drop(inner);
        eprintln!("task_queue: enqueued ids={:?}", ids);
        Ok(ids)
    }
}

fn worker_loop(queue: Arc<Mutex<QueueInner>>, handle: AppHandle) {
    eprintln!("worker_loop: started");
    loop {
        let task = {
            let mut inner = queue.lock().unwrap();
            inner.queue.pop_front()
        };
        match task {
            Some(t) => {
                eprintln!("worker: picked task_id={} mode={} device={}", t.id, t.mode, t.device_path);
                emit(&handle, &t, "flashing", 0.0, 0.0, 0.0, 0, 0, None);
                let result = match t.mode.as_str() {
                    "flash" => flasher::flash(&handle, &t),
                    "export" => exporter::export(&handle, &t),
                    _ => Err("未知模式".to_string()),
                };
                match result {
                    Ok(PollOutcome::Done) => {
                        emit(&handle, &t, "done", 100.0, 0.0, 0.0, 0, 0, None);
                    }
                    Ok(PollOutcome::Cancelled) => {
                        emit(&handle, &t, "cancelled", 0.0, 0.0, 0.0, 0, 0, None);
                    }
                    Err(e) => {
                        emit(&handle, &t, "error", 0.0, 0.0, 0.0, 0, 0, Some(e));
                    }
                }
            }
            None => {
                std::thread::sleep(Duration::from_millis(300));
            }
        }
    }
}

fn emit(
    handle: &AppHandle,
    task: &QueuedTask,
    stage: &str,
    percent: f64,
    speed_mbps: f64,
    eta_seconds: f64,
    bytes_done: u64,
    bytes_total: u64,
    message: Option<String>,
) {
    let _ = handle.emit(
        "flash:progress",
        ProgressPayload {
            task_id: task.id,
            stage: stage.to_string(),
            percent,
            speed_mbps,
            eta_seconds,
            bytes_done,
            bytes_total,
            message,
        },
    );
}

/// 轮询 helper 守护进程写出的日志文件，将进度转发到前端。
/// 若守护进程消失且日志无明确结果，视为异常。
pub fn poll_log(
    handle: &AppHandle,
    task: &QueuedTask,
    log_path: &PathBuf,
) -> Result<PollOutcome, String> {
    let mut last_line: Option<String> = None;
    loop {
        if let Ok(content) = std::fs::read_to_string(log_path) {
            if let Some(line) = content.lines().last() {
                if last_line.as_deref() != Some(line) {
                    last_line = Some(line.to_string());
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                        match v["stage"].as_str() {
                            Some("done") => return Ok(PollOutcome::Done),
                            Some("cancelled") => return Ok(PollOutcome::Cancelled),
                            Some("error") => {
                                return Err(v["error"]
                                    .as_str()
                                    .unwrap_or("未知错误")
                                    .to_string())
                            }
                            Some(stage) => {
                                emit(
                                    handle,
                                    task,
                                    stage,
                                    v["percent"].as_f64().unwrap_or(0.0),
                                    v["speed_mbps"].as_f64().unwrap_or(0.0),
                                    v["eta_seconds"].as_f64().unwrap_or(0.0),
                                    v["bytes_done"].as_u64().unwrap_or(0),
                                    v["bytes_total"].as_u64().unwrap_or(0),
                                    None,
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if !crate::permission::daemon_alive() {
            if let Ok(content) = std::fs::read_to_string(log_path) {
                if let Some(line) = content.lines().last() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                        match v["stage"].as_str() {
                            Some("done") => return Ok(PollOutcome::Done),
                            Some("cancelled") => return Ok(PollOutcome::Cancelled),
                            Some("error") => {
                                return Err(v["error"]
                                    .as_str()
                                    .unwrap_or("未知错误")
                                    .to_string())
                            }
                            _ => {}
                        }
                    }
                }
            }
            return Err("权限助手进程已退出，任务异常终止".to_string());
        }

        std::thread::sleep(Duration::from_millis(150));
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PollOutcome {
    Done,
    Cancelled,
}
