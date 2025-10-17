use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;
use std::fs;
use chrono::Utc;
use anyhow::Context;

/// Simple interpolation: replace {{task.output}} and {{vars.NAME}}
pub fn interpolate_command(template: &str, outputs: &HashMap<String, String>, vars: &HashMap<String, String>) -> String {
    let mut s = template.to_string();

    // Replace vars
    for (k, v) in vars {
        let p1 = format!("{{{{vars.{}}}}}", k);
        let p2 = format!("{{{{vars.{} }}}}", k);
        s = s.replace(&p1, v);
        s = s.replace(&p2, v);
    }

    // Replace outputs
    for (task, out) in outputs {
        let p1 = format!("{{{{{}.output}}}}", task);
        let p2 = format!("{{{{{}.output }}}}", task);
        s = s.replace(&p1, out.trim());
        s = s.replace(&p2, out.trim());
    }

    // Remove any remaining {{...}} to avoid executing raw templates later
    let re = Regex::new(r"\{\{.*?\}\}").unwrap();
    s = re.replace_all(&s, "").to_string();

    s
}

/// Create a run directory and return it
pub fn create_run_dir(base: &Path) -> anyhow::Result<std::path::PathBuf> {
    let run_id = Uuid::new_v4().to_string();
    let dir = base.join("runs").join(run_id);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn write_artifact(dir: &Path, name: &str, content: &str) -> anyhow::Result<()> {
    let path = dir.join(name);
    fs::write(path, content)?;
    Ok(())
}

pub fn timestamp() -> String {
    // Format: YYYY-MM-DD_HH-MM-SS
    Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}
