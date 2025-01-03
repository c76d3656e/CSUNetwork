use chrono::Local;
use std::fs::{self, OpenOptions};
use env_logger::{Builder, fmt::Color};
use std::io::Write;
use std::path::Path;
use log::LevelFilter;
use std::sync::Once;

static LOGGER_INIT: Once = Once::new();

pub struct Logger;

impl Logger {
    /// 初始化日志系统
    /// 配置日志输出格式，同时输出到控制台和文件
    pub fn init() -> Result<(), Box<dyn std::error::Error>> {
        LOGGER_INIT.call_once(|| {
            if let Err(e) = Self::init_logger_internal() {
                eprintln!("Failed to initialize logger: {}", e);
            }
        });
        Ok(())
    }

    /// 获取日志文件路径和句柄
    fn get_log_file() -> Result<(std::fs::File, String), Box<dyn std::error::Error>> {
        // 创建日志目录
        fs::create_dir_all("./logs")?;

        // 生成当月的日志文件名
        let current_time = Local::now();
        let log_file_name = format!(
            "./logs/campus_network_{}.log",
            current_time.format("%Y-%m")
        );

        // 检查文件是否已存在
        let file_exists = Path::new(&log_file_name).exists();

        // 打开或创建日志文件（追加模式）
        let mut log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_name)?;

        // 如果是新文件，写入文件头
        if !file_exists {
            writeln!(log_file, "\n=== 日志开始于 {} ===\n", 
                current_time.format("%Y-%m-%d %H:%M:%S"))?;
        } else {
            writeln!(log_file, "\n=== 程序启动于 {} ===\n",
                current_time.format("%Y-%m-%d %H:%M:%S"))?;
        }

        Ok((log_file, log_file_name))
    }

    /// 内部初始化函数
    fn init_logger_internal() -> Result<(), Box<dyn std::error::Error>> {
        // 获取日志文件
        let (log_file, _) = Self::get_log_file()?;

        // 创建多重写入器
        let multi_writer = MultiWriter::new(vec![
            Box::new(log_file),
            Box::new(std::io::stderr()),
        ]);

        // 创建日志构建器
        let mut builder = Builder::new();
        
        // 设置日志格式
        builder.format(|buf, record| {
            let mut style = buf.style();
            let level_color = match record.level() {
                log::Level::Error => Color::Red,
                log::Level::Warn => Color::Yellow,
                log::Level::Info => Color::Green,
                log::Level::Debug => Color::Blue,
                log::Level::Trace => Color::Cyan,
            };
            style.set_color(level_color).set_bold(true);

            writeln!(
                buf,
                "[{}] {} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                style.value(record.level()),
                record.target(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .target(env_logger::Target::Pipe(Box::new(multi_writer)));

        // 初始化日志系统
        builder.init();

        Ok(())
    }
}

/// 多重写入器结构体，用于同时写入多个输出目标
struct MultiWriter {
    writers: Vec<Box<dyn Write + Send + Sync>>,
}

impl MultiWriter {
    fn new(writers: Vec<Box<dyn Write + Send + Sync>>) -> Self {
        Self { writers }
    }
}

impl Write for MultiWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for writer in &mut self.writers {
            writer.write_all(buf)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        for writer in &mut self.writers {
            writer.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use log::{info, error, warn};

    #[test]
    fn test_logger_initialization() {
        // 初始化日志系统
        assert!(Logger::init().is_ok());
        
        // 写入测试日志
        info!("Test info message");
        warn!("Test warning message");
        error!("Test error message");
        
        // 验证日志文件是否创建
        let logs_dir = Path::new("./logs");
        assert!(logs_dir.exists());
        assert!(logs_dir.is_dir());
        
        // 清理测试文件
        let _ = fs::remove_dir_all(logs_dir);
    }

    #[test]
    fn test_log_file_creation() {
        // 测试日志文件创建
        let result = Logger::get_log_file();
        assert!(result.is_ok());
        
        let (_, file_name) = result.unwrap();
        let log_file = Path::new(&file_name);
        assert!(log_file.exists());
        
        // 清理测试文件
        let _ = fs::remove_file(log_file);
        let _ = fs::remove_dir("./logs");
    }

    #[test]
    fn test_multi_writer() {
        // 创建测试文件
        let test_file = tempfile::NamedTempFile::new().unwrap();
        let test_file2 = tempfile::NamedTempFile::new().unwrap();
        
        // 创建多重写入器
        let mut writer = MultiWriter::new(vec![
            Box::new(test_file.reopen().unwrap()),
            Box::new(test_file2.reopen().unwrap()),
        ]);
        
        // 写入测试数据
        let test_data = b"Test message\n";
        let write_result = writer.write(test_data);
        assert!(write_result.is_ok());
        assert_eq!(write_result.unwrap(), test_data.len());
        
        // 验证数据写入
        let content1 = fs::read(test_file.path()).unwrap();
        let content2 = fs::read(test_file2.path()).unwrap();
        assert_eq!(content1, test_data);
        assert_eq!(content2, test_data);
    }
} 