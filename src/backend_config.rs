use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::{Context, Result};
use std::sync::Arc;
use crate::backends::{Backend, LocalBackend, SSHBackend, DockerBackend, KubernetesBackend};

/// Backend configuration that can be defined in YAML
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BackendConfig {
    Local,
    Ssh {
        host: String,
        #[serde(default)]
        user: Option<String>,
        #[serde(default)]
        port: Option<u16>,
        #[serde(default)]
        key_path: Option<String>,
        #[serde(default)]
        extra_args: Vec<String>,
    },
    Docker {
        image: String,
        #[serde(default)]
        extra_args: Vec<String>,
    },
    Kubernetes {
        image: String,
        #[serde(default)]
        namespace: Option<String>,
        #[serde(default)]
        extra_args: Vec<String>,
    },
}

impl BackendConfig {
    /// Convert config to a trait object
    pub fn to_backend(&self) -> Result<Arc<dyn Backend>> {
        match self {
            BackendConfig::Local => Ok(Arc::new(LocalBackend::new())),
            BackendConfig::Ssh { host, user, port, key_path, extra_args } => {
                let mut backend = SSHBackend::new(host);
                if let Some(u) = user {
                    backend = backend.with_user(u);
                }
                if let Some(p) = port {
                    backend = backend.with_port(*p);
                }
                if let Some(k) = key_path {
                    backend = backend.with_key(k);
                }
                if !extra_args.is_empty() {
                    backend = backend.with_args(extra_args.clone());
                }
                Ok(Arc::new(backend))
            }
            BackendConfig::Docker { image, extra_args } => {
                let mut backend = DockerBackend::new(image);
                if !extra_args.is_empty() {
                    backend = backend.with_args(extra_args.clone());
                }
                Ok(Arc::new(backend))
            }
            BackendConfig::Kubernetes { image, namespace, extra_args } => {
                let mut backend = KubernetesBackend::new(image);
                if let Some(ns) = namespace {
                    backend = backend.with_namespace(ns);
                }
                if !extra_args.is_empty() {
                    backend = backend.with_args(extra_args.clone());
                }
                Ok(Arc::new(backend))
            }
        }
    }
}

/// Manager for resolving backend names to implementations
pub struct BackendManager {
    backends: HashMap<String, Arc<dyn Backend>>,
    default_backend: Arc<dyn Backend>,
}

impl BackendManager {
    /// Create a new backend manager from a config map
    pub fn new(configs: HashMap<String, BackendConfig>) -> Result<Self> {
        let mut backends = HashMap::new();

        for (name, config) in configs {
            let backend = config.to_backend()
                .with_context(|| format!("failed to create backend '{}'", name))?;
            backends.insert(name, backend);
        }

        // Add a default local backend if not specified
        let default_backend = Arc::new(LocalBackend::new());
        backends.entry("local".to_string()).or_insert_with(|| default_backend.clone());

        Ok(Self {
            backends,
            default_backend,
        })
    }

    /// Get a backend by name, falling back to default if not found
    pub fn get(&self, name: &str) -> Arc<dyn Backend> {
        self.backends.get(name)
            .cloned()
            .unwrap_or_else(|| self.default_backend.clone())
    }

    /// Get the default backend
    pub fn default(&self) -> Arc<dyn Backend> {
        self.default_backend.clone()
    }
}

impl Default for BackendManager {
    fn default() -> Self {
        let default_backend: Arc<dyn Backend> = Arc::new(LocalBackend::new());
        let mut backends: HashMap<String, Arc<dyn Backend>> = HashMap::new();
        backends.insert("local".to_string(), default_backend.clone());

        Self {
            backends,
            default_backend,
        }
    }
}
