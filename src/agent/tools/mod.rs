use crate::provider::ToolSpec;
use anyhow::{Result, bail};

use super::harness::truncate_tool_output;

pub mod bash_ops;
pub mod code_symbols;
pub mod file_ops;
pub mod git_ops;
pub mod search_ops;
pub mod subagent;
pub mod todo_ops;
pub mod web_ops;

pub struct ToolRegistry;

impl ToolRegistry {
    pub fn all_specs() -> Vec<ToolSpec> {
        let mut specs = Vec::new();
        specs.extend(file_ops::get_file_tools_specs());
        specs.extend(bash_ops::get_bash_tools_specs());
        specs.extend(git_ops::get_git_tools_specs());
        specs.extend(search_ops::get_search_tools_specs());
        specs.extend(web_ops::get_web_tools_specs());
        specs.extend(code_symbols::get_symbol_tools_specs());
        specs.extend(todo_ops::get_todo_tools_specs());
        specs.extend(subagent::get_subagent_tools_specs());
        specs
    }

    pub async fn execute_tool(name: &str, arguments: &serde_json::Value) -> Result<String> {
        let name = name.to_string();
        let arguments = arguments.clone();
        let result = match name.as_str() {
            "read_file" => file_ops::read_file(&arguments),
            "write_file" => file_ops::write_file(&arguments),
            "edit_file" => file_ops::edit_file(&arguments),
            "list_dir" => file_ops::list_dir(&arguments),
            "glob_files" => file_ops::glob_files(&arguments),
            "run_command" => bash_ops::run_command(&arguments).await,
            "git_status" => git_ops::git_status(&arguments).await,
            "git_diff" => git_ops::git_diff(&arguments).await,
            "git_log" => git_ops::git_log(&arguments).await,
            "git_commit" => git_ops::git_commit(&arguments).await,
            "search_code" => search_ops::search_code(&arguments),
            "fetch_url" => web_ops::fetch_url(&arguments).await,
            "find_symbols" => code_symbols::find_symbols(&arguments),
            "update_todos" => {
                let todos = todo_ops::parse_todos(&arguments)?;
                Ok(todo_ops::format_todos(&todos))
            }
            _ => bail!("Unknown tool name '{}'", name),
        };
        result.map(truncate_tool_output)
    }
}
