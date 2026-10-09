use anyhow::Result;
use orbis::provider::config::Config;
use std::fs;
use tempfile::TempDir;

/// Integration test helper that creates a temp workspace
struct TestWorkspace {
    dir: TempDir,
    #[expect(dead_code)]
    config: Config,
}

impl TestWorkspace {
    fn new() -> Result<Self> {
        let dir = TempDir::new()?;
        let config = Config {
            gemini_api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        Ok(Self { dir, config })
    }

    fn path(&self, p: &str) -> std::path::PathBuf {
        self.dir.path().join(p)
    }

    fn write_file(&self, path: &str, content: &str) -> Result<()> {
        let full = self.path(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full, content)?;
        Ok(())
    }

    fn read_file(&self, path: &str) -> Result<String> {
        fs::read_to_string(self.path(path)).map_err(Into::into)
    }
}

#[tokio::test]
async fn test_agent_creates_file() -> Result<()> {
    let _ws = TestWorkspace::new()?;

    // This test would need a mock provider to work without real API keys
    // For now, we test the tool execution directly
    Ok(())
}

#[tokio::test]
async fn test_tool_execution_read_write() -> Result<()> {
    let ws = TestWorkspace::new()?;

    // Test write_file tool
    ws.write_file("test.txt", "hello world")?;
    assert_eq!(ws.read_file("test.txt")?, "hello world");

    // Test edit_file tool
    ws.write_file("edit_test.rs", "fn main() { println!(\"old\"); }")?;
    // Would need to invoke edit_file tool directly

    Ok(())
}

#[tokio::test]
async fn test_session_persistence() -> Result<()> {
    use orbis::agent::session::{Session, load, new_session_id, save};
    use orbis::provider::ChatMessage;

    let ws = TestWorkspace::new()?;
    std::env::set_current_dir(ws.dir.path())?;

    let session = Session {
        id: new_session_id(),
        title: "Test Session".to_string(),
        updated_unix: 1234567890,
        cwd: ws.dir.path().to_string_lossy().to_string(),
        messages: vec![
            ChatMessage::system("test system"),
            ChatMessage::user("test user"),
            ChatMessage::assistant("test assistant"),
        ],
        todos: vec![],
        input_history: vec!["history1".to_string()],
    };

    let path = save(&session)?;
    assert!(path.exists());

    let loaded = load(&session.id)?;
    assert_eq!(loaded.title, "Test Session");
    assert_eq!(loaded.messages.len(), 3);
    assert_eq!(loaded.input_history.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_compaction_preserves_tool_pairs() -> Result<()> {
    use orbis::agent::compact::{compact_messages, should_compact};
    use orbis::provider::{ChatMessage, Role, ToolCall};

    let messages = vec![
        ChatMessage::system("sys"),
        ChatMessage::user("old task"),
        ChatMessage::assistant_with_tools(
            "working",
            vec![ToolCall {
                id: "t1".into(),
                name: "read_file".into(),
                arguments: serde_json::json!({"path": "a.rs"}),
            }],
        ),
        ChatMessage::tool_response("t1", "read_file", "fn main() {}"),
        ChatMessage::user("new task"),
        ChatMessage::assistant("ok"),
    ];

    // should_compact returns false for small conversations (correct behavior)
    assert!(!should_compact(&messages));
    // But compact_messages should still work when called directly

    let compacted = compact_messages(&messages, 2, 50);
    assert_eq!(compacted[0].role, Role::System);

    // Tool call and response should both be preserved
    let has_tool_call = compacted.iter().any(|m| {
        m.role == Role::Assistant
            && m.tool_calls
                .as_ref()
                .is_some_and(|c| c.iter().any(|t| t.id == "t1"))
    });
    let has_tool_response = compacted
        .iter()
        .any(|m| m.role == Role::Tool && m.tool_call_id.as_deref() == Some("t1"));
    assert!(has_tool_call);
    assert!(has_tool_response);

    // New task should be preserved
    assert!(
        compacted
            .iter()
            .any(|m| { m.role == Role::User && m.content.contains("new task") })
    );

    Ok(())
}

#[tokio::test]
async fn test_glob_matching() -> Result<()> {
    use orbis::agent::workspace::glob_match;

    assert!(glob_match("*.rs", "main.rs"));
    assert!(!glob_match("*.rs", "src/main.rs"));
    assert!(glob_match("**/*.rs", "src/main.rs"));
    assert!(glob_match("**/mod.rs", "cli/src/ui/mod.rs"));
    assert!(!glob_match("*.rs", "main.toml"));
    assert!(glob_match("cli/src/*.rs", "cli/src/main.rs"));
    assert!(glob_match("**/*", "a/b/c"));

    Ok(())
}

#[tokio::test]
async fn test_todo_parsing() -> Result<()> {
    use orbis::agent::tools::todo_ops::{TodoStatus, parse_todos};
    use serde_json::json;

    let todos = parse_todos(&json!({
        "items": [
            {"id": "1", "content": "Read event.rs", "status": "completed"},
            {"content": "Wire sessions", "status": "in_progress"}
        ]
    }))?;

    assert_eq!(todos.len(), 2);
    assert_eq!(todos[0].status, TodoStatus::Completed);
    assert_eq!(todos[1].id, "2");
    assert_eq!(todos[1].status, TodoStatus::InProgress);

    Ok(())
}

#[tokio::test]
async fn test_tools_for_depth() -> Result<()> {
    use orbis::agent::runner::tools_for_depth;

    let top = tools_for_depth(0);
    let nested = tools_for_depth(1);

    assert!(top.iter().any(|t| t.name == "spawn_subagent"));
    assert!(nested.iter().all(|t| t.name != "spawn_subagent"));

    Ok(())
}

#[cfg(test)]
mod cli_tests {
    use orbis::args::parse_args;

    fn args(s: &str) -> Vec<String> {
        std::iter::once("orbis".to_string())
            .chain(s.split_whitespace().map(|x| x.to_string()))
            .collect()
    }

    #[test]
    fn test_parse_print_and_provider() {
        let a = parse_args(&args("--print --provider openai --model gpt-4o fix tests")).unwrap();
        assert!(a.print);
        assert_eq!(a.prompt, "fix tests");
        assert_eq!(a.model.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn test_parse_print_consumes_next_token() {
        let a = parse_args(&args("-p hello")).unwrap();
        assert!(a.print);
        assert_eq!(a.prompt, "hello");
    }

    #[test]
    fn test_unknown_flag_errors() {
        assert!(parse_args(&args("--nope")).is_err());
    }

    #[test]
    fn test_parse_readonly() {
        let a = parse_args(&args("--readonly -p test")).unwrap();
        assert!(a.readonly);
        assert_eq!(a.prompt, "test");
    }

    #[test]
    fn test_parse_auto() {
        let a = parse_args(&args("--auto -p test")).unwrap();
        assert!(a.auto);
    }

    #[test]
    fn test_parse_yolo() {
        let a = parse_args(&args("--yolo -p test")).unwrap();
        assert!(a.auto); // --yolo is alias for --auto
    }
}
