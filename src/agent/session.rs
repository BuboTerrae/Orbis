use crate::provider::ChatMessage;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::tools::todo_ops::TodoItem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub updated_unix: u64,
    pub cwd: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub todos: Vec<TodoItem>,
    #[serde(default)]
    pub input_history: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub updated_unix: u64,
}

pub fn sessions_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|p| p.join("polynia").join("sessions"))
}

pub fn new_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn title_from_messages(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .find(|m| m.role == crate::provider::Role::User && !m.content.starts_with('['))
        .map(|m| {
            let line = m.content.lines().next().unwrap_or("").trim();
            let mut t: String = line.chars().take(48).collect();
            if line.chars().count() > 48 {
                t.push('…');
            }
            if t.is_empty() {
                "untitled".to_string()
            } else {
                t
            }
        })
        .unwrap_or_else(|| "untitled".to_string())
}

pub fn save(session: &Session) -> Result<PathBuf> {
    let dir = sessions_dir().context("Could not resolve data directory for sessions")?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", session.id));
    let json = serde_json::to_string_pretty(session)?;
    fs::write(&path, json)?;
    Ok(path)
}

pub fn load(id: &str) -> Result<Session> {
    let dir = sessions_dir().context("Could not resolve data directory for sessions")?;
    let path = dir.join(format!("{id}.json"));
    let raw = fs::read_to_string(&path).with_context(|| format!("Session '{}' not found", id))?;
    let session = serde_json::from_str(&raw).context("Invalid session file")?;
    Ok(session)
}

pub fn list() -> Result<Vec<SessionMeta>> {
    let Some(dir) = sessions_dir() else {
        return Ok(Vec::new());
    };
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(session) = serde_json::from_str::<Session>(&raw) else {
            continue;
        };
        out.push(SessionMeta {
            id: session.id,
            title: session.title,
            updated_unix: session.updated_unix,
        });
    }
    out.sort_by(|a, b| b.updated_unix.cmp(&a.updated_unix));
    out.truncate(30);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::title_from_messages;
    use crate::provider::ChatMessage;

    #[test]
    fn title_uses_first_user_prompt() {
        let msgs = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("Fix the agent loop in event.rs please"),
        ];
        assert_eq!(
            title_from_messages(&msgs),
            "Fix the agent loop in event.rs please"
        );
    }
}
