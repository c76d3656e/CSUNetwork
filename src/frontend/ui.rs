// 前端界面模块
use eframe::egui;
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::runtime::Runtime;
use std::time::Duration;
use crate::backend::network_monitor::NetworkMonitor;
use crate::backend::config::{Config, ISP};
use crate::backend::authentication::Authenticator;

// UI主结构体
pub struct UI {
    pub network_monitor: Arc<NetworkMonitor>,
    pub config: Config,
    pub log_messages: Vec<String>,
    authenticator: Option<Authenticator>,
    auto_login_handle: Option<std::thread::JoinHandle<()>>,
    network_monitor_handle: Option<std::thread::JoinHandle<()>>,
    last_network_status: bool,
    chrome_installed: bool,
}

impl UI {
    // 创建新的UI实例
    pub fn new(network_monitor: Arc<NetworkMonitor>) -> Self {
        // 尝试加载配置，如果失败则使用默认值
        let config = Config::load().unwrap_or_else(|_| Config::default());
        
        let mut ui = Self {
            network_monitor,
            config,
            log_messages: Vec::new(),
            authenticator: None,
            auto_login_handle: None,
            network_monitor_handle: None,
            last_network_status: false,
            chrome_installed: Self::check_chrome_installed(),
        };

        // 启动网络监控线程
        ui.start_network_monitor();
        
        // 如果配置了自动登录，启动自动登录线程
        if ui.config.auto_login && !ui.config.username.is_empty() && !ui.config.password.is_empty() {
            ui.start_auto_login();
        }
        
        ui
    }

    // 检查 Chrome 和 ChromeDriver 是否已安装
    fn check_chrome_installed() -> bool {
        let current_dir = std::env::current_dir().unwrap_or_default();
        let chrome_exists = current_dir.join("chrome-win32").exists();
        let chromedriver_exists = current_dir.join("chromedriver.exe").exists();
        chrome_exists && chromedriver_exists
    }

    // 安装 Chrome 和 ChromeDriver
    async fn install_chrome(&mut self) {
        self.add_log("Starting Chrome and ChromeDriver installation...".to_string());
        
        // 创建一个新的线程来处理安装过程
        let log_messages = Arc::new(Mutex::new(Vec::new()));
        let log_messages_clone = Arc::clone(&log_messages);
        
        let handle = std::thread::spawn(move || {
            let rt = match Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    log_messages_clone.lock().push(format!("Failed to create runtime: {}", e));
                    return;
                }
            };

            rt.block_on(async {
                match crate::backend::downloader::Downloader::ensure_chrome_and_driver_async().await {
                    Ok(_) => {
                        log_messages_clone.lock().push("Chrome and ChromeDriver installed successfully".to_string());
                    }
                    Err(e) => {
                        log_messages_clone.lock().push(format!("Installation failed: {}", e));
                        // 添加更详细的错误信息
                        if e.to_string().contains("tcp connect error") {
                            log_messages_clone.lock().push("Network error: Please check your internet connection".to_string());
                        } else if e.to_string().contains("permission denied") {
                            log_messages_clone.lock().push("Permission error: Please run the program with administrator privileges".to_string());
                        }
                    }
                }
            });
        });

        // 等待安装完成
        if let Ok(_) = handle.join() {
            // 获取日志消息并添加到UI
            if let Ok(messages) = Arc::try_unwrap(log_messages) {
                let messages = messages.into_inner();
                for msg in messages {
                    self.add_log(msg);
                }
            }
        }

        // 更新安装状态
        self.chrome_installed = Self::check_chrome_installed();
    }

    // 创建新的UI实例（用于测试）
    #[cfg(test)]
    pub fn new_empty(network_monitor: Arc<NetworkMonitor>) -> Self {
        let mut ui = Self {
            network_monitor,
            config: Config {
                auth_url: "http://10.1.1.1".to_string(),
                ..Default::default()
            },
            log_messages: Vec::new(),
            authenticator: None,
            auto_login_handle: None,
            network_monitor_handle: None,
            last_network_status: false,
            chrome_installed: false,
        };

        // 启动网络监控线程
        ui.start_network_monitor();
        
        ui
    }

    // 启动网络监控线程
    fn start_network_monitor(&mut self) {
        let network_monitor = Arc::clone(&self.network_monitor);
        let log_messages = Arc::new(Mutex::new(Vec::new()));
        let log_messages_clone = Arc::clone(&log_messages);

        let handle = std::thread::spawn(move || {
            let rt = Runtime::new().expect("Failed to create runtime");
            let mut last_status = false;
            
            loop {
                // 使用runtime执行异步网络检查
                rt.block_on(async {
                    network_monitor.check_connection().await;
                });

                // 获取当前网络状态
                let current_status = network_monitor.is_connected();
                
                // 如果状态发生变化，记录日志
                if current_status != last_status {
                    log_messages_clone.lock().push(format!("Network status changed to: {}", 
                        if current_status { "Connected" } else { "Disconnected" }
                    ));
                    last_status = current_status;
                }
                
                // 每30秒检查一次网络状态
                std::thread::sleep(Duration::from_secs(30));
            }
        });

        self.network_monitor_handle = Some(handle);
    }

    // 运行UI程序
    pub fn run(self) -> Result<(), eframe::Error> {
        let options = eframe::NativeOptions::default();
        eframe::run_native(
            "Campus Network Assistant",
            options,
            Box::new(|_cc| Box::new(self)),
        )
    }

    // 添加日志记录
    fn add_log(&mut self, message: String) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        self.log_messages.push(format!("[{}] {}", timestamp, message));
        if self.log_messages.len() > 100 {
            self.log_messages.remove(0);
        }
    }

    // 保存配置
    fn save_config(&mut self) {
        if let Err(e) = self.config.save() {
            self.add_log(format!("Failed to save config: {}", e));
        } else {
            self.add_log("Configuration saved successfully".to_string());
        }
    }

    // 获取网络状态文本和颜色
    fn get_network_status(&self) -> (&'static str, egui::Color32) {
        if self.network_monitor.is_connected() {
            ("Connected", egui::Color32::GREEN)
        } else {
            ("Disconnected", egui::Color32::RED)
        }
    }

    // 初始化认证器
    async fn init_authenticator(&mut self) -> bool {
        let config = Arc::new(self.config.clone());
        let mut auth = Authenticator::new(config);
        match auth.init().await {
            Ok(_) => {
                self.authenticator = Some(auth);
                self.add_log("Authentication system initialized".to_string());
                true
            }
            Err(e) => {
                self.add_log(format!("Failed to initialize authentication system: {}", e));
                false
            }
        }
    }

    // 打开认证页面并执行登录
    fn perform_login(&mut self) {
        self.add_log("Starting login process".to_string());
        
        // 克隆需要的数据
        let config = Arc::new(self.config.clone());
        let log_messages = Arc::new(Mutex::new(Vec::new()));
        let log_messages_clone = Arc::clone(&log_messages);

        // 创建新线程执行登录
        let handle = std::thread::spawn(move || {
            // 在新线程中创建runtime
            let rt = Runtime::new().expect("Failed to create runtime");
            
            rt.block_on(async {
                let mut auth = Authenticator::new(config);
                if let Err(e) = auth.init().await {
                    log_messages_clone.lock().push(format!("Failed to initialize authenticator: {}", e));
                    return;
                }

                match auth.open_auth_page().await {
                    Ok(_) => {
                        log_messages_clone.lock().push("Authentication page opened".to_string());
                        match auth.login().await {
                            Ok(_) => log_messages_clone.lock().push("Login successful".to_string()),
                            Err(e) => log_messages_clone.lock().push(format!("Login failed: {}", e)),
                        }
                    }
                    Err(e) => log_messages_clone.lock().push(format!("Failed to open authentication page: {}", e)),
                }
            });
        });

        // 等待登录完成
        if let Ok(_) = handle.join() {
            // 获取日志消息并添加到UI
            if let Ok(messages) = Arc::try_unwrap(log_messages) {
                let messages = messages.into_inner();
                for msg in messages {
                    self.add_log(msg);
                }
            }
        }
    }

    // 打开认证页面并执行登出
    fn perform_logout(&mut self) {
        self.add_log("Starting logout process".to_string());
        
        // 克隆需要的数据
        let config = Arc::new(self.config.clone());
        let log_messages = Arc::new(Mutex::new(Vec::new()));
        let log_messages_clone = Arc::clone(&log_messages);

        // 创建新线程执行登出
        let handle = std::thread::spawn(move || {
            // 在新线程中创建runtime
            let rt = Runtime::new().expect("Failed to create runtime");
            
            rt.block_on(async {
                let mut auth = Authenticator::new(config);
                if let Err(e) = auth.init().await {
                    log_messages_clone.lock().push(format!("Failed to initialize authenticator: {}", e));
                    return;
                }

                match auth.open_auth_page().await {
                    Ok(_) => {
                        log_messages_clone.lock().push("Authentication page opened".to_string());
                        match auth.logout().await {
                            Ok(_) => log_messages_clone.lock().push("Logout successful".to_string()),
                            Err(e) => log_messages_clone.lock().push(format!("Logout failed: {}", e)),
                        }
                    }
                    Err(e) => log_messages_clone.lock().push(format!("Failed to open authentication page: {}", e)),
                }
            });
        });

        // 等待登出完成
        if let Ok(_) = handle.join() {
            // 获取日志消息并添加到UI
            if let Ok(messages) = Arc::try_unwrap(log_messages) {
                let messages = messages.into_inner();
                for msg in messages {
                    self.add_log(msg);
                }
            }
        }
    }

    // 开启自动登录线程
    fn start_auto_login(&mut self) {
        // 检查必要的输入是否完整
        if self.config.username.is_empty() || self.config.password.is_empty() {
            self.add_log("Auto login failed: Username or password is empty".to_string());
            return;
        }

        // 克隆需要的数据用于线程
        let config = Arc::new(self.config.clone());
        let network_monitor = Arc::clone(&self.network_monitor);
        let log_messages = Arc::new(Mutex::new(Vec::new()));
        let log_messages_clone = Arc::clone(&log_messages);

        // 启动自动登录线程
        let handle = std::thread::spawn(move || {
            // 在新线程中创建runtime
            let rt = Runtime::new().expect("Failed to create runtime");
            let mut last_status = network_monitor.is_connected();
            let mut login_in_progress = false;
            let mut retry_count = 0;
            
            loop {
                let current_status = network_monitor.is_connected();
                
                // 只有当网络状态从连接变为断开时才尝试登录
                if last_status && !current_status && !login_in_progress {
                    login_in_progress = true;
                    log_messages_clone.lock().push("Network disconnected, attempting auto login...".to_string());
                    
                    rt.block_on(async {
                        let mut auth = Authenticator::new(Arc::clone(&config));
                        match auth.init().await {
                            Ok(_) => {
                                match auth.login().await {
                                    Ok(_) => {
                                        log_messages_clone.lock().push("Auto login successful".to_string());
                                        login_in_progress = false;
                                        retry_count = 0;
                                    }
                                    Err(e) => {
                                        log_messages_clone.lock().push(format!("Auto login failed: {}", e));
                                        retry_count += 1;
                                        // 根据重试次数增加等待时间
                                        let wait_time = if retry_count > 3 {
                                            120 // 如果失败超过3次，等待2分钟
                                        } else {
                                            30 // 否则等待30秒
                                        };
                                        tokio::time::sleep(Duration::from_secs(wait_time)).await;
                                        login_in_progress = false;
                                    }
                                }
                            }
                            Err(e) => {
                                log_messages_clone.lock().push(format!("Failed to initialize authenticator: {}", e));
                                login_in_progress = false;
                                retry_count += 1;
                            }
                        }
                    });
                } else if current_status {
                    // 如果网络已连接，重置重试计数
                    retry_count = 0;
                }
                
                last_status = current_status;
                
                // 根据重试次数调整检查间隔
                let check_interval = if retry_count > 3 {
                    60 // 如果失败次数多，降低检查频率到60秒
                } else {
                    15 // 正常情况下15秒检查一次
                };
                
                std::thread::sleep(Duration::from_secs(check_interval));
            }
        });

        self.auto_login_handle = Some(handle);
        self.add_log("Auto login thread started".to_string());
    }

    // 更新UI中的网络状态显示
    fn update_network_status(&mut self, ui: &mut egui::Ui) {
        let current_status = self.network_monitor.is_connected();
        
        // 如果状态发生变化，更新UI并添加日志
        if current_status != self.last_network_status {
            self.last_network_status = current_status;
            self.add_log(format!("Network status changed to: {}", 
                if current_status { "Connected" } else { "Disconnected" }
            ));
        }

        ui.horizontal(|ui| {
            ui.label("Current Status: ");
            ui.colored_label(
                if current_status { egui::Color32::GREEN } else { egui::Color32::RED },
                if current_status { "Connected" } else { "Disconnected" }
            );
        });
    }
}

impl eframe::App for UI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 顶部面板
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Campus Network Assistant");
            });
        });

        // 主面板
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                ui.heading("Campus Network Login");
                ui.add_space(20.0);
            });

            // 左右分栏布局
            ui.columns(2, |columns| {
                // 左侧面板 - 登录区域
                columns[0].group(|ui| {
                    // 认证URL
                    ui.heading("Authentication Settings");
                    ui.add_space(10.0);
                    
                    ui.horizontal(|ui| {
                        ui.label("Auth URL:").on_hover_text("Enter the authentication URL");
                        if ui.add_sized([200.0, 20.0], egui::TextEdit::singleline(&mut self.config.auth_url)).changed() {
                            self.save_config();
                        }
                    });
                    
                    // 运营商选择
                    ui.horizontal(|ui| {
                        ui.label("ISP:").on_hover_text("Select your Internet Service Provider");
                        egui::ComboBox::from_label("")
                            .selected_text(match self.config.isp {
                                ISP::Mobile => "Mobile",
                                ISP::Unicom => "Unicom",
                                ISP::Telecom => "Telecom",
                                ISP::School => "School",
                            })
                            .show_ui(ui, |ui| {
                                let mut changed = false;
                                changed |= ui.selectable_value(&mut self.config.isp, ISP::Mobile, "Mobile").clicked();
                                changed |= ui.selectable_value(&mut self.config.isp, ISP::Unicom, "Unicom").clicked();
                                changed |= ui.selectable_value(&mut self.config.isp, ISP::Telecom, "Telecom").clicked();
                                changed |= ui.selectable_value(&mut self.config.isp, ISP::School, "School").clicked();
                                if changed {
                                    self.save_config();
                                }
                            });
                    });
                    
                    ui.add_space(20.0);
                    
                    // 账号部分
                    ui.heading("Account");
                    ui.add_space(10.0);
                    
                    // 用户名输入框
                    ui.horizontal(|ui| {
                        ui.label("Username:").on_hover_text("Enter your campus network username");
                        if ui.add_sized([200.0, 20.0], egui::TextEdit::singleline(&mut self.config.username)).changed() {
                            self.save_config();
                        }
                    });
                    
                    // 密码输入框
                    ui.horizontal(|ui| {
                        ui.label("Password:").on_hover_text("Enter your campus network password");
                        if ui.add_sized([200.0, 20.0], egui::TextEdit::singleline(&mut self.config.password)
                            .password(true)).changed() && self.config.remember_password {
                            self.save_config();
                        }
                    });
                    
                    ui.add_space(10.0);
                    
                    // 复选框
                    if ui.checkbox(&mut self.config.remember_password, "Remember Password")
                        .on_hover_text("Save credentials for next login").changed() {
                        if !self.config.remember_password {
                            self.config.auto_login = false;
                        }
                        self.save_config();
                    }

                    if ui.checkbox(&mut self.config.auto_login, "Auto Login")
                        .on_hover_text("Automatically login when application starts")
                        .clicked() {
                        if self.config.auto_login {
                            self.config.remember_password = true;
                            // 启动自动登录线程
                            self.start_auto_login();
                        } else {
                            // 如果取消自动登录，停止自动登录线程
                            if let Some(handle) = self.auto_login_handle.take() {
                                let _ = handle.join();
                            }
                        }
                        self.save_config();
                    }
                    
                    ui.add_space(20.0);
                    
                    // 登录/登出按钮
                    ui.horizontal(|ui| {
                        if ui.add_sized([120.0, 30.0], egui::Button::new("🔑 Login")).clicked() {
                            self.add_log("Starting login process...".to_string());
                            self.perform_login();
                        }
                        ui.add_space(10.0);
                        if ui.add_sized([120.0, 30.0], egui::Button::new("🚪 Logout")).clicked() {
                            self.add_log("Starting logout process...".to_string());
                            self.perform_logout();
                        }
                    });

                    ui.add_space(20.0);

                    // Chrome 安装状态和按钮
                    ui.horizontal(|ui| {
                        // 每次渲染时检查安装状态
                        self.chrome_installed = Self::check_chrome_installed();
                        
                        ui.label("Chrome Status:").on_hover_text("Chrome and ChromeDriver installation status");
                        ui.colored_label(
                            if self.chrome_installed { egui::Color32::GREEN } else { egui::Color32::RED },
                            if self.chrome_installed { "Installed" } else { "Not Installed" }
                        );
                        if !self.chrome_installed {
                            if ui.add_sized([120.0, 30.0], egui::Button::new("🔧 Install Chrome")).clicked() {
                                // 创建一个新的线程来处理安装过程
                                let log_messages = Arc::new(Mutex::new(Vec::new()));
                                let log_messages_clone = Arc::clone(&log_messages);
                                
                                // 克隆 self.add_log 需要的数据
                                let ui_messages = Arc::new(Mutex::new(self.log_messages.clone()));
                                let ui_messages_clone = Arc::clone(&ui_messages);
                                
                                std::thread::spawn(move || {
                                    let rt = match Runtime::new() {
                                        Ok(rt) => rt,
                                        Err(e) => {
                                            let error_msg = format!("Failed to create runtime: {}", e);
                                            log_messages_clone.lock().push(error_msg.clone());
                                            ui_messages_clone.lock().push(error_msg);
                                            return;
                                        }
                                    };

                                    rt.block_on(async {
                                        match crate::backend::downloader::Downloader::ensure_chrome_and_driver_async().await {
                                            Ok(_) => {
                                                let success_msg = "Chrome and ChromeDriver installed successfully".to_string();
                                                log_messages_clone.lock().push(success_msg.clone());
                                                ui_messages_clone.lock().push(success_msg);
                                            }
                                            Err(e) => {
                                                let error_msg = format!("Installation failed: {}", e);
                                                log_messages_clone.lock().push(error_msg.clone());
                                                ui_messages_clone.lock().push(error_msg);

                                                // 添加更详细的错误信息
                                                if e.to_string().contains("tcp connect error") {
                                                    let network_error = "Network error: Please check your internet connection".to_string();
                                                    log_messages_clone.lock().push(network_error.clone());
                                                    ui_messages_clone.lock().push(network_error);
                                                } else if e.to_string().contains("permission denied") {
                                                    let permission_error = "Permission error: Please run the program with administrator privileges".to_string();
                                                    log_messages_clone.lock().push(permission_error.clone());
                                                    ui_messages_clone.lock().push(permission_error);
                                                }
                                            }
                                        }
                                    });
                                });
                            }
                        }
                    });
                });

                // 右侧面板 - 状态和日志
                columns[1].group(|ui| {
                    // 网络状态
                    ui.heading("Network Status");
                    ui.add_space(10.0);
                    
                    // 使用新的网络状态更新方法
                    self.update_network_status(ui);
                    
                    ui.add_space(20.0);
                    
                    // 日志显示区域
                    ui.heading("System Log");
                    ui.add_space(10.0);
                    
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for message in self.log_messages.iter().rev() {
                                ui.label(message);
                            }
                        });
                });
            });
        });

        // 每秒刷新一次UI
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
}

// 测试模块
#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_ui_creation() {
        let network_monitor = Arc::new(NetworkMonitor::new());
        let ui = UI::new_empty(network_monitor);
        assert!(ui.log_messages.is_empty());
        assert_eq!(ui.config.auth_url, "http://10.1.1.1");
        assert!(matches!(ui.config.isp, ISP::School));
    }

    #[tokio::test]
    async fn test_add_log() {
        let network_monitor = Arc::new(NetworkMonitor::new());
        let mut ui = UI::new_empty(network_monitor);
        
        // 测试添加日志
        ui.add_log("Test message 1".to_string());
        assert_eq!(ui.log_messages.len(), 1);
        assert!(ui.log_messages[0].contains("Test message 1"));
        
        // 测试日志轮转
        for i in 0..110 {
            ui.add_log(format!("Test message {}", i));
        }
        assert_eq!(ui.log_messages.len(), 100);
    }

    #[tokio::test]
    async fn test_network_status_display() {
        let network_monitor = Arc::new(NetworkMonitor::new());
        let ui = UI::new_empty(network_monitor.clone());
        
        // 测试初始状态（未连接）
        let (status_text, status_color) = ui.get_network_status();
        assert_eq!(status_text, "Disconnected");
        assert_eq!(status_color, egui::Color32::RED);
        
        // 测试已连接状态
        network_monitor.set_connected(true);
        let (status_text, status_color) = ui.get_network_status();
        assert_eq!(status_text, "Connected");
        assert_eq!(status_color, egui::Color32::GREEN);

        // 测试断开连接状态
        network_monitor.set_connected(false);
        let (status_text, status_color) = ui.get_network_status();
        assert_eq!(status_text, "Disconnected");
        assert_eq!(status_color, egui::Color32::RED);
    }

    #[tokio::test]
    async fn test_config_initialization() {
        let network_monitor = Arc::new(NetworkMonitor::new());
        let ui = UI::new_empty(network_monitor);
        
        // 测试配置初始值
        assert_eq!(ui.config.username, "");
        assert_eq!(ui.config.password, "");
        assert!(!ui.config.remember_password);
        assert!(!ui.config.auto_login);
        assert_eq!(ui.config.auth_url, "http://10.1.1.1");
        assert!(matches!(ui.config.isp, ISP::School));
    }

    #[tokio::test]
    async fn test_login_process() {
        let network_monitor = Arc::new(NetworkMonitor::new());
        let mut ui = UI::new_empty(network_monitor);
        
        // 设置测试配置
        ui.config.username = "test_user".to_string();
        ui.config.password = "test_pass".to_string();
        ui.config.auth_url = "http://10.1.1.1".to_string();
        ui.config.isp = ISP::School;

        // 执行登录
        ui.perform_login();

        // 验证日志消息
        let log_messages: Vec<_> = ui.log_messages.iter().collect();
        assert!(log_messages.iter().any(|msg| msg.contains("Starting login process")), "没有找到登录开始消息");
        
        // 由于没有 ChromeDriver，应该看到初始化失败的消息
        assert!(log_messages.iter().any(|msg| msg.contains("Failed to initialize")), "没有找到初始化失败消息");
    }

    #[tokio::test]
    async fn test_logout_process() {
        let network_monitor = Arc::new(NetworkMonitor::new());
        let mut ui = UI::new_empty(network_monitor);
        
        // 设置测试配置
        ui.config.username = "test_user".to_string();
        ui.config.password = "test_pass".to_string();
        ui.config.auth_url = "http://10.1.1.1".to_string();
        ui.config.isp = ISP::School;

        // 执行登出
        ui.perform_logout();

        // 验证日志消息
        let log_messages: Vec<_> = ui.log_messages.iter().collect();
        assert!(log_messages.iter().any(|msg| msg.contains("Starting logout process")), "没有找到登出开始消息");
        
        // 由于没有 ChromeDriver，应该看到初始化失败的消息
        assert!(log_messages.iter().any(|msg| msg.contains("Failed to initialize")), "没有找到初始化失败消息");
    }

    #[tokio::test]
    async fn test_login_process_no_authenticator() {
        let network_monitor = Arc::new(NetworkMonitor::new());
        let mut ui = UI::new_empty(network_monitor);
        
        // 不设置任何配置，直接尝试登录
        ui.perform_login();

        // 验证日志消息
        let log_messages: Vec<_> = ui.log_messages.iter().collect();
        assert!(log_messages.iter().any(|msg| msg.contains("Starting login process")), "没有找到登录开始消息");
        assert!(log_messages.iter().any(|msg| msg.contains("Failed to initialize")), "没有找到初始化失败消息");
    }

    #[tokio::test]
    async fn test_logout_process_no_authenticator() {
        let network_monitor = Arc::new(NetworkMonitor::new());
        let mut ui = UI::new_empty(network_monitor);
        
        // 不设置任何配置，直接尝试登出
        ui.perform_logout();

        // 验证日志消息
        let log_messages: Vec<_> = ui.log_messages.iter().collect();
        assert!(log_messages.iter().any(|msg| msg.contains("Starting logout process")), "没有找到登出开始消息");
        assert!(log_messages.iter().any(|msg| msg.contains("Failed to initialize")), "没有找到初始化失败消息");
    }

    #[tokio::test]
    async fn test_authenticator_initialization() {
        let network_monitor = Arc::new(NetworkMonitor::new());
        let mut ui = UI::new_empty(network_monitor);
        
        // 设置测试配置
        ui.config.username = "test_user".to_string();
        ui.config.password = "test_pass".to_string();
        ui.config.auth_url = "http://10.1.1.1".to_string();
        ui.config.isp = ISP::School;
        
        // 执行初始化
        let result = ui.init_authenticator().await;
        // 由于测试环境中没有 ChromeDriver，我们期望初始化失败
        assert!(!result, "在没有 ChromeDriver 的环境中，初始化应该失败");
        assert!(ui.authenticator.is_none(), "在初始化失败时，认证器应该为 None");
        
        // 验证日志消息
        assert!(ui.log_messages.iter().any(|msg| msg.contains("Failed to initialize")), 
            "应该记录初始化失败的日志消息");
    }
} 