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

    pub fn is_port_listening(&self) -> bool {
        TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", self.port).parse().unwrap(),
            Duration::from_millis(200),
        ).is_ok()
    }

    pub fn is_running(&self) -> bool {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(ref mut child) = *guard {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        log::info!("Proxy exited: {:?}", status.code());
                        *guard = None;
                    }
                    Ok(None) => return true,
                    Err(_) => { *guard = None; }
                }
            }
        }
        self.is_port_listening()
    }

    pub fn port(&self) -> u16 { self.port }

    pub fn start(&self, proxy_path: &str) -> Result<(), String> {
        if self.is_port_listening() {
            log::info!("Proxy port {} already in use", self.port);
            return Ok(());
        }
        if self.is_running() { return Ok(()); }

        log::info!("Starting proxy: node {}", proxy_path);
        let mut cmd = Command::new("node");
        cmd.arg(proxy_path).env("PROXY_PORT", self.port.to_string());
        #[cfg(windows)] { cmd.creation_flags(0x08000000); }

        let child = cmd.spawn().map_err(|e| format!("Failed to start proxy: {}", e))?;
        std::thread::sleep(Duration::from_millis(500));
        if let Ok(mut guard) = self.child.lock() { *guard = Some(child); }
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        // Kill our child
        if let Ok(mut guard) = self.child.lock() {
            if let Some(ref mut child) = *guard {
                child.kill().ok();
                child.wait().ok();
            }
            *guard = None;
        }
        // Kill anything on the port — try tskill (no admin needed on Windows)
        let port = self.port;
        #[cfg(windows)]
        {
            // tskill by port: find PID from netstat then kill
            if let Ok(out) = std::process::Command::new("cmd")
                .args(["/c", &format!("for /f \"tokens=5\" %a in ('netstat -ano ^| findstr :{port} ^| findstr LISTENING') do taskkill /F /PID %a >nul 2>&1")])
                .creation_flags(0x08000000)
                .output()
            {
                log::info!("stop cmd output: {}", String::from_utf8_lossy(&out.stdout).trim());
            }
        }
        // Wait for port to free
        for _ in 0..5 {
            if !self.is_port_listening() { return Ok(()); }
            std::thread::sleep(Duration::from_millis(500));
        }
        if self.is_port_listening() {
            Err(format!("Port {} still in use after stop", self.port))
        } else {
            Ok(())
        }
    }
}

pub type SharedProxyManager = Arc<ProxyManager>;
