use orbis_plugin_sdk::{export_plugin, OrbisPlugin, PluginMetadata, ToolSpec, ToolParameterSchema, ToolProperty, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Default)]
pub struct ExamplePlugin;

#[async_trait]
impl OrbisPlugin for ExamplePlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "example".to_string(),
            version: "0.1.0".to_string(),
            description: "Example plugin demonstrating the Orbis Plugin SDK".to_string(),
            author: "Orbis Team".to_string(),
            min_orbis_version: "0.1.0".to_string(),
        }
    }

    fn tools(&self) -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: "example_echo".to_string(),
                description: "Echo back the input message".to_string(),
                parameters: ToolParameterSchema {
                    param_type: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("message".to_string(), ToolProperty {
                            prop_type: "string".to_string(),
                            description: Some("Message to echo".to_string()),
                            enum_values: None,
                            default: None,
                        });
                        props
                    },
                    required: vec!["message".to_string()],
                    additional_properties: Some(false),
                },
            },
            ToolSpec {
                name: "example_calc".to_string(),
                description: "Perform a simple calculation".to_string(),
                parameters: ToolParameterSchema {
                    param_type: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("a".to_string(), ToolProperty {
                            prop_type: "number".to_string(),
                            description: Some("First number".to_string()),
                            enum_values: None,
                            default: None,
                        });
                        props.insert("b".to_string(), ToolProperty {
                            prop_type: "number".to_string(),
                            description: Some("Second number".to_string()),
                            enum_values: None,
                            default: None,
                        });
                        props.insert("operation".to_string(), ToolProperty {
                            prop_type: "string".to_string(),
                            description: Some("Operation: add, sub, mul, div".to_string()),
                            enum_values: Some(vec!["add".to_string(), "sub".to_string(), "mul".to_string(), "div".to_string()]),
                            default: Some(json!("add")),
                        });
                        props
                    },
                    required: vec!["a".to_string(), "b".to_string()],
                    additional_properties: Some(false),
                },
            },
        ]
    }

    async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Value,
        _context: ToolContext,
    ) -> anyhow::Result<ToolResult> {
        match tool_name {
            "example_echo" => {
                let message = arguments["message"].as_str().unwrap_or("");
                Ok(ToolResult {
                    success: true,
                    output: format!("Echo: {}", message),
                    error: None,
                    metadata: HashMap::new(),
                })
            }
            "example_calc" => {
                let a = arguments["a"].as_f64().unwrap_or(0.0);
                let b = arguments["b"].as_f64().unwrap_or(0.0);
                let op = arguments["operation"].as_str().unwrap_or("add");

                let result = match op {
                    "add" => a + b,
                    "sub" => a - b,
                    "mul" => a * b,
                    "div" => if b != 0.0 { a / b } else { return Err(anyhow::anyhow!("Division by zero")); },
                    _ => return Err(anyhow::anyhow!("Unknown operation: {}", op)),
                };

                Ok(ToolResult {
                    success: true,
                    output: format!("Result: {}", result),
                    error: None,
                    metadata: HashMap::new(),
                })
            }
            _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
        }
    }
}

export_plugin!(ExamplePlugin);