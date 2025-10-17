use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::Context;
use std::collections::{HashMap, HashSet};

/// Pipeline and TaskDef with Serialize + Deserialize so we can read & write YAML
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Pipeline {
    pub name: Option<String>,
    #[serde(default)]
    pub concurrency: Option<usize>,
    #[serde(default)]
    pub stop_on_fail: Option<bool>,
    pub tasks: Vec<TaskDef>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskDef {
    pub name: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub run: String,
    #[serde(default)]
    pub retries: Option<u32>,
    #[serde(default)]
    pub timeout: Option<u64>, // seconds
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub cache_key: Option<String>,
    #[serde(default)]
    pub continue_on_fail: Option<bool>,
}

/// Load YAML file into Pipeline
pub fn load_pipeline(path: &Path) -> anyhow::Result<Pipeline> {
    let content = std::fs::read_to_string(path).with_context(|| format!("failed to read {:?}", path))?;
    let p: Pipeline = serde_yaml::from_str(&content).with_context(|| format!("failed to parse YAML {:?}", path))?;
    Ok(p)
}

/// Validate DAG: unique names, existing deps, cycles
pub fn validate_pipeline(p: &Pipeline) -> anyhow::Result<()> {
    let mut names = HashSet::new();
    for t in &p.tasks {
        if !names.insert(t.name.clone()) {
            anyhow::bail!("duplicate task name '{}'", t.name);
        }
    }

    // All depends_on refer to existing tasks
    let name_set: HashSet<String> = p.tasks.iter().map(|t| t.name.clone()).collect();
    for t in &p.tasks {
        for dep in &t.depends_on {
            if !name_set.contains(dep) {
                anyhow::bail!("task '{}' depends on unknown '{}'", t.name, dep);
            }
        }
    }

    // Build adjacency (dep -> dependents) to check cycles
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for t in &p.tasks {
        for dep in &t.depends_on {
            adj.entry(dep.clone()).or_default().push(t.name.clone());
        }
    }

    // DFS to detect cycles
    let mut visited: HashMap<String, i32> = HashMap::new(); // 0=unvisited,1=visiting,2=done
    fn dfs(node: &str, adj: &HashMap<String, Vec<String>>, visited: &mut HashMap<String, i32>) -> Result<(), String> {
        if let Some(&v) = visited.get(node) {
            if v == 1 {
                return Err(format!("cycle detected at {}", node));
            }
            if v == 2 {
                return Ok(());
            }
        }
        visited.insert(node.to_string(), 1);
        if let Some(nei) = adj.get(node) {
            for n in nei {
                dfs(n, adj, visited)?;
            }
        }
        visited.insert(node.to_string(), 2);
        Ok(())
    }

    for t in &p.tasks {
        dfs(&t.name, &adj, &mut visited).map_err(|e| anyhow::anyhow!(e))?;
    }

    Ok(())
}

/// Helper: validate pipeline file path (for main)
pub fn validate_pipeline_file(path: &Path) -> anyhow::Result<()> {
    let pipeline = load_pipeline(path)?;
    validate_pipeline(&pipeline)?;
    println!("Pipeline '{}' validated", pipeline.name.clone().unwrap_or_else(|| "<unnamed>".to_string()));
    Ok(())
}
