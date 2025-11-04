use anyhow::Context;
use std::path::Path;
use async_trait::async_trait;
use tokio::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
    async fn run(&self, cmd: &str, cwd: &Path, timeout_secs: Option<u64>) -> anyhow::Result<(String, String, std::process::ExitStatus)> {
        if cfg!(windows) {
            let mut c = Command::new("powershell.exe");
            c.arg("-NoLogo").arg("-NoProfile").arg("-Command").arg(cmd).current_dir(cwd);

            if let Some(secs) = timeout_secs {
                let mut child = c
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .context("local backend failed to spawn process")?;
                match tokio::time::timeout(std::time::Duration::from_secs(secs), child.wait_with_output()).await {
                    Ok(output_res) => {
                        let output = output_res.context("waiting for child failed")?;
                        let out = String::from_utf8_lossy(&output.stdout).to_string();
                        let err = String::from_utf8_lossy(&output.stderr).to_string();
                        return Ok((out, err, output.status));
                    }
                    Err(_) => {
                        let _ = child.kill();
                        let _ = child.wait().await;
                        return Err(anyhow::anyhow!("local backend timed out after {}s", secs));
                    }
                }
            } else {
                let output = c.output().await.context("local backend failed")?;
                let out = String::from_utf8_lossy(&output.stdout).to_string();
                let err = String::from_utf8_lossy(&output.stderr).to_string();
                Ok((out, err, output.status))
            }
        } else {
            let mut c = Command::new("sh");
            c.arg("-c").arg(cmd).current_dir(cwd);

            if let Some(secs) = timeout_secs {
                let mut child = c
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .context("local backend failed to spawn process")?;
                match tokio::time::timeout(std::time::Duration::from_secs(secs), child.wait_with_output()).await {
                    Ok(output_res) => {
                        let output = output_res.context("waiting for child failed")?;
                        let out = String::from_utf8_lossy(&output.stdout).to_string();
                        let err = String::from_utf8_lossy(&output.stderr).to_string();
                        return Ok((out, err, output.status));
                    }
                    Err(_) => {
                        let _ = child.kill();
                        let _ = child.wait().await;
                        return Err(anyhow::anyhow!("local backend timed out after {}s", secs));
                    }
                }
            } else {
                let output = c.output().await.context("local backend failed")?;
                let out = String::from_utf8_lossy(&output.stdout).to_string();
                let err = String::from_utf8_lossy(&output.stderr).to_string();
                Ok((out, err, output.status))
            }
        }
    }
}
/// Docker backend: runs the given command inside a Docker container using `docker run`.
/// - mounts the provided `cwd` into the container at `/workdir`
/// - sets the container working directory to `/workdir`
/// - runs `sh -c "<cmd>"` inside the container (image must provide `sh`)
/// Note: path handling for Windows host -> Docker mounts may need adjustment depending on the
/// user's Docker setup (Docker Desktop vs. other runtimes).
pub struct DockerBackend {
    image: String,
    /// Optional extra args passed to `docker run` (e.g. ["--network", "host"])
    extra_args: Vec<String>,
}

impl DockerBackend {
    /// Create a new DockerBackend for `image`.
    pub fn new(image: impl Into<String>) -> Self {
        Self {
            image: image.into(),
            extra_args: Vec::new(),
        }
    }

    /// Add extra args to the docker run invocation.
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }
}

#[async_trait]
impl Backend for DockerBackend {
    async fn run(
        &self,
        cmd: &str,
        cwd: &Path,
        timeout_secs: Option<u64>,
    ) -> anyhow::Result<(String, String, std::process::ExitStatus)> {
        // Canonicalize the host path to produce an absolute path for the Docker mount.
        // If canonicalization fails, return an error early with context.
        let host_path = cwd
            .canonicalize()
            .with_context(|| format!("failed to canonicalize path {:?}", cwd))?;
        let mut host_path_str = host_path.to_string_lossy().to_string();

        // On Windows convert "C:\path" (or "\\?\\C:\path") into Docker-friendly "/c/path".
        // Also turn backslashes into forward slashes.
        #[cfg(windows)]
        {
            // Replace backslashes with forward slashes first.
            let mut s = host_path_str.replace('\\', "/");

            // Remove the extended path prefix if present (e.g. "\\?\" -> "//?/" after replace).
            if s.starts_with("//?/") {
                s = s.replacen("//?/", "", 1);
            } else if s.starts_with("/?/") {
                s = s.replacen("/?/", "", 1);
            }

            // If path starts with a drive letter like "C:/" convert to "/c/...".
            if s.len() >= 2 && s.as_bytes()[1] == b':' {
                if let Some(drive) = s.chars().next() {
                    let drive = drive.to_ascii_lowercase();
                    // skip the "X:" prefix
                    s = format!("/{}{}", drive, &s[2..]);
                }
            }
            host_path_str = s;
        }

        // Inside the container we mount the host dir at /workdir and use that as the working dir.
        let container_workdir = "/workdir";

        // Build base docker run command: docker run --rm -w /workdir -v <host_path>:/workdir <extra_args...> <image> sh -c "<cmd>"
        let mut c = Command::new("docker");
        c.arg("run").arg("--rm").arg("-w").arg(container_workdir);

        // Mount the current working directory into the container.
        c.arg("-v")
            .arg(format!("{}:{}", host_path_str, container_workdir));

        // Append any extra args the backend was created with.
        for a in &self.extra_args {
            c.arg(a);
        }

        // Image and command to run inside container.
        c.arg(&self.image)
            .arg("sh")
            .arg("-c")
            .arg(cmd);

        // If a timeout is requested, spawn and enforce it; otherwise wait for output directly.
        if let Some(secs) = timeout_secs {
            let mut child = c
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .context("docker backend failed to spawn process")?;

            match tokio::time::timeout(std::time::Duration::from_secs(secs), child.wait_with_output()).await {
                Ok(output_res) => {
                    let output = output_res.context("waiting for docker child failed")?;
                    let out = String::from_utf8_lossy(&output.stdout).to_string();
                    let err = String::from_utf8_lossy(&output.stderr).to_string();
                    Ok((out, err, output.status))
                }
                Err(_) => {
                    // Timed out: attempt to kill the container process.
                    let _ = child.kill();
                    let _ = child.wait().await;
                    Err(anyhow::anyhow!("docker backend timed out after {}s", secs))
                }
            }
        } else {
            let output = c.output().await.context("docker backend failed")?;
            let out = String::from_utf8_lossy(&output.stdout).to_string();
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            Ok((out, err, output.status))
        }
    }
}

/// SSH backend: runs commands on a remote host via the `ssh` binary.
///
/// This backend shells out to the platform `ssh` client instead of implementing an SSH client
/// itself. This keeps the implementation small and leverages existing, well-tested clients.
///
/// Notes:
/// - Uses `sh -lc "<cmd>"` on the remote side to allow arbitrary shell command strings.
/// - The caller can configure user, port, identity file and additional ssh args.
/// - Requires `ssh` to be available on the host where this program runs.
pub struct SSHBackend {
    host: String,
    user: Option<String>,
    port: Option<u16>,
    key_path: Option<String>,
    extra_args: Vec<String>,
}

impl SSHBackend {
    /// Create a new SSHBackend targeting `host` (IP or DNS name).
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            user: None,
            port: None,
            key_path: None,
            extra_args: Vec::new(),
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn with_key(mut self, key_path: impl Into<String>) -> Self {
        self.key_path = Some(key_path.into());
        self
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }
}

#[async_trait]
impl Backend for SSHBackend {
    async fn run(&self, cmd: &str, _cwd: &Path, timeout_secs: Option<u64>) -> anyhow::Result<(String, String, std::process::ExitStatus)> {
        // Build ssh target string: user@host or host
        let target = if let Some(u) = &self.user {
            format!("{}@{}", u, self.host)
        } else {
            self.host.clone()
        };

        // Build ssh invocation.
        // Use conservative safe defaults: non-interactive (BatchMode) and a connection timeout.
        let mut c = Command::new("ssh");
        if let Some(p) = self.port {
            c.arg("-p").arg(p.to_string());
        }
        if let Some(k) = &self.key_path {
            c.arg("-i").arg(k);
        }

        // Prevent ssh from prompting for passwords or host key verification in batch scenarios.
        // Note: StrictHostKeyChecking=accept-new can be used in some environments,
        // but it depends on OpenSSH version. We keep it simple and non-interactive.
        c.arg("-o").arg("BatchMode=yes");
        c.arg("-o").arg("ConnectTimeout=10");

        // Append any user-specified extra args (allows overriding / adding options).
        for a in &self.extra_args {
            c.arg(a);
        }

        // target and remote command.
        c.arg(target);
        // Execute via a POSIX shell on remote side to support complex command strings.
        c.arg("sh").arg("-lc").arg(cmd);

        // For SSH backend we don't change local cwd — remote cwd is controlled by ssh command / remote env.

        if let Some(secs) = timeout_secs {
            let mut child = c
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .context("ssh backend failed to spawn ssh process")?;

            match tokio::time::timeout(std::time::Duration::from_secs(secs), child.wait_with_output()).await {
                Ok(output_res) => {
                    let output = output_res.context("waiting for ssh child failed")?;
                    let out = String::from_utf8_lossy(&output.stdout).to_string();
                    let err = String::from_utf8_lossy(&output.stderr).to_string();
                    Ok((out, err, output.status))
                }
                Err(_) => {
                    // Try to kill the ssh client process. Remote command may still be running.
                    let _ = child.kill();
                    let _ = child.wait().await;
                    Err(anyhow::anyhow!("ssh backend timed out after {}s", secs))
                }
            }
        } else {
            let output = c.output().await.context("ssh backend failed")?;
            let out = String::from_utf8_lossy(&output.stdout).to_string();
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            Ok((out, err, output.status))
        }
    }
}

/// Kubernetes backend: runs workloads inside the cluster using the `kubectl` binary.
///
/// This implementation shells out to `kubectl` to keep the dependency surface small and to
/// leverage an already-configured kubeconfig or in-cluster configuration via the CLI.
/// It creates a short-lived Pod via `kubectl run --rm` and executes the provided command
/// in that ephemeral pod using the provided image. The pod name is generated to avoid collisions.
///
/// Requirements & notes:
/// - Requires `kubectl` to be available and configured (context/namespace) on the machine where
///   this program runs.
/// - This backend is intended for short-lived commands. For long-running or production workloads,
///   consider a more robust controller-based approach.
pub struct KubernetesBackend {
    image: String,
    namespace: Option<String>,
    /// Additional args passed to `kubectl run`, e.g. ["--serviceaccount=xxx"]
    extra_args: Vec<String>,
}

impl KubernetesBackend {
    /// Create a backend that will run commands inside a pod instantiated from `image`.
    pub fn new(image: impl Into<String>) -> Self {
        Self {
            image: image.into(),
            namespace: None,
            extra_args: Vec::new(),
        }
    }

    pub fn with_namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = Some(ns.into());
        self
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }
}

#[async_trait]
impl Backend for KubernetesBackend {
    async fn run(&self, cmd: &str, _cwd: &Path, timeout_secs: Option<u64>) -> anyhow::Result<(String, String, std::process::ExitStatus)> {
        // Generate a lightweight unique pod name based on epoch nanos.
        let pod_name = {
            let now = SystemTime::now().duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            format!("rustypipe-{}", now)
        };

        // Build kubectl invocation:
        // kubectl run <pod_name> --rm --restart=Never --image <image> [--namespace NAMESPACE] [extra_args...] -- sh -c "<cmd>"
        let mut c = Command::new("kubectl");
        c.arg("run");
        c.arg(&pod_name);
        c.arg("--rm"); // remove pod after completion
        c.arg("--restart=Never"); // run as a pod, not a controller
        c.arg("--image").arg(&self.image);

        if let Some(ns) = &self.namespace {
            c.arg("--namespace").arg(ns);
        }

        // Append extra args (user may include serviceaccount, env, etc).
        for a in &self.extra_args {
            c.arg(a);
        }

        // Ensure kubectl treats subsequent args as the container command.
        c.arg("--");
        // Use sh -c so that the provided cmd string is interpreted by a shell inside the pod.
        c.arg("sh").arg("-c").arg(cmd);

        if let Some(secs) = timeout_secs {
            let mut child = c
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .context("kubernetes backend failed to spawn kubectl process")?;

            match tokio::time::timeout(std::time::Duration::from_secs(secs), child.wait_with_output()).await {
                Ok(output_res) => {
                    let output = output_res.context("waiting for kubectl child failed")?;
                    let out = String::from_utf8_lossy(&output.stdout).to_string();
                    let err = String::from_utf8_lossy(&output.stderr).to_string();
                    Ok((out, err, output.status))
                }
                Err(_) => {
                    // Timeouts often leave the ephemeral pod running (kubectl may still be waiting).
                    // Attempt to kill the kubectl process, then try deleting the pod by name to avoid leakage.
                    let _ = child.kill();
                    let _ = child.wait().await;

                    // Best-effort cleanup: delete the created pod.
                    // We ignore errors here because the cluster state may have already removed the pod
                    // or the operation may not be permitted in the current context.
                    let mut cleanup = Command::new("kubectl");
                    cleanup.arg("delete").arg("pod").arg(&pod_name);
                    if let Some(ns) = &self.namespace {
                        cleanup.arg("--namespace").arg(ns);
                    }
                    let _ = cleanup.output().await;

                    Err(anyhow::anyhow!("kubernetes backend timed out after {}s", secs))
                }
            }
        } else {
            let output = c.output().await.context("kubernetes backend failed")?;
            let out = String::from_utf8_lossy(&output.stdout).to_string();
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            Ok((out, err, output.status))
        }
    }
}