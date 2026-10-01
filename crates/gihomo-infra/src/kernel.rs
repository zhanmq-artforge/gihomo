use crate::error::InfraError;
use crate::storage::StorageManager;
use gihomo_core::KernelStatus;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tokio::process::Command;
use tracing::{debug, info, warn};

pub struct KernelManager;

impl KernelManager {
    /// Search for the Mihomo kernel executable in predefined directories and PATH
    pub fn find_kernel_binary(storage: &StorageManager) -> Option<PathBuf> {
        let candidates = [
            PathBuf::from("/usr/lib/gihomo/bin/mihomo"),
            PathBuf::from("/usr/lib/gihomo/mihomo"),
            storage.default_kernel_path(),
            PathBuf::from("/usr/local/bin/mihomo"),
            PathBuf::from("/usr/bin/mihomo"),
        ];

        for path in &candidates {
            if path.is_file() {
                debug!("Found Mihomo kernel at {:?}", path);
                return Some(path.clone());
            }
        }

        // Try discovering via PATH
        if let Some(paths) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&paths) {
                let candidate = dir.join("mihomo");
                if candidate.is_file() {
                    debug!("Found Mihomo kernel in PATH at {:?}", candidate);
                    return Some(candidate);
                }
            }
        }

        None
    }

    /// Check if the kernel binary has CAP_NET_ADMIN and CAP_NET_BIND_SERVICE capabilities
    pub async fn check_tun_capabilities(kernel_path: &Path) -> bool {
        let output = match Command::new("getcap").arg(kernel_path).output().await {
            Ok(out) => out,
            Err(_) => return false,
        };

        if !output.status.success() {
            return false;
        }

        let cap_str = String::from_utf8_lossy(&output.stdout);
        cap_str.contains("cap_net_admin")
    }

    /// Request TUN capabilities via Polkit (pkexec setcap and polkit rules setup)
    pub async fn request_tun_permissions(kernel_path: &Path) -> Result<(), InfraError> {
        let path_str = kernel_path.to_str().unwrap_or("");
        info!("Requesting TUN capabilities for {:?}", kernel_path);

        let script = format!(
            r#"setcap cap_net_admin,cap_net_bind_service=+ep "{path_str}" && \
if [ ! -f /usr/share/polkit-1/rules.d/art.artforge.Gihomo.rules ] && [ ! -f /etc/polkit-1/rules.d/10-gihomo.rules ]; then
    mkdir -p /etc/polkit-1/rules.d
    cat << 'RULE_EOF' > /etc/polkit-1/rules.d/10-gihomo.rules
polkit.addRule(function(action, subject) {{
    if ((action.id == "org.freedesktop.resolve1.set-dns-servers" ||
         action.id == "org.freedesktop.resolve1.set-domains" ||
         action.id == "org.freedesktop.resolve1.set-default-route" ||
         action.id == "org.freedesktop.resolve1.set-dns-over-tls" ||
         action.id == "org.freedesktop.resolve1.set-dnssec" ||
         action.id == "org.freedesktop.resolve1.revert") &&
        subject.active == true && subject.local == true &&
        (subject.isInGroup("sudo") || subject.isInGroup("wheel") || subject.isInGroup("netdev") || subject.isInGroup("admin"))) {{
        return polkit.Result.YES;
    }}
}});
RULE_EOF
    chmod 644 /etc/polkit-1/rules.d/10-gihomo.rules
fi"#
        );

        let output = Command::new("pkexec")
            .args(["sh", "-c", &script])
            .output()
            .await
            .map_err(|e| InfraError::Kernel(format!("执行 pkexec 提权失败: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(InfraError::Kernel(format!("授权失败: {}", stderr.trim())));
        }

        Ok(())
    }

    /// Read kernel version string
    pub async fn get_kernel_version(kernel_path: &Path) -> Option<String> {
        let output = Command::new(kernel_path).arg("-v").output().await.ok()?;
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout);
            Some(version.lines().next().unwrap_or("").trim().to_string())
        } else {
            None
        }
    }

    /// Fast health probe to verify if Mihomo external controller is responsive
    pub async fn is_running_via_api(controller_port: u16, secret: &str) -> bool {
        let url = format!("http://127.0.0.1:{}/version", controller_port);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build();

        if let Ok(client) = client {
            let mut req = client.get(&url);
            if !secret.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", secret));
            }
            if let Ok(resp) = req.send().await {
                return resp.status().is_success();
            }
        }
        false
    }

    /// Inspect current kernel status (Running, Stopped, NotFound, Error)
    pub async fn check_status(
        storage: &StorageManager,
        controller_port: u16,
        secret: &str,
    ) -> Result<KernelStatus, InfraError> {
        let kernel_bin = Self::find_kernel_binary(storage);
        if kernel_bin.is_none() {
            return Ok(KernelStatus::NotFound);
        }

        // 1. If API is responding, kernel is definitively running
        if Self::is_running_via_api(controller_port, secret).await {
            return Ok(KernelStatus::Running);
        }

        // 2. Check PID file if API is not yet ready or dead
        let pid_path = storage.pid_file_path();
        if pid_path.exists() {
            if let Ok(content) = fs::read_to_string(&pid_path).await {
                if let Ok(pid) = content.trim().parse::<u32>() {
                    let proc_path = format!("/proc/{}", pid);
                    if Path::new(&proc_path).exists() {
                        // Process exists in procfs
                        return Ok(KernelStatus::Running);
                    } else {
                        // Stale PID file
                        let _ = fs::remove_file(pid_path).await;
                    }
                }
            }
        }

        Ok(KernelStatus::Stopped)
    }

    /// Launch the native Mihomo kernel process
    pub async fn start(
        storage: &StorageManager,
        controller_port: u16,
        secret: &str,
    ) -> Result<(), InfraError> {
        let kernel_bin = Self::find_kernel_binary(storage).ok_or_else(|| {
            InfraError::Kernel(
                "未找到 Mihomo 内核文件，请将其放置于 bin 目录或安装到系统".to_string(),
            )
        })?;

        if Self::is_running_via_api(controller_port, secret).await {
            info!("Mihomo kernel is already running");
            return Ok(());
        }

        info!("Starting native Mihomo kernel from {:?}", kernel_bin);

        // Ensure active config exists and always enforce local-only controller access.
        let config_path = storage.active_config_path();
        let current_config = if config_path.exists() {
            fs::read_to_string(&config_path).await?
        } else {
            warn!(
                "Active config does not exist at {:?}, generating base config",
                config_path
            );
            gihomo_core::generate_base_config(7890, controller_port, secret, false)
        };
        let secured_config =
            gihomo_core::secure_controller_config(&current_config, controller_port, secret)?;
        storage.write_active_config(&secured_config).await?;

        // Prepare log redirection
        let log_path = storage.kernel_log_path();
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| InfraError::Kernel(format!("无法打开日志文件 {:?}: {}", log_path, e)))?;
        let err_file = log_file
            .try_clone()
            .map_err(|e| InfraError::Kernel(format!("无法克隆日志文件句柄: {}", e)))?;

        let mut cmd = Command::new(&kernel_bin);
        cmd.arg("-d")
            .arg(storage.mihomo_dir())
            .stdout(std::process::Stdio::from(log_file))
            .stderr(std::process::Stdio::from(err_file));

        #[cfg(target_os = "linux")]
        unsafe {
            cmd.pre_exec(|| {
                // SAFETY: The child hook only makes the async-signal-safe prctl syscall and accesses no shared state.
                if libc::prctl(
                    libc::PR_SET_PDEATHSIG,
                    libc::SIGTERM as libc::c_ulong,
                    0,
                    0,
                    0,
                ) == -1
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let child = cmd
            .spawn()
            .map_err(|e| InfraError::Kernel(format!("启动内核失败: {}", e)))?;

        if let Some(pid) = child.id() {
            info!("Mihomo kernel spawned with PID: {}", pid);
            let _ = fs::write(storage.pid_file_path(), pid.to_string()).await;
        }

        // Wait up to 3 seconds for controller to become ready
        for _ in 0..15 {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if Self::is_running_via_api(controller_port, secret).await {
                info!("Mihomo external controller is online");
                return Ok(());
            }
        }

        // If not ready, check if process died
        let pid_path = storage.pid_file_path();
        if pid_path.exists() {
            if let Ok(content) = fs::read_to_string(&pid_path).await {
                if let Ok(pid) = content.trim().parse::<u32>() {
                    let proc_path = format!("/proc/{}", pid);
                    if !Path::new(&proc_path).exists() {
                        let _ = fs::remove_file(&pid_path).await;
                        let last_lines = Self::tail_log_file(&log_path, 5).await;
                        return Err(InfraError::Kernel(format!(
                            "内核启动后异常退出，日志末尾: {}",
                            last_lines
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Terminate running Mihomo process
    pub async fn stop(storage: &StorageManager) -> Result<(), InfraError> {
        info!("Stopping Mihomo kernel");
        let pid_path = storage.pid_file_path();

        if pid_path.exists() {
            if let Ok(content) = fs::read_to_string(&pid_path).await {
                if let Ok(pid) = content.trim().parse::<u32>() {
                    #[cfg(unix)]
                    unsafe {
                        // SAFETY: Sending SIGTERM signal to specific known child PID
                        libc::kill(pid as libc::pid_t, libc::SIGTERM);
                    }

                    // Wait up to 2 seconds for graceful exit
                    for _ in 0..10 {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        let proc_path = format!("/proc/{}", pid);
                        if !Path::new(&proc_path).exists() {
                            break;
                        }
                    }

                    // Force kill if still lingering
                    let proc_path = format!("/proc/{}", pid);
                    if Path::new(&proc_path).exists() {
                        #[cfg(unix)]
                        unsafe {
                            // SAFETY: Sending SIGKILL signal to specific known child PID
                            libc::kill(pid as libc::pid_t, libc::SIGKILL);
                        }
                    }
                }
            }
            let _ = fs::remove_file(&pid_path).await;
        }

        Ok(())
    }

    /// Restart the kernel
    pub async fn restart(
        storage: &StorageManager,
        controller_port: u16,
        secret: &str,
    ) -> Result<(), InfraError> {
        info!("Restarting Mihomo kernel");
        let _ = Self::stop(storage).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Self::start(storage, controller_port, secret).await
    }

    async fn tail_log_file(log_path: &Path, lines: usize) -> String {
        if let Ok(content) = fs::read_to_string(log_path).await {
            let all_lines: Vec<&str> = content.lines().collect();
            let start = all_lines.len().saturating_sub(lines);
            all_lines[start..].join("\n")
        } else {
            "无法读取日志".to_string()
        }
    }
}
