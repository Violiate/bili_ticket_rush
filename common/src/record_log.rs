use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, TryLockError};

use chrono::{DateTime, Local, NaiveDate};
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use once_cell::sync::Lazy;

// 每个抢票 async 任务携带的短标签，如 "#1 昵称"；非任务上下文中 try_with 返回 Err 自然无前缀
tokio::task_local! {
    pub static TASK_LABEL: String;
}

/// 内存中保留的最大日志条数，避免长时间运行导致内存无限增长
pub const MAX_LOG_ENTRIES: usize = 5000;

struct LogFileState {
    date: NaiveDate,
    writer: BufWriter<File>,
}

// 日志文件处理相关内容（带缓冲与日期缓存，避免每行都做日期字符串格式化与系统调用 flush）
static LOG_FILE: Lazy<Mutex<Option<LogFileState>>> = Lazy::new(|| Mutex::new(None));

// 创建新的日志文件
fn create_log_file(now: DateTime<Local>) -> Option<LogFileState> {
    // 确保日志目录存在
    let log_dir = Path::new("Log");
    if let Err(e) = fs::create_dir_all(log_dir) {
        let _ = writeln!(std::io::stderr(), "无法创建日志目录: {}", e);
        return None;
    }

    // 创建带有时间戳的文件名
    let timestamp = now.format("%Y%m%d_%H%M%S");
    let filename = format!("Log/log_{}.log", timestamp);

    // 打开文件
    match OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(&filename)
    {
        Ok(file) => Some(LogFileState {
            date: now.date_naive(),
            writer: BufWriter::new(file),
        }),
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "无法创建日志文件 {}: {}", filename, e);
            None
        }
    }
}

fn should_flush_log_level(level: Level) -> bool {
    matches!(level, Level::Error | Level::Warn | Level::Info)
}

fn write_line_to_writer(writer: &mut BufWriter<File>, level: Level, message: &str) -> bool {
    if writeln!(writer, "{}", message).is_err() {
        return false;
    }
    if should_flush_log_level(level) && writer.flush().is_err() {
        return false;
    }
    true
}

fn write_to_log_file(now: DateTime<Local>, level: Level, message: &str) -> bool {
    let today = now.date_naive();
    let mut file_guard = LOG_FILE.lock().unwrap_or_else(|e| e.into_inner());

    // 检查是否需要按日期切换日志文件
    let create_new_file = match file_guard.as_ref() {
        Some(state) => state.date != today,
        None => true,
    };

    if create_new_file {
        if let Some(prev) = file_guard.as_mut() {
            let _ = prev.writer.flush();
        }
        if let Some(new_state) = create_log_file(now) {
            *file_guard = Some(new_state);
        }
    }

    // 写入日志；Error/Warn/Info 级别立即 flush 确保实时落盘，仅 Debug/Trace 保持缓冲
    if let Some(state) = file_guard.as_mut() {
        return write_line_to_writer(&mut state.writer, level, message);
    }

    false
}

// 日志记录器（有界环形队列）
pub struct LogCollector {
    pub logs: VecDeque<String>,
}

impl Default for LogCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl LogCollector {
    pub fn new() -> Self {
        Self {
            logs: VecDeque::new(),
        }
    }

    // 添加日志，超过容量时丢弃最旧条目
    pub fn add(&mut self, message: String) {
        while self.logs.len() >= MAX_LOG_ENTRIES {
            self.logs.pop_front();
        }
        self.logs.push_back(message);
    }

    // 获取并排空日志
    pub fn get_logs(&mut self) -> Option<Vec<String>> {
        if self.logs.is_empty() {
            None
        } else {
            Some(self.logs.drain(..).collect())
        }
    }

    // 清空日志
    pub fn clear_logs(&mut self) {
        self.logs.clear();
    }
}

pub static LOG_COLLECTOR: Lazy<Arc<Mutex<LogCollector>>> =
    Lazy::new(|| Arc::new(Mutex::new(LogCollector::new())));

fn is_workspace_target(target: &str) -> bool {
    matches!(
        target.split("::").next(),
        Some("common" | "backend" | "frontend")
    )
}

struct CollectorLogger;

impl log::Log for CollectorLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level() && is_workspace_target(metadata.target())
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let now = Local::now();
        let timestamp = now.format("%Y-%m-%d %H:%M:%S.%3f");
        let label = TASK_LABEL.try_with(|l| l.clone()).ok();
        let log_message = match label {
            Some(l) => format!("[{}] {} [{}]: {}", timestamp, record.level(), l, record.args()),
            None => format!("[{}] {}: {}", timestamp, record.level(), record.args()),
        };

        // 使用 try_lock 避免长时间等待或潜在重入死锁，并兼容 poisoned 锁
        match LOG_COLLECTOR.try_lock() {
            Ok(mut collector) => collector.add(log_message.clone()),
            Err(TryLockError::Poisoned(e)) => e.into_inner().add(log_message.clone()),
            Err(TryLockError::WouldBlock) => {}
        }

        #[cfg(debug_assertions)]
        {
            let _ = writeln!(std::io::stdout(), "{}", log_message);
        }

        // 单独处理文件写入，避免同时持有多个锁
        let _ = write_to_log_file(now, record.level(), &log_message);
    }

    fn flush(&self) {
        // 使用 try_lock 避免在 panic hook 等异常路径下发生死锁
        match LOG_FILE.try_lock() {
            Ok(mut guard) => {
                if let Some(state) = guard.as_mut() {
                    let _ = state.writer.flush();
                }
            }
            Err(TryLockError::Poisoned(e)) => {
                let mut guard = e.into_inner();
                if let Some(state) = guard.as_mut() {
                    let _ = state.writer.flush();
                }
            }
            Err(TryLockError::WouldBlock) => {}
        }
    }
}

// 静态日志记录器
static LOGGER: CollectorLogger = CollectorLogger;

// 初始化日志系统
pub fn init() -> Result<(), SetLoggerError> {
    #[cfg(debug_assertions)]
    {
        let _ = writeln!(std::io::stdout(), "调试模式启动");
    }

    // 根据构建模式设置不同的日志级别
    log::set_logger(&LOGGER).map(|()| {
        if cfg!(debug_assertions) {
            log::set_max_level(LevelFilter::Debug);
        } else {
            log::set_max_level(LevelFilter::Info);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_collector_is_bounded_to_max_entries() {
        let mut collector = LogCollector::new();
        for i in 0..(MAX_LOG_ENTRIES + 250) {
            collector.add(format!("entry-{}", i));
        }

        assert_eq!(collector.logs.len(), MAX_LOG_ENTRIES);
        assert_eq!(collector.logs.front().map(String::as_str), Some("entry-250"));
        let expected_last = format!("entry-{}", MAX_LOG_ENTRIES + 249);
        assert_eq!(
            collector.logs.back().map(String::as_str),
            Some(expected_last.as_str())
        );
    }

    #[test]
    fn log_collector_get_logs_drains_properly() {
        let mut collector = LogCollector::new();
        assert!(collector.get_logs().is_none());

        collector.add("first".to_string());
        collector.add("second".to_string());

        let drained = collector.get_logs().expect("should return collected logs");
        assert_eq!(drained, vec!["first".to_string(), "second".to_string()]);
        assert!(collector.logs.is_empty());
        assert!(collector.get_logs().is_none());
    }

    #[test]
    fn workspace_target_filter_blocks_noisy_dependencies() {
        assert!(is_workspace_target("common"));
        assert!(is_workspace_target("common::push"));
        assert!(is_workspace_target("backend::api"));
        assert!(is_workspace_target("frontend::ui::tabs"));

        assert!(!is_workspace_target("hyper::proto::h2"));
        assert!(!is_workspace_target("h2::codec"));
        assert!(!is_workspace_target("reqwest::connect"));
        assert!(!is_workspace_target("egui"));
        assert!(!is_workspace_target("eframe::native"));
        assert!(!is_workspace_target("winit::platform_impl"));
        assert!(!is_workspace_target("rustls::client"));
        assert!(!is_workspace_target("common_other::mod"));
        assert!(!is_workspace_target(""));
    }

    #[test]
    fn info_warn_error_levels_flush_immediately_while_debug_buffers() {
        assert!(should_flush_log_level(Level::Error));
        assert!(should_flush_log_level(Level::Warn));
        assert!(should_flush_log_level(Level::Info));
        assert!(!should_flush_log_level(Level::Debug));
        assert!(!should_flush_log_level(Level::Trace));

        let temp_path = std::env::temp_dir().join(format!(
            "biliticket_log_flush_test_{}_{}.log",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)
            .expect("create temp log file");
        let mut writer = BufWriter::new(file);

        assert!(write_line_to_writer(
            &mut writer,
            Level::Debug,
            "debug-buffered-only"
        ));
        let after_debug = fs::read_to_string(&temp_path).expect("read after debug");
        assert!(
            after_debug.is_empty(),
            "Debug log should remain buffered before flush"
        );

        assert!(write_line_to_writer(
            &mut writer,
            Level::Info,
            "order_id=987654321"
        ));
        let after_info = fs::read_to_string(&temp_path).expect("read after info");
        assert!(after_info.contains("debug-buffered-only"));
        assert!(after_info.contains("order_id=987654321"));

        drop(writer);
        let _ = fs::remove_file(&temp_path);
    }
}
