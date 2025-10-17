use std::collections::HashMap;

/// Simple plugin trait; real plugins could be dynamic libraries or config-driven
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn preprocess(&self, _task: &mut crate::pipeline::parser::TaskDef, _globals: &HashMap<String,String>) -> anyhow::Result<()>;
}

/// Example trivial plugin that does nothing
pub struct NoopPlugin;
impl Plugin for NoopPlugin {
    fn name(&self) -> &str { "noop" }
    fn preprocess(&self, _task: &mut crate::pipeline::parser::TaskDef, _globals: &HashMap<String,String>) -> anyhow::Result<()> {
        Ok(())
    }
}
