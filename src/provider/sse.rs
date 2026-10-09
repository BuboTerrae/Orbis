/// Incremental SSE `data:` line splitter. Handles `\r\n` and partial chunks.
#[derive(Default)]
pub struct SseParser {
    buffer: String,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buffer.push_str(&String::from_utf8_lossy(bytes));
        let mut events = Vec::new();
        while let Some(idx) = self.buffer.find('\n') {
            let mut line = self.buffer[..idx].to_string();
            self.buffer.drain(..=idx);
            if line.ends_with('\r') {
                line.pop();
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("data:") {
                let payload = rest.trim();
                if !payload.is_empty() {
                    events.push(payload.to_string());
                }
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::SseParser;

    #[test]
    fn splits_partial_sse_chunks() {
        let mut p = SseParser::new();
        assert!(p.push(b"data: {\"a\":").is_empty());
        let ev = p.push(b"1}\n\ndata: [DONE]\n");
        assert_eq!(ev, vec!["{\"a\":1}", "[DONE]"]);
    }
}
