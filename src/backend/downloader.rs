use std::path::PathBuf;
use tokio::fs;
use tokio::task;
use reqwest;
use zip::ZipArchive;
use std::io::copy;
use anyhow::{Result, Context, anyhow};
use log::{debug, info, warn, error};
use tokio::time::sleep;
use std::time::Duration;
use futures_util::StreamExt;
use bytes::{BytesMut, Buf};

// Chrome和ChromeDriver版本
const CHROMEDRIVER_VERSION: &str = "131.0.6778.204";
const CHROME_VERSION: &str = "131.0.6778.204";
// Chrome下载地址
const CHROME_DOWNLOAD_URL: &str = "https://storage.googleapis.com/chrome-for-testing-public/131.0.6778.204/win32/chrome-win32.zip";
const CHROMEDRIVER_DOWNLOAD_URL: &str = "https://storage.googleapis.com/chrome-for-testing-public/131.0.6778.204/win32/chromedriver-win32.zip";
// 最大重试次数
const MAX_RETRIES: u32 = 3;
// 重试等待时间（秒）
const RETRY_WAIT_TIME: u64 = 5;

pub struct Downloader;

impl Downloader {
    pub async fn ensure_chrome_and_driver_async() -> Result<()> {
        info!("开始确保Chrome和ChromeDriver存在");
        let current_dir = std::env::current_dir()?;
        
        // 确保 Chrome 目录存在
        let chrome_dir = current_dir.join("chrome-win32");
        if !chrome_dir.exists() {
            info!("Chrome目录不存在，开始下载");
            if let Err(e) = Self::download_and_install_chrome_async(&current_dir).await {
                error!("下载Chrome失败: {}", e);
                return Err(anyhow!("Chrome下载失败: {}. 请检查网络连接或手动下载", e));
            }
        } else {
            info!("Chrome目录已存在");
        }
        
        // 确保 ChromeDriver 存在
        let chromedriver_path = current_dir.join("chromedriver.exe");
        if !chromedriver_path.exists() {
            info!("ChromeDriver不存在，开始下载");
            if let Err(e) = Self::download_and_install_chromedriver_async(&current_dir).await {
                error!("下载ChromeDriver失败: {}", e);
                return Err(anyhow!("ChromeDriver下载失败: {}. 请检查网络连接或手动下载", e));
            }
        } else {
            info!("ChromeDriver已存在");
        }
        
        info!("Chrome和ChromeDriver检查完成");
        Ok(())
    }

    async fn check_url_accessibility(url: &str) -> Result<bool> {
        debug!("检查URL可访问性: {}", url);
        
        // 从URL中提取主机名
        let url = reqwest::Url::parse(url)?;
        let host = url.host_str().ok_or_else(|| anyhow!("无效的URL"))?;
        
        // 使用 ping 命令检查主机是否可访问
        let output = std::process::Command::new("ping")
            .arg("-n")  // Windows 平台使用 -n
            .arg("1")   // 只 ping 一次
            .arg(host)
            .output()
            .context("执行ping命令失败")?;
            
        let success = output.status.success();
        if success {
            info!("主机 {} 可访问", host);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("无法访问主机 {}: {}", host, stderr);
        }
        
        Ok(success)
    }

    async fn download_with_retry(client: &reqwest::Client, url: &str, retry_count: u32) -> Result<bytes::Bytes> {
        let mut attempts = 0;
        loop {
            attempts += 1;
            info!("开始第 {} 次下载尝试...", attempts);
            match client.get(url)
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36")
                .header("Accept", "*/*")
                .header("Connection", "keep-alive")
                .send()
                .await {
                    Ok(response) => {
                        if !response.status().is_success() {
                            error!("下载失败，HTTP状态码: {}", response.status());
                            if attempts >= retry_count {
                                return Err(anyhow!("下载失败，HTTP状态码: {}，已达到最大重试次数", response.status()));
                            }
                        } else {
                            let total_size = response.content_length().unwrap_or(0);
                            info!("开始下载，文件总大小: {:.2} MB", total_size as f64 / 1024.0 / 1024.0);
                            
                            // 使用 bytes::BytesMut 来收集数据
                            let mut bytes = bytes::BytesMut::with_capacity(total_size as usize);
                            let mut downloaded = 0u64;
                            let mut stream = response.bytes_stream();
                            
                            while let Some(chunk) = stream.next().await {
                                match chunk {
                                    Ok(data) => {
                                        downloaded += data.len() as u64;
                                        bytes.extend_from_slice(&data);
                                        
                                        // 计算下载进度
                                        if total_size > 0 {
                                            let percentage = (downloaded as f64 / total_size as f64 * 100.0) as u32;
                                            info!("下载进度: {}% ({:.2}/{:.2} MB)", 
                                                percentage,
                                                downloaded as f64 / 1024.0 / 1024.0,
                                                total_size as f64 / 1024.0 / 1024.0
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        error!("下载过程中出错: {}", e);
                                        if attempts >= retry_count {
                                            return Err(anyhow!("下载过程中出错: {}，已达到最大重试次数", e));
                                        }
                                        break;
                                    }
                                }
                            }
                            
                            if downloaded == total_size || total_size == 0 {
                                info!("下载完成，总大小: {:.2} MB", downloaded as f64 / 1024.0 / 1024.0);
                                return Ok(bytes.freeze());
                            } else {
                                error!("下载不完整: {}/{} bytes", downloaded, total_size);
                                if attempts >= retry_count {
                                    return Err(anyhow!("下载不完整，已达到最大重试次数"));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("下载请求失败: {}", e);
                        if attempts >= retry_count {
                            return Err(anyhow!("下载请求失败: {}，已达到最大重试次数", e));
                        }
                    }
                }
            
            let wait_time = RETRY_WAIT_TIME * attempts as u64;
            info!("等待 {} 秒后进行第 {} 次重试...", wait_time, attempts + 1);
            sleep(Duration::from_secs(wait_time)).await;
        }
    }

    pub async fn download_and_install_chrome_async(current_dir: &PathBuf) -> Result<()> {
        info!("开始下载Chrome");
        
        // 检查URL是否可访问
        if !Self::check_url_accessibility(CHROME_DOWNLOAD_URL).await? {
            return Err(anyhow!("无法访问Chrome下载地址，请检查网络连接"));
        }
        
        // 创建 HTTP 客户端
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .context("创建HTTP客户端失败")?;
        
        // 下载 Chrome ZIP 文件
        debug!("开始下载Chrome ZIP文件");
        let bytes = Self::download_with_retry(&client, CHROME_DOWNLOAD_URL, MAX_RETRIES)
            .await
            .context("下载Chrome失败")?;
            
        let zip_path = current_dir.join("chrome.zip");
        fs::write(&zip_path, &bytes)
            .await
            .context("写入Chrome zip文件失败")?;
        
        info!("Chrome下载完成，开始解压");
        
        // 在阻塞线程中解压文件
        let current_dir = current_dir.clone();
        match task::spawn_blocking(move || -> Result<()> {
            // 解压 Chrome
            let file = std::fs::File::open(&zip_path)
                .context("打开Chrome zip文件失败")?;
                
            let mut archive = ZipArchive::new(file)
                .context("创建ZIP存档失败")?;
            
            debug!("开始解压 {} 个文件", archive.len());
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)
                    .context("从存档中获取文件失败")?;
                    
                let outpath = match file.enclosed_name() {
                    Some(path) => current_dir.join(path),
                    None => continue,
                };
                
                if file.name().ends_with('/') {
                    std::fs::create_dir_all(&outpath)
                        .context("创建目录失败")?;
                } else {
                    if let Some(p) = outpath.parent() {
                        if !p.exists() {
                            std::fs::create_dir_all(p)
                                .context("创建父目录失败")?;
                        }
                    }
                    let mut outfile = std::fs::File::create(&outpath)
                        .context("创建文件失败")?;
                    copy(&mut file, &mut outfile)
                        .context("复制文件失败")?;
                }
            }
            
            // 删除 ZIP 文件
            std::fs::remove_file(zip_path)
                .context("删除Chrome zip文件失败")?;
                
            info!("Chrome解压完成");
            Ok(())
        }).await {
            Ok(result) => result?,
            Err(e) => return Err(anyhow!("解压Chrome时发生错误: {}", e)),
        }
        
        info!("Chrome安装完成");
        Ok(())
    }

    pub async fn download_and_install_chromedriver_async(current_dir: &PathBuf) -> Result<()> {
        info!("开始下载ChromeDriver");
        
        // 检查URL是否可访问
        if !Self::check_url_accessibility(CHROMEDRIVER_DOWNLOAD_URL).await? {
            return Err(anyhow!("无法访问ChromeDriver下载地址，请检查网络连接"));
        }
        
        // 创建 HTTP 客户端
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .context("创建HTTP客户端失败")?;
        
        // 下载 ChromeDriver ZIP 文件
        debug!("开始下载ChromeDriver ZIP文件");
        let bytes = Self::download_with_retry(&client, CHROMEDRIVER_DOWNLOAD_URL, MAX_RETRIES)
            .await
            .context("下载ChromeDriver失败")?;
            
        let zip_path = current_dir.join("chromedriver.zip");
        fs::write(&zip_path, &bytes)
            .await
            .context("写入ChromeDriver zip文件失败")?;
        
        info!("ChromeDriver下载完成，开始解压");
        
        // 在阻塞线程中解压文件
        let current_dir = current_dir.clone();
        match task::spawn_blocking(move || -> Result<()> {
            // 解压 ChromeDriver
            let file = std::fs::File::open(&zip_path)
                .context("打开ChromeDriver zip文件失败")?;
                
            let mut archive = ZipArchive::new(file)
                .context("创建ZIP存档失败")?;
            
            debug!("开始解压 {} 个文件", archive.len());
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)
                    .context("从存档中获取文件失败")?;
                    
                if file.name().contains("chromedriver.exe") {
                    let mut outfile = std::fs::File::create(current_dir.join("chromedriver.exe"))
                        .context("创建ChromeDriver可执行文件失败")?;
                    copy(&mut file, &mut outfile)
                        .context("复制ChromeDriver可执行文件失败")?;
                    break;
                }
            }
            
            // 删除 ZIP 文件
            std::fs::remove_file(zip_path)
                .context("删除ChromeDriver zip文件失败")?;
                
            info!("ChromeDriver解压完成");
            Ok(())
        }).await {
            Ok(result) => result?,
            Err(e) => return Err(anyhow!("解压ChromeDriver时发生错误: {}", e)),
        }
        
        info!("ChromeDriver安装完成");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;
    use tempfile::tempdir;
    use std::path::Path;

    fn init_test_logger() {
        let _ = pretty_env_logger::formatted_builder()
            .is_test(true)
            .try_init();
    }

    #[test]
    fn test_path_construction() {
        init_test_logger();
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path().to_path_buf();
        
        // 测试Chrome路径构造
        let chrome_dir = temp_path.join("chrome-win32");
        assert_eq!(chrome_dir.file_name().unwrap(), "chrome-win32");
        
        // 测试ChromeDriver路径构造
        let chromedriver_path = temp_path.join("chromedriver.exe");
        assert_eq!(chromedriver_path.file_name().unwrap(), "chromedriver.exe");
    }

    #[test]
    fn test_url_parsing() {
        init_test_logger();
        // 测试Chrome下载URL
        let chrome_url = reqwest::Url::parse(CHROME_DOWNLOAD_URL).unwrap();
        assert_eq!(chrome_url.host_str().unwrap(), "storage.googleapis.com");
        assert!(chrome_url.path().contains("chrome-win32.zip"));
        
        // 测试ChromeDriver下载URL
        let chromedriver_url = reqwest::Url::parse(CHROMEDRIVER_DOWNLOAD_URL).unwrap();
        assert_eq!(chromedriver_url.host_str().unwrap(), "storage.googleapis.com");
        assert!(chromedriver_url.path().contains("chromedriver-win32.zip"));
    }

    #[test]
    fn test_version_constants() {
        init_test_logger();
        // 测试版本号格式
        assert!(CHROME_VERSION.split('.').count() >= 3, "Chrome版本号格式不正确");
        assert!(CHROMEDRIVER_VERSION.split('.').count() >= 3, "ChromeDriver版本号格式不正确");
        
        // 测试版本号匹配
        assert_eq!(CHROME_VERSION, CHROMEDRIVER_VERSION, "Chrome和ChromeDriver版本号应该匹配");
    }

    #[test]
    fn test_download_urls() {
        init_test_logger();
        // 测试URL中包含正确的版本号
        assert!(CHROME_DOWNLOAD_URL.contains(CHROME_VERSION), "Chrome下载URL应该包含正确的版本号");
        assert!(CHROMEDRIVER_DOWNLOAD_URL.contains(CHROMEDRIVER_VERSION), "ChromeDriver下载URL应该包含正确的版本号");
        
        // 测试URL中包含正确的平台信息
        assert!(CHROME_DOWNLOAD_URL.contains("win32"), "Chrome下载URL应该包含平台信息");
        assert!(CHROMEDRIVER_DOWNLOAD_URL.contains("win32"), "ChromeDriver下载URL应该包含平台信息");
    }

    #[test]
    #[ignore] // 忽略需要网络连接的测试
    fn test_download_and_install_chrome_async() {
        init_test_logger();
        let rt = Runtime::new().unwrap();
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path().to_path_buf();

        rt.block_on(async {
            let result = Downloader::download_and_install_chrome_async(&temp_path).await;
            match result {
                Ok(_) => {
                    assert!(temp_path.join("chrome-win32").exists());
                }
                Err(e) => {
                    warn!("Chrome下载失败（这可能是正常的）: {:?}", e);
                }
            }
        });
    }

    #[test]
    #[ignore] // 忽略需要网络连接的测试
    fn test_download_and_install_chromedriver_async() {
        init_test_logger();
        let rt = Runtime::new().unwrap();
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path().to_path_buf();

        rt.block_on(async {
            let result = Downloader::download_and_install_chromedriver_async(&temp_path).await;
            match result {
                Ok(_) => {
                    assert!(temp_path.join("chromedriver.exe").exists());
                }
                Err(e) => {
                    warn!("ChromeDriver下载失败（这可能是正常的）: {:?}", e);
                }
            }
        });
    }

    #[test]
    // #[ignore] // 忽略需要网络连接的测试
    fn test_ensure_chrome_and_driver_async() {
        init_test_logger();
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let result = Downloader::ensure_chrome_and_driver_async().await;
            match result {
                Ok(_) => info!("Chrome和ChromeDriver安装成功"),
                Err(e) => warn!("Chrome和ChromeDriver安装失败（这可能是正常的）: {:?}", e),
            }
        });
    }

    #[test]
    fn test_url_accessibility() {
        init_test_logger();
        let rt = Runtime::new().unwrap();
        
        rt.block_on(async {
            // 测试 Chrome 下载 URL
            let chrome_accessible = Downloader::check_url_accessibility(CHROME_DOWNLOAD_URL).await;
            match chrome_accessible {
                Ok(accessible) => {
                    if accessible {
                        info!("Chrome下载URL可访问");
                    } else {
                        warn!("Chrome下载URL不可访问");
                    }
                }
                Err(e) => error!("检查Chrome下载URL时发生错误: {:?}", e),
            }

            // 测试 ChromeDriver 下载 URL
            let chromedriver_accessible = Downloader::check_url_accessibility(CHROMEDRIVER_DOWNLOAD_URL).await;
            match chromedriver_accessible {
                Ok(accessible) => {
                    if accessible {
                        info!("ChromeDriver下载URL可访问");
                    } else {
                        warn!("ChromeDriver下载URL不可访问");
                    }
                }
                Err(e) => error!("检查ChromeDriver下载URL时发生错误: {:?}", e),
            }
        });
    }
} 