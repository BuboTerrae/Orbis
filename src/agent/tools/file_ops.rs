use crate::agent::workspace::{glob_match, read_text_file, resolve_path, skip_dir_name};
use crate::provider::ToolSpec;
use anyhow::{Context, Result};
use serde_json::json;
use std::{fs, path::Path};

pub fn read_file(args: &serde_json::Value) -> Result<String> {
    let path_str = args["path"]
        .as_str()
        .context("Missing 'path' argument in read_file")?;
    let start_line = args["start_line"].as_u64().map(|n| n as usize);
    let end_line = args["end_line"].as_u64().map(|n| n as usize);

    let path = resolve_path(path_str);
    let content = read_text_file(&path)?;

    let lines: Vec<&str> = content.lines().collect();
    let start = start_line.unwrap_or(1).saturating_sub(1);
    let end = end_line.unwrap_or(lines.len()).min(lines.len());

    if start >= lines.len() {
        return Ok(format!(
            "[File: {} is empty or start line exceeds total lines ({})]",
            path.display(),
            lines.len()
        ));
    }

    let selected_lines = &lines[start..end];
    let numbered_content: Vec<String> = selected_lines
        .iter()
        .enumerate()
        .map(|(idx, line)| format!("{:4} | {}", start + idx + 1, line))
        .collect();

    Ok(format!(
        "[File: {} | lines {}-{} of {}]\n{}",
        path.display(),
        start + 1,
        end,
        lines.len(),
        numbered_content.join("\n")
    ))
}

pub fn write_file(args: &serde_json::Value) -> Result<String> {
    let path_str = args["path"]
        .as_str()
        .context("Missing 'path' argument in write_file")?;
    let content = args["content"]
        .as_str()
        .context("Missing 'content' argument in write_file")?;

    let path = resolve_path(path_str);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&path, content)
        .with_context(|| format!("Failed to write file at path '{}'", path.display()))?;

    Ok(format!(
        "Successfully wrote {} bytes to '{}'",
        content.len(),
        path.display()
    ))
}

pub fn list_dir(args: &serde_json::Value) -> Result<String> {
    let path_str = args["path"].as_str().unwrap_or(".");
    let path = resolve_path(path_str);

    let entries = fs::read_dir(&path)
        .with_context(|| format!("Failed to list directory contents at '{}'", path.display()))?;

    let mut result = Vec::new();
    for entry in entries {
        let entry = entry?;
        let meta = entry.metadata()?;
        let file_type = if meta.is_dir() { "DIR " } else { "FILE" };
        let name = entry.file_name().to_string_lossy().to_string();
        result.push(format!("[{}] {} ({} bytes)", file_type, name, meta.len()));
    }

    result.sort();
    if result.is_empty() {
        Ok(format!("Directory '{}' is empty.", path.display()))
    } else {
        Ok(format!(
            "Directory '{}':\n{}",
            path.display(),
            result.join("\n")
        ))
    }
}

pub fn edit_file(args: &serde_json::Value) -> Result<String> {
    let path_str = args["path"]
        .as_str()
        .context("Missing 'path' argument in edit_file")?;
    let target_content = args["target_content"]
        .as_str()
        .context("Missing 'target_content' argument in edit_file")?;
    let replacement_content = args["replacement_content"]
        .as_str()
        .context("Missing 'replacement_content' argument in edit_file")?;
    let replace_all = args["replace_all"].as_bool().unwrap_or(false);

    let path = resolve_path(path_str);
    let content = read_text_file(&path)?;

    let occurrences = content.matches(target_content).count();
    if occurrences == 0 {
        anyhow::bail!(
            "Target content to replace was not found in '{}'. Ensure an exact match.",
            path.display()
        );
    }
    if !replace_all && occurrences > 1 {
        anyhow::bail!(
            "Target content matched {} times in '{}'. Pass replace_all=true or include more surrounding context.",
            occurrences,
            path.display()
        );
    }

    let new_content = if replace_all {
        content.replace(target_content, replacement_content)
    } else {
        content.replacen(target_content, replacement_content, 1)
    };
    fs::write(&path, new_content)
        .with_context(|| format!("Failed to write edited file '{}'", path.display()))?;

    Ok(format!(
        "Successfully edited '{}'. Replaced {} occurrence(s).",
        path.display(),
        if replace_all { occurrences } else { 1 }
    ))
}

pub fn glob_files(args: &serde_json::Value) -> Result<String> {
    let pattern = args["pattern"]
        .as_str()
        .context("Missing 'pattern' argument in glob_files")?;
    let path_str = args["path"].as_str().unwrap_or(".");
    let root = resolve_path(path_str);

    let mut matches = Vec::new();
    fn walk(
        dir: &Path,
        pattern: &str,
        root: &Path,
        out: &mut Vec<String>,
        max: usize,
    ) -> Result<()> {
        if out.len() >= max {
            return Ok(());
        }
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };
        for entry in entries.flatten() {
            if out.len() >= max {
                break;
            }
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if skip_dir_name(&name) {
                continue;
            }
            if path.is_dir() {
                walk(&path, pattern, root, out, max)?;
            } else if path.is_file() {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if glob_match(pattern, &rel_str) || glob_match(pattern, &name) {
                    out.push(rel_str);
                }
            }
        }
        Ok(())
    }

    walk(&root, pattern, &root, &mut matches, 200)?;
    matches.sort();
    if matches.is_empty() {
        Ok(format!(
            "No files matching '{}' under '{}'",
            pattern,
            root.display()
        ))
    } else {
        Ok(format!(
            "{} file(s) matching '{}':\n{}",
            matches.len(),
            pattern,
            matches.join("\n")
        ))
    }
}

pub fn get_file_tools_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "read_file".to_string(),
            description: "Read a UTF-8 file with optional 1-indexed line range. Returns numbered lines.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Relative or absolute file path" },
                    "start_line": { "type": "integer", "description": "Optional 1-indexed start line" },
                    "end_line": { "type": "integer", "description": "Optional 1-indexed end line" }
                },
                "required": ["path"]
            }),
        },
        ToolSpec {
            name: "write_file".to_string(),
            description: "Create or overwrite a file. Creates parent directories if needed.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Target file path" },
                    "content": { "type": "string", "description": "Full file contents" }
                },
                "required": ["path", "content"]
            }),
        },
        ToolSpec {
            name: "edit_file".to_string(),
            description: "Replace an exact substring in a file. Fails if the target is missing or matches more than once unless replace_all is true.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File to modify" },
                    "target_content": { "type": "string", "description": "Exact text to replace" },
                    "replacement_content": { "type": "string", "description": "Replacement text" },
                    "replace_all": { "type": "boolean", "description": "Replace every occurrence (default false)" }
                },
                "required": ["path", "target_content", "replacement_content"]
            }),
        },
        ToolSpec {
            name: "list_dir".to_string(),
            description: "List files and subdirectories in a directory.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path (defaults to workspace root)" }
                }
            }),
        },
        ToolSpec {
            name: "glob_files".to_string(),
            description: "Find files by glob pattern (e.g. **/*.rs, src/**/mod.rs). Skips target/, node_modules/, and hidden dirs.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Glob pattern. * matches within a segment; ** matches across directories." },
                    "path": { "type": "string", "description": "Directory to search (defaults to workspace root)" }
                },
                "required": ["pattern"]
            }),
        },
    ]
}
