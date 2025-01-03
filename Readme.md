# Campus Network Assistant

校园网认证助手是一个用 Rust 编写的跨平台桌面应用程序，旨在简化校园网的登录和管理过程。

## 项目目的

1. 自动化校园网认证过程
2. 提供网络状态监控
3. 断网自动重连
4. 提供友好的图形界面
5. 支持多运营商切换

## 项目结构

```
src/
├── main.rs              # 程序入口
├── frontend/           
│   └── ui.rs           # 图形界面实现
└── backend/
    ├── authentication.rs # 认证模块
    ├── config.rs        # 配置管理
    ├── network_monitor.rs # 网络监控
    ├── logger.rs        # 日志系统
    └── downloader.rs    # Chrome下载器
```

## 模块功能说明

### 1. 主程序 (main.rs)
- 程序入口点配置
- 初始化各个模块
- 启动UI界面

### 2. 前端界面 (frontend/ui.rs)
- UI 结构体：管理界面状态和组件
- 主要功能：
  - `new()`: 创建新的 UI 实例
  - `run()`: 运行界面主循环
  - `perform_login()`: 执行登录操作
  - `perform_logout()`: 执行登出操作
  - `start_auto_login()`: 启动自动登录
  - `update_network_status()`: 更新网络状态显示
  - `add_log()`: 添加日志记录
  - `save_config()`: 保存配置信息

### 3. 认证模块 (backend/authentication.rs)
- 认证器结构体：管理认证状态和操作
- 主要功能：
  - `init()`: 初始化认证器
  - `login()`: 执行登录流程
  - `logout()`: 执行登出流程
  - `create_webdriver()`: 创建浏览器驱动
  - `start_chromedriver()`: 启动 ChromeDriver
  - `quit()`: 清理资源

### 4. 配置管理 (backend/config.rs)
- 配置结构体：存储用户配置
- 主要功能：
  - `load()`: 加载配置文件
  - `save()`: 保存配置到文件
  - `default()`: 创建默认配置

### 5. 网络监控 (backend/network_monitor.rs)
- 网络监控器：监控网络状态
- 主要功能：
  - `check_connection()`: 检查网络连接
  - `is_connected()`: 获取当前连接状态
  - `ping()`: 执行网络测试

### 6. 日志系统 (backend/logger.rs)
- 日志管理器：处理日志记录和输出
- 主要功能：
  - `init()`: 初始化日志系统
  - `get_log_file()`: 获取日志文件
  - 按月自动分割日志文件
  - 同时输出到控制台和文件
  - 支持彩色日志输出
  - 自动管理日志文件的创建和追加
  - 提供完整的单元测试

### 7. 下载器 (backend/downloader.rs)
- Chrome 和 ChromeDriver 下载管理
- 主要功能：
  - `ensure_chrome_and_driver_async()`: 确保必要组件存在
  - `download_and_install_chrome_async()`: 下载安装 Chrome
  - `download_and_install_chromedriver_async()`: 下载安装 ChromeDriver

## 日志系统特性

1. 日志分类管理
   - 按月自动创建新的日志文件
   - 自动在同一文件中追加日志
   - 清晰的日志分隔标记

2. 输出格式
   - 时间戳: [YYYY-MM-DD HH:mm:ss]
   - 日志级别: 使用不同颜色区分
   - 模块名称: 显示日志来源
   - 详细信息: 具体的日志内容

3. 日志级别
   - ERROR: 红色显示
   - WARN: 黄色显示
   - INFO: 绿色显示
   - DEBUG: 蓝色显示
   - TRACE: 青色显示

4. 输出目标
   - 控制台: 彩色输出
   - 文件: 纯文本格式
   - 支持同时输出到多个目标

## 待改进事项

1. 功能改进
   - [ ] 添加多账号管理功能
   - [ ] 支持自定义认证页面模板
   - [ ] 添加网络质量监测
   - [ ] 实现配置导入导出功能
   - [ ] 添加系统托盘功能

2. 性能优化
   - [ ] 优化 Chrome 启动速度
   - [ ] 减少内存占用
   - [ ] 改进网络检测机制

3. 用户体验
   - [ ] 添加深色模式
   - [ ] 支持快捷键操作
   - [ ] 添加操作引导
   - [ ] 优化错误提示

4. 安全性
   - [ ] 添加配置文件加密
   - [ ] 实现密码安全存储
   - [ ] 添加日志脱敏功能

5. 日志系统改进
   - [ ] 添加日志压缩功能
   - [ ] 实现日志轮转策略
   - [ ] 添加日志过滤功能
   - [ ] 支持自定义日志格式
   - [ ] 添加日志查看器

6. 其他
   - [ ] 添加自动更新功能
   - [ ] 完善单元测试
   - [ ] 添加 CI/CD 支持
   - [ ] 支持更多认证方式

## 贡献指南

欢迎提交 Issue 和 Pull Request 来帮助改进项目。在提交代码前，请确保：

1. 代码符合项目的编码规范
2. 添加了必要的测试
3. 更新了相关文档
4. 提交信息清晰明了

## 许可证

本项目采用 MIT 许可证。详见 [LICENSE](LICENSE) 文件。

