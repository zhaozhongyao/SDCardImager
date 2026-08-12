use serde_json::json;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::process::{Command, ExitCode};
use std::time::Instant;

const DD_BS: u64 = 4 * 1024 * 1024;
const SEG_BLOCKS: u64 = 16;
const MAX_CONCURRENT: usize = 3;
const CANCELLED: &str = "__CANCELLED__";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 2 && args[1] == "serve" {
        return serve();
    }

    if args.len() != 5 {
        eprintln!("usage: flash-helper <log_path> <mode:flash|export|verify> <arg1> <arg2>");
        eprintln!("       flash-helper serve");
        return ExitCode::from(2);
    }
    let log = &args[1];
    let mode = &args[2];
    let (arg1, arg2) = (&args[3], &args[4]);

    let result = match mode.as_str() {
        "flash" => run_flash(log, arg1, arg2, "cli"),
        "export" => run_export(log, arg1, arg2, "cli", 0, u64::MAX),
        "verify" => verify_dd(log, arg1, &raw_device(arg2), "cli"),
        other => Err(format!("未知模式: {other}")),
    };

    match result {
        Ok(()) => {
            let _ = log_line(log, &json!({"stage": "done", "percent": 100.0}));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("flash-helper error: {e}");
            let _ = log_line(log, &json!({"stage": "error", "error": e}));
            ExitCode::from(1)
        }
    }
}

/// 常驻守护进程模式：由主应用通过 osascript 提权启动。
/// 轮询 /tmp/flash-task-<id>.json 任务文件，最多 MAX_CONCURRENT 个任务并发执行。
fn serve() -> ExitCode {
    if !claim_single_instance() {
        return ExitCode::SUCCESS;
    }
    let _ = std::fs::write("/tmp/flash-helper.pid", std::process::id().to_string());
    let daemon_log = "/tmp/flash-helper-daemon.log";
    let _ = log_line(daemon_log, &json!({"event": "serve-start", "pid": std::process::id()}));

    let mut last_pid_write = std::time::Instant::now();
    let mut last_bin_check = std::time::Instant::now();
    let my_binary = std::env::current_exe().ok();
    let started_mtime = my_binary
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok());
    loop {
        if last_pid_write.elapsed().as_secs() >= 5 {
            let _ = std::fs::write("/tmp/flash-helper.pid", std::process::id().to_string());
            last_pid_write = std::time::Instant::now();
        }

        if last_bin_check.elapsed().as_secs() >= 15 {
            if let (Some(exe), Some(started)) = (&my_binary, started_mtime) {
                if let Ok(md) = std::fs::metadata(exe) {
                    if let Ok(now_m) = md.modified() {
                        if now_m != started {
                            let _ = log_line(
                                &daemon_log,
                                &json!({"event": "self-restart"}),
                            );
                            let _ = std::fs::remove_file("/tmp/flash-helper.pid");
                            let _ = Command::new(exe).arg("serve").spawn();
                            return ExitCode::SUCCESS;
                        }
                    }
                }
            }
            last_bin_check = std::time::Instant::now();
        }

        let max_concurrent = {
            let mut c = MAX_CONCURRENT;
            if let Ok(content) = std::fs::read_to_string("/tmp/flash-concurrency") {
                if let Ok(n) = content.trim().parse::<usize>() {
                    c = n.clamp(1, 8);
                }
            }
            c
        };
        let processing_count = count_processing_tasks();
        if processing_count < max_concurrent {
            let slots = max_concurrent - processing_count;
            let candidates: Vec<std::path::PathBuf> = if let Ok(entries) = std::fs::read_dir("/tmp") {
                entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        name.starts_with("flash-task-") && name.ends_with(".json")
                    })
                    .take(slots)
                    .collect()
            } else {
                Vec::new()
            };

            for task in candidates {
                let processing = task.with_extension("processing");
                if std::fs::rename(&task, &processing).is_ok() {
                    let _ = log_line(
                        &daemon_log,
                        &json!({"event": "task-picked", "file": processing.display().to_string()}),
                    );
                    std::thread::spawn(move || handle_task(processing));
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

fn count_processing_tasks() -> usize {
    if let Ok(entries) = std::fs::read_dir("/tmp") {
        return entries
            .flatten()
            .filter(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                name.starts_with("flash-task-") && name.ends_with(".processing")
            })
            .count();
    }
    0
}

fn handle_task(processing: std::path::PathBuf) {
    let daemon_log = "/tmp/flash-helper-daemon.log";
    let content = std::fs::read_to_string(&processing).unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
    let mode = v["mode"].as_str().unwrap_or("");
    let image = v["image"].as_str().unwrap_or("");
    let device = v["device"].as_str().unwrap_or("");
    let log = v["log"].as_str().unwrap_or("/tmp/flash-helper-daemon.jsonl");
    let id = v["id"].as_str().unwrap_or("0");

    let result = match mode {
        "flash" => run_flash(log, image, device, id),
        "export" => {
            let start = v["export_start"].as_u64().unwrap_or(0);
            let length = v["export_length"].as_u64().unwrap_or(u64::MAX);
            run_export(log, device, image, id, start, length)
        }
        _ => Err(format!("未知任务模式: {mode}")),
    };
    match result {
        Ok(()) => {
            let _ = log_line(log, &json!({"stage": "done", "percent": 100.0}));
        }
        Err(e) if e == CANCELLED => {
            // cancelled 行已写入日志
        }
        Err(e) => {
            let _ = log_line(log, &json!({"stage": "error", "error": e}));
        }
    }
    let _ = std::fs::remove_file(&processing);
    let _ = std::fs::remove_file(format!("/tmp/flash-cancel-{id}"));
    let _ = log_line(daemon_log, &json!({"event": "task-done"}));
}

/// GUI 通过写 /tmp/flash-cancel-<id> 文件请求取消任务
fn cancelled_flag(task_id: &str) -> bool {
    std::path::Path::new(&format!("/tmp/flash-cancel-{task_id}")).exists()
}

fn check_cancelled(task_id: &str, log: &str) -> Result<(), String> {
    if cancelled_flag(task_id) {
        let _ = log_line(log, &json!({"stage": "cancelled"}));
        return Err(CANCELLED.to_string());
    }
    Ok(())
}

/// 单实例保护：如果已有存活实例（pid 文件中的进程仍存在），当前实例退出
fn claim_single_instance() -> bool {
    if let Ok(content) = std::fs::read_to_string("/tmp/flash-helper.pid") {
        if let Ok(pid) = content.trim().parse::<i32>() {
            if pid != std::process::id() as i32 && unsafe { libc::kill(pid, 0) } == 0 {
                return false;
            }
        }
    }
    true
}

fn run_flash(log: &str, image_path: &str, device_path: &str, task_id: &str) -> Result<(), String> {
    unmount_device(device_path);
    let dev = raw_device(device_path);

    let total = std::fs::metadata(image_path)
        .map_err(|e| format!("无法读取镜像文件: {e}"))?
        .len();
    if total == 0 {
        return Err("镜像文件为空".to_string());
    }

    match write_direct(log, image_path, &dev, total, task_id) {
        Ok(()) => verify_direct(log, image_path, &dev, total, task_id),
        Err(e) if e.contains("Operation not permitted") || e.contains("Permission denied") => {
            eprintln!("flash: direct write unavailable ({}), falling back to dd", e.trim());
            dd_io(log, "flashing", image_path, &dev, total, task_id)?;
            verify_dd(log, image_path, &dev, task_id)
        }
        Err(e) => Err(e),
    }
}

fn run_export(
    log: &str,
    device_path: &str,
    image_path: &str,
    task_id: &str,
    start: u64,
    length: u64,
) -> Result<(), String> {
    let dev = raw_device(device_path);
    let total = device_size(&dev)?;
    let (start, length) = if length == u64::MAX {
        (0u64, total)
    } else {
        let start = start.min(total);
        (start, length.min(total - start))
    };
    match export_direct(log, &dev, image_path, total, task_id, start, length) {
        Ok(()) => Ok(()),
        Err(e) if e.contains("Operation not permitted") || e.contains("Permission denied") => {
            eprintln!("export: direct read unavailable ({}), falling back to dd", e.trim());
            let _ = std::fs::remove_file(image_path);
            dd_io(log, "exporting", &dev, image_path, total, task_id)
        }
        Err(e) => Err(e),
    }
}

const BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// 直接写入设备（daemon 具备完全磁盘访问权限时可用），比 dd 分段更快更平滑。
/// raw 设备要求写入大小是 512 倍数，尾部补零。
fn write_direct(log: &str, image_path: &str, dev: &str, total: u64, task_id: &str) -> Result<(), String> {
    let mut dst = OpenOptions::new()
        .write(true)
        .open(dev)
        .map_err(|e| format!("无法打开设备 {dev}: {e}"))?;
    let mut src = File::open(image_path).map_err(|e| format!("无法打开镜像: {e}"))?;
    let mut tracker = Tracker::new(total);
    let mut buf = vec![0u8; BUFFER_SIZE];
    let mut done: u64 = 0;
    loop {
        check_cancelled(task_id, log)?;
        let n = src.read(&mut buf).map_err(|e| format!("读取镜像失败: {e}"))?;
        if n == 0 {
            break;
        }
        done += n as u64;
        let write_len = if n % 512 == 0 {
            n
        } else {
            let pad = (n as u64).div_ceil(512) * 512;
            buf[n..pad as usize].fill(0);
            pad as usize
        };
        dst.write_all(&buf[..write_len])
            .map_err(|e| format!("写入设备失败: {e}"))?;
        tracker.set(done, log, "flashing")?;
    }
    dst.flush().map_err(|e| format!("刷新设备失败: {e}"))?;
    Ok(())
}

/// 直接读回校验（内存逐块比较），比 cmp 分段快一个数量级
fn verify_direct(log: &str, image_path: &str, dev: &str, total: u64, task_id: &str) -> Result<(), String> {
    let mut src = File::open(image_path).map_err(|e| format!("无法打开镜像: {e}"))?;
    let mut dst = File::open(dev).map_err(|e| format!("无法打开设备 {dev}: {e}"))?;
    let mut tracker = Tracker::new(total);
    let mut a = vec![0u8; BUFFER_SIZE];
    let mut b = vec![0u8; BUFFER_SIZE];
    let mut done: u64 = 0;
    loop {
        check_cancelled(task_id, log)?;
        let na = src.read(&mut a).map_err(|e| format!("校验读取镜像失败: {e}"))?;
        if na == 0 {
            break;
        }
        let padded = (na as u64).div_ceil(512) * 512;
        if padded != na as u64 {
            a[na..padded as usize].fill(0);
        }
        dst.read_exact(&mut b[..padded as usize])
            .map_err(|e| format!("校验读取设备失败: {e}"))?;
        if a[..padded as usize] != b[..padded as usize] {
            let off = a[..na]
                .iter()
                .zip(b[..na].iter())
                .position(|(x, y)| x != y)
                .unwrap_or(0);
            return Err(format!(
                "校验失败：偏移量 {} 字节处数据不一致",
                done + off as u64
            ));
        }
        done += na as u64;
        tracker.set(done, log, "verifying")?;
    }
    Ok(())
}

fn export_direct(
    log: &str,
    dev: &str,
    image_path: &str,
    total: u64,
    task_id: &str,
    start: u64,
    length: u64,
) -> Result<(), String> {
    let mut src = File::open(dev).map_err(|e| format!("无法打开设备 {dev}: {e}"))?;
    if start > 0 {
        std::io::Seek::seek(&mut src, std::io::SeekFrom::Start(start))
            .map_err(|e| format!("无法定位设备: {e}"))?;
    }
    let _ = std::fs::remove_file(image_path);
    let mut dst = File::create(image_path).map_err(|e| format!("无法创建镜像文件: {e}"))?;
    let mut tracker = Tracker::new(length);
    let mut buf = vec![0u8; BUFFER_SIZE];
    let mut done: u64 = 0;
    loop {
        check_cancelled(task_id, log)?;
        let n = src.read(&mut buf).map_err(|e| format!("读取设备失败: {e}"))?;
        if n == 0 {
            break;
        }
        let write_len = (n as u64).min(length - done) as usize;
        dst.write_all(&buf[..write_len])
            .map_err(|e| format!("写入镜像文件失败: {e}"))?;
        done += write_len as u64;
        tracker.set(done, log, "exporting")?;
        if done >= length {
            break;
        }
    }
    dst.flush().map_err(|e| format!("刷新文件失败: {e}"))?;
    Ok(())
}

fn verify_dd(log: &str, image_path: &str, device_path: &str, task_id: &str) -> Result<(), String> {
    let total = std::fs::metadata(image_path)
        .map_err(|e| format!("无法读取镜像: {e}"))?
        .len();
    let blocks = total.div_ceil(DD_BS);
    let mut tracker = Tracker::new(total);
    let mut done_blocks: u64 = 0;

    while done_blocks < blocks {
        check_cancelled(task_id, log)?;
        let count = (blocks - done_blocks).min(SEG_BLOCKS);
        let off = done_blocks * DD_BS;
        let n = (count * DD_BS).min(total - off);
        let out = Command::new("/usr/bin/cmp")
            .arg("-i")
            .arg(format!("{off}:{off}"))
            .arg("-n")
            .arg(n.to_string())
            .arg(image_path)
            .arg(device_path)
            .output()
            .map_err(|e| format!("无法执行 cmp: {e}"))?;

        match out.status.code() {
            Some(0) => {}
            Some(1) => {
                let msg = String::from_utf8_lossy(&out.stderr);
                let char_no = msg
                    .split("char ")
                    .nth(1)
                    .and_then(|s| s.split(',').next())
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .unwrap_or(1);
                return Err(format!(
                    "校验失败：偏移量 {} 字节处数据不一致",
                    off + char_no - 1
                ));
            }
            _ => {
                let msg = String::from_utf8_lossy(&out.stderr);
                return Err(format!("校验命令失败: {}", msg.trim()));
            }
        }

        done_blocks += count;
        tracker.set((done_blocks * DD_BS).min(total), log, "verifying")?;
    }
    Ok(())
}

/// 分段调用 /bin/dd（Apple 系统二进制，具备磁盘访问权限），每段更新进度。
/// raw 设备要求写入大小必须是 512 字节的倍数，因此尾部不足 512 的部分补零后写入。
fn dd_io(log: &str, stage: &str, input: &str, output: &str, total: u64, task_id: &str) -> Result<(), String> {
    let tail_off = total - total % DD_BS;
    let blocks = tail_off / DD_BS;
    let mut tracker = Tracker::new(total);
    let mut done_blocks: u64 = 0;

    while done_blocks < blocks {
        check_cancelled(task_id, log)?;
        let count = (blocks - done_blocks).min(SEG_BLOCKS);
        let out = Command::new("/bin/dd")
            .arg(format!("if={input}"))
            .arg(format!("of={output}"))
            .arg(format!("bs={DD_BS}"))
            .arg(format!("skip={done_blocks}"))
            .arg(format!("seek={done_blocks}"))
            .arg(format!("count={count}"))
            .output()
            .map_err(|e| format!("无法执行 dd: {e}"))?;
        if !out.status.success() {
            let msg = String::from_utf8_lossy(&out.stderr);
            return Err(format!(
                "写入失败：dd 退出码 {:?}，错误: {}",
                out.status.code(),
                msg.trim()
            ));
        }
        done_blocks += count;
        tracker.set((done_blocks * DD_BS).min(total), log, stage)?;
    }

    if total % DD_BS != 0 {
        let tmp_tail = format!("/tmp/flash-helper-tail-{}.bin", std::process::id());
        let rem = total - tail_off;
        let mut f = File::open(input).map_err(|e| format!("无法打开镜像: {e}"))?;
        std::io::Seek::seek(&mut f, std::io::SeekFrom::Start(tail_off))
            .map_err(|e| format!("无法定位镜像: {e}"))?;
        let mut buf = vec![0u8; rem as usize];
        f.read_exact(&mut buf).map_err(|e| format!("读取镜像尾部失败: {e}"))?;
        drop(f);
        let pad_len = (rem).div_ceil(512) * 512;
        buf.resize(pad_len as usize, 0);
        std::fs::write(&tmp_tail, &buf).map_err(|e| format!("无法写临时文件: {e}"))?;

        let out = Command::new("/bin/dd")
            .arg(format!("if={tmp_tail}"))
            .arg(format!("of={output}"))
            .arg("bs=512")
            .arg(format!("seek={}", tail_off / 512))
            .arg(format!("count={}", pad_len / 512))
            .output()
            .map_err(|e| format!("无法执行 dd: {e}"))?;
        let _ = std::fs::remove_file(&tmp_tail);
        if !out.status.success() {
            let msg = String::from_utf8_lossy(&out.stderr);
            return Err(format!(
                "写入尾部失败：dd 退出码 {:?}，错误: {}",
                out.status.code(),
                msg.trim()
            ));
        }
        tracker.set(total, log, stage)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn raw_device(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/dev/disk") {
        return format!("/dev/rdisk{rest}");
    }
    path.to_string()
}

#[cfg(not(target_os = "macos"))]
fn raw_device(path: &str) -> String {
    path.to_string()
}

#[cfg(target_os = "macos")]
fn unmount_device(device_path: &str) {
    if let Some(rest) = device_path.strip_prefix("/dev/disk") {
        let _ = Command::new("diskutil")
            .args(["unmountDisk", "force", &format!("disk{rest}")])
            .output();
    }
}

#[cfg(not(target_os = "macos"))]
fn unmount_device(_device_path: &str) {}

/// 通过 diskutil（Apple 二进制）获取设备大小，避免直接 open 设备被 TCC 拦截
#[cfg(target_os = "macos")]
fn device_size(device_path: &str) -> Result<u64, String> {
    let disk_name = device_path
        .trim_start_matches("/dev/")
        .trim_start_matches("rdisk")
        .trim_start_matches("disk");
    let out = Command::new("diskutil")
        .args(["info", "-plist", &format!("disk{disk_name}")])
        .output()
        .map_err(|e| format!("无法执行 diskutil: {e}"))?;
    let plist = String::from_utf8_lossy(&out.stdout);
    for (i, line) in plist.lines().enumerate() {
        if line.contains("<key>Size</key>") {
            if let Some(next) = plist.lines().nth(i + 1) {
                if let Some(v) = next.trim().strip_prefix("<integer>") {
                    if let Some(num) = v.strip_suffix("</integer>") {
                        if let Ok(n) = num.parse::<u64>() {
                            return Ok(n);
                        }
                    }
                }
            }
        }
    }
    // 回退：普通文件/块设备的大小
    if let Ok(md) = std::fs::metadata(device_path) {
        return Ok(md.len());
    }
    Err("无法获取设备大小（diskutil 输出解析失败）".to_string())
}

#[cfg(not(target_os = "macos"))]
fn device_size(_path: &str) -> Result<u64, String> {
    Err("此平台暂不支持导出功能".to_string())
}

struct Tracker {
    total: u64,
    done: u64,
    start: Instant,
    last_log: Instant,
}

impl Tracker {
    fn new(total: u64) -> Self {
        Self {
            total,
            done: 0,
            start: Instant::now(),
            last_log: Instant::now(),
        }
    }

    fn set(&mut self, done: u64, log: &str, stage: &str) -> Result<(), String> {
        self.done = done;
        let now = Instant::now();
        if now.duration_since(self.last_log).as_millis() < 100 {
            return Ok(());
        }
        let elapsed = now.duration_since(self.start).as_secs_f64().max(0.001);
        let speed = self.done as f64 / elapsed / 1e6;
        let percent = (self.done as f64 / self.total as f64) * 100.0;
        let eta = if speed > 0.001 {
            (self.total - self.done) as f64 / (speed * 1e6)
        } else {
            0.0
        };
        log_line(
            log,
            &json!({
                "stage": stage,
                "percent": percent,
                "speed_mbps": speed,
                "eta_seconds": eta,
                "bytes_done": self.done,
                "bytes_total": self.total,
            }),
        )?;
        self.last_log = now;
        Ok(())
    }
}

fn log_line(log: &str, line: &serde_json::Value) -> Result<(), String> {
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .map_err(|e| format!("无法写入日志文件: {e}"))?;
    writeln!(f, "{line}").map_err(|e| format!("写入日志失败: {e}"))
}
