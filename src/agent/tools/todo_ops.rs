use crate::provider::ToolSpec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

impl std::fmt::Display for TodoStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TodoStatus::Pending => write!(f, "pending"),
            TodoStatus::InProgress => write!(f, "in_progress"),
            TodoStatus::Completed => write!(f, "completed"),
            TodoStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
}

pub fn parse_todos(args: &serde_json::Value) -> Result<Vec<TodoItem>> {
    let items = args["items"]
        .as_array()
        .context("Missing 'items' array in update_todos")?;
    let mut todos = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let id = item["id"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}", i + 1));
        let content = item["content"]
            .as_str()
            .context("Each todo needs 'content'")?
            .to_string();
        let status = match item["status"].as_str().unwrap_or("pending") {
            "in_progress" | "in-progress" => TodoStatus::InProgress,
            "completed" | "done" => TodoStatus::Completed,
            "cancelled" | "canceled" => TodoStatus::Cancelled,
            _ => TodoStatus::Pending,
        };
        todos.push(TodoItem {
            id,
            content,
            status,
        });
    }
    Ok(todos)
}

pub fn format_todos(todos: &[TodoItem]) -> String {
    if todos.is_empty() {
        return "Todo list cleared.".to_string();
    }
    let mut lines = vec![format!("{} todo(s):", todos.len())];
    for t in todos {
        let mark = match t.status {
            TodoStatus::Completed => "x",
            TodoStatus::InProgress => "*",
            TodoStatus::Cancelled => "-",
            TodoStatus::Pending => " ",
        };
        lines.push(format!("[{mark}] {} · {}", t.id, t.content));
    }
    lines.join("\n")
}

pub fn get_todo_tools_specs() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "update_todos".to_string(),
        description: "Replace the in-session plan/todo list. Use for multi-step work: keep exactly one item in_progress, mark items completed as you finish them.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "content": { "type": "string" },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed", "cancelled"]
                            }
                        },
                        "required": ["content", "status"]
                    }
                }
            },
            "required": ["items"]
        }),
    }]
}

#[cfg(test)]
mod tests {
    use super::{TodoStatus, parse_todos};
    use serde_json::json;

    #[test]
    fn parses_items() {
        let todos = parse_todos(&json!({
            "items": [
                {"id": "1", "content": "Read event.rs", "status": "completed"},
                {"content": "Wire sessions", "status": "in_progress"}
            ]
        }))
        .unwrap();
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].status, TodoStatus::Completed);
        assert_eq!(todos[1].id, "2");
    }
}
