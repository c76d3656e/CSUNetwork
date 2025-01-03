use std::sync::atomic::{AtomicBool, Ordering};
use log::info;
use std::time::Duration;
use surge_ping::{Client, Config as PingConfig, PingIdentifier, PingSequence};
use std::net::ToSocketAddrs;
use std::sync::Arc;
use rand::random;

// 定义一个宏来同时输出到日志和控制台
macro_rules! log_and_print {
    ($level:expr, $($arg:tt)+) => {{
        let message = format!($($arg)+);
        println!("{}", message);
        match $level {
            "info" => info!("{}", message),
            "error" => log::error!("{}", message),
            "warn" => log::warn!("{}", message),
            "debug" => log::debug!("{}", message),
            "trace" => log::trace!("{}", message),
            _ => info!("{}", message),
        }
    }};
}

pub struct NetworkMonitor {
    is_connected: AtomicBool,
    ping_client: Arc<Client>,
}

impl NetworkMonitor {
    pub fn new() -> Self {
        let config = PingConfig::default();
        let client = Arc::new(Client::new(&config).unwrap());
        
        Self {
            is_connected: AtomicBool::new(false),
            ping_client: client,
        }
    }

    pub async fn init() -> Self {
        let config = PingConfig::default();
        let client = Arc::new(Client::new(&config).unwrap());
        
        Self {
            is_connected: AtomicBool::new(false),
            ping_client: client,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }

    pub async fn check_connection(&self) {
        // 定义多个检测目标
        let test_targets = vec![
            "www.baidu.com",
            "www.opendns.com",
            "1.1.1.1",
            "114.114.114.114",  // 114 DNS
            "8.8.8.8",          // Google DNS
            "223.5.5.5",        // AliDNS
        ];

        log_and_print!("info", "Network connection check started");
        
        for target in test_targets {
            log_and_print!("info", "Pinging {}", target);
            
            // 解析域名为IP地址
            if let Ok(mut addrs) = format!("{}:80", target).to_socket_addrs() {
                if let Some(addr) = addrs.next() {
                    let ip = addr.ip();
                    
                    // 创建pinger，使用随机标识符
                    let mut pinger = self.ping_client.pinger(ip, PingIdentifier(random::<u16>())).await;
                    
                    // 执行ping，使用序列号0和默认payload
                    match pinger.ping(PingSequence(0), &[0; 16]).await {
                        Ok((_, duration)) => {
                            log_and_print!("info", "Ping successful to {} ({}ms)", target, duration.as_millis());
                            self.is_connected.store(true, Ordering::Relaxed);
                            log_and_print!("info", "Network status: Connected");
                            return;
                        }
                        Err(e) => {
                            log_and_print!("info", "Failed to ping {}: {}", target, e);
                        }
                    }
                } else {
                    log_and_print!("info", "Could not resolve IP address for {}", target);
                }
            } else {
                log_and_print!("info", "Failed to resolve {}", target);
            }
            
            // 每次ping之间稍微等待一下
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // 所有目标都无法连通
        self.is_connected.store(false, Ordering::Relaxed);
        log_and_print!("info", "Network status: Disconnected (all ping targets unreachable)");
    }

    // 用于测试的方法
    #[cfg(test)]
    pub fn set_connected(&self, connected: bool) {
        self.is_connected.store(connected, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_network_monitor_initialization() {
        let monitor = NetworkMonitor::new();
        assert!(!monitor.is_connected());
        
        // 测试 ping_client 是否正确初始化
        assert!(Arc::strong_count(&monitor.ping_client) == 1);
    }

    #[tokio::test]
    async fn test_network_monitor_init() {
        let monitor = NetworkMonitor::init().await;
        assert!(!monitor.is_connected());
        
        // 测试 ping_client 是否正确初始化
        assert!(Arc::strong_count(&monitor.ping_client) == 1);
    }

    #[tokio::test]
    async fn test_set_connected() {
        let monitor = NetworkMonitor::new();
        assert!(!monitor.is_connected());

        // 测试设置连接状态
        monitor.set_connected(true);
        assert!(monitor.is_connected());

        monitor.set_connected(false);
        assert!(!monitor.is_connected());
    }

    #[tokio::test]
    async fn test_check_connection() {
        let monitor = NetworkMonitor::new();
        
        // 执行连接检查
        monitor.check_connection().await;
        
        // 获取连接状态
        let is_connected = monitor.is_connected();
        
        // 由于这是实际的网络测试，我们只记录结果而不断言具体状态
        log_and_print!("info", "Network connection test result: {}", 
            if is_connected { "Connected" } else { "Disconnected" }
        );
    }

    #[tokio::test]
    async fn test_multiple_connection_checks() {
        let monitor = NetworkMonitor::new();
        
        // 执行多次连接检查
        for i in 0..3 {
            log_and_print!("info", "Running connection check iteration {}", i + 1);
            monitor.check_connection().await;
            let is_connected = monitor.is_connected();
            log_and_print!("info", "Connection check {} result: {}", 
                i + 1,
                if is_connected { "Connected" } else { "Disconnected" }
            );
            
            // 在检查之间添加短暂延迟
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    #[tokio::test]
    async fn test_concurrent_connection_checks() {
        let monitor = Arc::new(NetworkMonitor::new());
        let mut handles = Vec::new();
        
        // 创建多个并发的连接检查
        for i in 0..3 {
            let monitor_clone = Arc::clone(&monitor);
            let handle = tokio::spawn(async move {
                log_and_print!("info", "Starting concurrent check {}", i + 1);
                monitor_clone.check_connection().await;
                log_and_print!("info", "Concurrent check {} completed, status: {}", 
                    i + 1,
                    if monitor_clone.is_connected() { "Connected" } else { "Disconnected" }
                );
            });
            handles.push(handle);
        }
        
        // 等待所有检查完成
        for handle in handles {
            handle.await.expect("Connection check task failed");
        }
    }
} 