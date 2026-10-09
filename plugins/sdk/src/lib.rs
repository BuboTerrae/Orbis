//! Orbis Plugin SDK
//! 
//! This crate defines the stable interface for Orbis plugins.
//! Plugins can be loaded dynamically and provide custom tools.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub min_orbis_version: String,
}

/// Tool parameter schema (JSON Schema)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameterSchema {
    #[serde(rename = "type")]
    pub param_type: String,
    pub properties: HashMap<String, ToolProperty>,
    pub required: Vec<String>,
    pub additional_properties: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolProperty {
    #[serde(rename = "type")]
    pub prop_type: String,
    pub description: Option<String>,
    pub enum_values: Option<Vec<String>>,
    pub default: Option<Value>,
}

/// Tool specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: ToolParameterSchema,
}

/// Tool execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContext {
    pub working_directory: String,
    pub session_id: String,
    pub user_id: Option<String>,
    pub metadata: HashMap<String, Value>,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub metadata: HashMap<String, Value>,
}

/// Main plugin trait - implement this to create a plugin
#[async_trait]
pub trait OrbisPlugin: Send + Sync {
    /// Returns plugin metadata
    fn metadata(&self) -> PluginMetadata;

    /// Returns all tools provided by this plugin
    fn tools(&self) -> Vec<ToolSpec>;

    /// Called when plugin is loaded
    async fn on_load(&mut self, _config: Value) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called when plugin is unloaded
    async fn on_unload(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Execute a tool by name
    async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Value,
        context: ToolContext,
    ) -> anyhow::Result<ToolResult>;

    /// Optional: health check
    async fn health_check(&self) -> anyhow::Result<bool> {
        Ok(true)
    }
}

/// Plugin entry point - must be exported by the plugin crate
/// 
/// This function should return a boxed instance of the plugin
pub type PluginEntryPoint = unsafe fn() -> *mut dyn OrbisPlugin;

/// Helper macro to export the plugin entry point
#[macro_export]
macro_rules! export_plugin {
    ($plugin_type:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn _orbis_plugin_entry() -> *mut dyn $crate::OrbisPlugin {
            let plugin: Box<dyn $crate::OrbisPlugin> = Box::new(<$plugin_type>::default());
            Box::into_raw(plugin)
        }
    };
}

/// Plugin registry for managing loaded plugins
pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn OrbisPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn register(&mut self, plugin: Box<dyn OrbisPlugin>) {
        let name = plugin.metadata().name.clone();
        self.plugins.insert(name, plugin);
    }

    pub fn get(&self, name: &str) -> Option<&dyn OrbisPlugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }

    pub fn get_mut(&mut self, name: &str) -> Option<Box<dyn OrbisPlugin>> {
        self.plugins.remove(name)
    }

    pub fn all_tools(&self) -> Vec<ToolSpec> {
        self.plugins
            .values()
            .flat_map(|p| p.tools())
            .collect()
    }

    pub async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Value,
        context: ToolContext,
    ) -> anyhow::Result<ToolResult> {
        for plugin in self.plugins.values() {
            for spec in plugin.tools() {
                if spec.name == tool_name {
                    return plugin.execute_tool(tool_name, arguments, context).await;
                }
            }
        }
        anyhow::bail!("Tool '{}' not found", tool_name)
    }

    pub async fn shutdown_all(&mut self) -> anyhow::Result<()> {
        for (_, mut plugin) in self.plugins.drain() {
            plugin.on_unload().await?;
        }
        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Dynamic plugin loader
pub struct PluginLoader;

impl PluginLoader {
    /// Load a plugin from a dynamic library
    /// 
    /// # Safety
    /// The library must export the `_orbis_plugin_entry` symbol
    pub unsafe fn load(path: &std::path::Path) -> anyhow::Result<Box<dyn OrbisPlugin>> {
        let lib = libloading::Library::new(path)?;
        let entry: libloading::Symbol<PluginEntryPoint> = lib.get(b"_orbis_plugin_entry")?;
        let plugin_ptr = entry();
        if plugin_ptr.is_null() {
            anyhow::bail!("Plugin entry point returned null");
        }
        Ok(Box::from_raw(plugin_ptr))
    }
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub enabled: bool,
    pub path: String,
    pub config: Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_metadata() {
        let metadata = PluginMetadata {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin".to_string(),
            author: "Test Author".to_string(),
            min_orbis_version: "0.1.0".to_string(),
        };
        assert_eq!(metadata.name, "test");
    }

    #[test]
    fn test_tool_spec() {
        let spec = ToolSpec {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: ToolParameterSchema {
                param_type: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
                additional_properties: Some(false),
            },
        };
        assert_eq!(spec.name, "test_tool");
    }
}