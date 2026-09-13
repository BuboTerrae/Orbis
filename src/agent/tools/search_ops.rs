use crate::agent::workspace::{read_text_file, resolve_path, skip_dir_name};
use crate::provider::ToolSpec;
use anyhow::{Context, Result};
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn search_code(args: &serde_json::Value) -> Result<String> {
    let query = args["query"]
        .as_str()
        .context("Missing 'query' argument in search_code")?;
    let path_str = args["path"].as_str().unwrap_or(".");
    let case_sensitive = args["case_sensitive"].as_bool().unwrap_or(false);
    let max_matches = args["max_matches"].as_u64().unwrap_or(50).clamp(1, 200) as usize;

    let query_search = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    let mut matches = Vec::new();
    let root = resolve_path(path_str);

    fn walk_and_search(
        dir: &Path,
        query: &str,
        case_sensitive: bool,
        matches: &mut Vec<String>,
        max_matches: usize,
    ) -> Result<()> {
        if matches.len() >= max_matches {
            return Ok(());
        }

        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            if skip_dir_name(&file_name) {
                continue;
            }

            if path.is_dir() {
                walk_and_search(&path, query, case_sensitive, matches, max_matches)?;
            } else if path.is_file() {
                if crate::agent::workspace::looks_binary(&path) {
                    continue;
                }
                let Ok(content) = read_text_file(&path) else {
                    continue;
                };
                for (idx, line) in content.lines().enumerate() {
                    let line_match = if case_sensitive {
                        line.contains(query)
                    } else {
                        line.to_lowercase().contains(query)
                    };

                    if line_match {
                        matches.push(format!(
                            "{}:{} | {}",
                            path.to_string_lossy(),
                            idx + 1,
                            line.trim()
                        ));
                        if matches.len() >= max_matches {
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    if root.is_file() {
        let content = read_text_file(&root)?;
        for (idx, line) in content.lines().enumerate() {
            let line_match = if case_sensitive {
                line.contains(&query_search)
            } else {
                line.to_lowercase().contains(&query_search)
            };
            if line_match {
                matches.push(format!("{}:{} | {}", root.display(), idx + 1, line.trim()));
                if matches.len() >= max_matches {
                    break;
                }
            }
        }
    } else {
        walk_and_search(
            &root,
            &query_search,
            case_sensitive,
            &mut matches,
            max_matches,
        )?;
    }

    if matches.is_empty() {
        Ok(format!("No matches found for query '{}'", query))
    } else {
        Ok(matches.join("\n"))
    }
}

pub fn get_search_tools_specs() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "search_code".to_string(),
        description: "Search a keyword or substring across source files. Skips target/, node_modules/, and hidden directories.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Text to search" },
                "path": { "type": "string", "description": "Directory or file to search (defaults to workspace root)" },
                "case_sensitive": { "type": "boolean", "description": "Case-sensitive match (default false)" },
                "max_matches": { "type": "integer", "description": "Max matches to return (default 50, max 200)" }
            },
            "required": ["query"]
        }),
    }]
}
