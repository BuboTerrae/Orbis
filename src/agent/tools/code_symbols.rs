use crate::agent::workspace::{read_text_file, resolve_path};
use crate::provider::ToolSpec;
use anyhow::{Context, Result};
use serde_json::json;

pub fn find_symbols(args: &serde_json::Value) -> Result<String> {
    let path_str = args["path"]
        .as_str()
        .context("Missing 'path' argument in find_symbols")?;

    let path = resolve_path(path_str);
    if !path.exists() {
        anyhow::bail!("Path '{}' does not exist", path.display());
    }

    let content = read_text_file(&path)?;

    let mut symbols = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Rust / TS / Python / C / Go symbols keywords
        let is_symbol = trimmed.starts_with("pub fn ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("pub struct ")
            || trimmed.starts_with("struct ")
            || trimmed.starts_with("pub enum ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("pub trait ")
            || trimmed.starts_with("trait ")
            || trimmed.starts_with("impl ")
            || trimmed.starts_with("pub mod ")
            || trimmed.starts_with("mod ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("interface ")
            || trimmed.starts_with("type ")
            || trimmed.starts_with("async fn ");

        if is_symbol {
            symbols.push(format!("{:4} | {}", idx + 1, trimmed));
        }
    }

    if symbols.is_empty() {
        Ok(format!(
            "No top-level symbol declarations found in '{}'",
            path.display()
        ))
    } else {
        Ok(format!(
            "Symbols in '{}':\n{}",
            path.display(),
            symbols.join("\n")
        ))
    }
}

pub fn get_symbol_tools_specs() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "find_symbols".to_string(),
        description: "List symbol declarations (functions, structs, impls, classes, traits) in a source code file.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Source code file path" }
            },
            "required": ["path"]
        }),
    }]
}
