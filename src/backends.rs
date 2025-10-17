use anyhow::Context;
use std::path::Path;
use async_trait::async_trait;
use tokio::process::Command;

/// Backend trait: run a command and return (stdout, stderr, exit_status)
#[async_trait]
pub trait Backend: Send + Sync {
    async fn run(&self, cmd: &str, cwd: &Path, timeout_secs: Option<u64>) -> anyhow::Result<(String, String, std::process::ExitStatus)>;
}

/// Local backend: runs in host shell (PowerShell on Windows, sh on Unix)
pub struct LocalBackend;

impl LocalBackend {
    pub fn new() -> Self { Self {} }
}

#[async_trait]
impl Backend for LocalBackend {
    async fn run(&self, cmd: &str, cwd: &Path, _timeout_secs: Option<u64>) -> anyhow::Result<(String, String, std::process::ExitStatus)> {
        if cfg!(windows) {
            let mut c = Command::new("powershell.exe");
            c.arg("-NoLogo").arg("-NoProfile").arg("-Command").arg(cmd).current_dir(cwd);
            let output = c.output().await.context("local backend failed")?;
            let out = String::from_utf8_lossy(&output.stdout).to_string();
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            Ok((out, err, output.status))
        } else {
            let mut c = Command::new("sh");
            c.arg("-c").arg(cmd).current_dir(cwd);
            let output = c.output().await.context("local backend failed")?;
            let out = String::from_utf8_lossy(&output.stdout).to_string();
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            Ok((out, err, output.status))
        }
    }
}
