use crate::provider::{ChatMessage, Role};

/// Compact a transcript without breaking assistant tool_calls / tool result pairing.
///
/// Keeps the leading system prompt, optionally inserts a compaction marker, and
/// retains the most recent complete user turns. Older tool payloads are stubbed
/// so the model still sees that a tool ran.
pub fn compact_messages(
    messages: &[ChatMessage],
    keep_user_turns: usize,
    max_tool_chars: usize,
) -> Vec<ChatMessage> {
    if messages.is_empty() {
        return Vec::new();
    }

    let mut system = Vec::new();
    let mut rest: Vec<ChatMessage> = Vec::new();
    for msg in messages {
        if msg.role == Role::System && rest.is_empty() {
            system.push(msg.clone());
        } else {
            rest.push(msg.clone());
        }
    }

    let user_idxs: Vec<usize> = rest
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == Role::User)
        .map(|(i, _)| i)
        .collect();

    let keep_from = if user_idxs.len() > keep_user_turns {
        user_idxs[user_idxs.len() - keep_user_turns]
    } else {
        0
    };

    let dropped_turns = user_idxs.iter().filter(|&&i| i < keep_from).count();

    let mut out = system;
    if dropped_turns > 0 {
        out.push(ChatMessage::user(format!(
            "[Conversation compacted: {dropped_turns} earlier user turn(s) omitted. Continue from the remaining context.]"
        )));
        out.push(ChatMessage::assistant(
            "Understood. I will proceed from the remaining conversation context.".to_string(),
        ));
    }

    for msg in rest.into_iter().skip(keep_from) {
        out.push(stub_tool_payload(msg, max_tool_chars));
    }

    repair_tool_pairing(&mut out);
    out
}

fn stub_tool_payload(mut msg: ChatMessage, max_tool_chars: usize) -> ChatMessage {
    if msg.role == Role::Tool && msg.content.len() > max_tool_chars {
        let preview: String = msg.content.chars().take(120).collect();
        let preview = preview.replace('\n', " ");
        msg.content = format!(
            "[compacted tool output · {} chars] {}",
            msg.content.len(),
            preview
        );
    }
    msg
}

/// Drop orphan tool messages whose tool_call_id is not referenced by a prior assistant.
fn repair_tool_pairing(messages: &mut Vec<ChatMessage>) {
    let mut known_ids = std::collections::HashSet::new();
    messages.retain(|msg| match msg.role {
        Role::Assistant => {
            if let Some(ref calls) = msg.tool_calls {
                for tc in calls {
                    known_ids.insert(tc.id.clone());
                }
            }
            true
        }
        Role::Tool => msg
            .tool_call_id
            .as_ref()
            .is_some_and(|id| known_ids.contains(id)),
        _ => true,
    });
}

pub fn should_compact(messages: &[ChatMessage]) -> bool {
    let chars: usize = messages.iter().map(|m| m.content.len()).sum();
    messages.len() > 40 || chars > 100_000
}

#[cfg(test)]
mod tests {
    use super::compact_messages;
    use crate::provider::{ChatMessage, Role, ToolCall};

    #[test]
    fn keeps_tool_results_with_their_calls() {
        let messages = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("old"),
            ChatMessage::assistant_with_tools(
                "working",
                vec![ToolCall {
                    id: "t1".into(),
                    name: "read_file".into(),
                    arguments: serde_json::json!({"path": "a.rs"}),
                    thought_signature: None,
                }],
            ),
            ChatMessage::tool_response("t1", "read_file", "fn main() {}"),
            ChatMessage::user("new task"),
            ChatMessage::assistant("ok"),
        ];
        let out = compact_messages(&messages, 2, 50);
        assert_eq!(out[0].role, Role::System);
        assert!(
            out.iter()
                .any(|m| m.role == Role::Tool && m.tool_call_id.as_deref() == Some("t1"))
        );
        assert!(out.iter().any(|m| {
            m.role == Role::Assistant
                && m.tool_calls
                    .as_ref()
                    .is_some_and(|c| c.iter().any(|t| t.id == "t1"))
        }));
        assert!(
            out.iter()
                .any(|m| m.role == Role::User && m.content.contains("new task"))
        );
    }

    #[test]
    fn stubs_large_tool_output() {
        let big = "x".repeat(500);
        let messages = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("q"),
            ChatMessage::assistant_with_tools(
                "",
                vec![ToolCall {
                    id: "t1".into(),
                    name: "read_file".into(),
                    arguments: serde_json::json!({}),
                    thought_signature: None,
                }],
            ),
            ChatMessage::tool_response("t1", "read_file", big),
        ];
        let out = compact_messages(&messages, 2, 80);
        let tool = out.iter().find(|m| m.role == Role::Tool).unwrap();
        assert!(tool.content.starts_with("[compacted tool output"));
        assert!(tool.content.len() < 500);
    }
}
