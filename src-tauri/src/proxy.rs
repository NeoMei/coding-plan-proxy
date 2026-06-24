use std::process::{Child, Command};
use std::sync::Mutex;
use std::sync::Arc;
use std::net::TcpStream;
use std::time::Duration;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub struct ProxyManager {
    child: Mutex<Option<Child>>,
    port: u16,
}

impl ProxyManager {
    pub fn new(port: u16) -> Self {
        ProxyManager { child: Mutex::new(None), port }
    }

    /// Check if the proxy port is accepting connections (regardless of who started it)
    pub fn is_port_listening(&self) -> bool {
        TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", self.port).parse().unwrap(),
            Duration::from_millis(200),
        ).is_ok()
    }

    /// Check if our managed child process is alive, OR the port is already in use
    pub fn is_running(&self) -> bool {
        // First check our own child
        if let Ok(mut guard) = self.child.lock() {
            if let Some(ref mut child) = *guard {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        log::info!("Proxy exited with status: {:?}", status.code());
                        *guard = None;
                        // Fall through to port check
                    }
                    Ok(None) => return true,
                    Err(e) => {
                        log::warn!("Proxy wait error: {}", e);
                        *guard = None;
                    }
                }
            }
        }
        // If our child isn't running, check if another proxy is on the port
        self.is_port_listening()
    }

    pub fn port(&self) -> u16 { self.port }

    pub fn start(&self, proxy_path: &str) -> Result<(), String> {
        // If port is already in use, consider it "started"
        if self.is_port_listening() {
            log::info!("Proxy port {} already in use — reusing existing proxy", self.port);
            return Ok(());
        }

        if self.is_running() { return Ok(()); }

        log::info!("Starting proxy: node {}", proxy_path);

        let mut cmd = Command::new("node");
        cmd.arg(proxy_path)
            .env("PROXY_PORT", self.port.to_string());

        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000);
        }

        let child = cmd.spawn()
            .map_err(|e| format!("Failed to start proxy: {}", e))?;

        // Wait a moment for the proxy to bind the port
        std::thread::sleep(Duration::from_millis(500));

        if let Ok(mut guard) = self.child.lock() {
            *guard = Some(child);
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        // Kill our managed child
        if let Ok(mut guard) = self.child.lock() {
            if let Some(ref mut child) = *guard {
                child.kill().ok();
                child.wait().ok();
            }
            *guard = None;
        }
        // Kill any process holding our port
        let port = self.port;
        std::thread::spawn(move || {
            let _ = std::process::Command::new("cmd")
                .args(["/c", &format!("for /f \"tokens=5\" %a in ('netstat -ano ^| findstr :{port}') do taskkill /F /PID %a >nul 2>&1")])
                .creation_flags(0x08000000)
                .spawn();
        });
        Ok(())
    }
}

pub type SharedProxyManager = Arc<ProxyManager>;
