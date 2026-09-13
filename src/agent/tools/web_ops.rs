use crate::provider::ToolSpec;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;

pub async fn fetch_url(args: &serde_json::Value) -> Result<String> {
    let url = args["url"]
        .as_str()
        .context("Missing 'url' argument in fetch_url")?;

    if !url.starts_with("http://") && !url.starts_with("https://") {
        anyhow::bail!("fetch_url only supports http:// and https:// URLs");
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("PolyniaCode/0.1")
        .build()?;

    let res = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch URL '{}'", url))?;

    let status = res.status();
    let text = res
        .text()
        .await
        .with_context(|| format!("Failed to read response body from '{}'", url))?;

    if text.len() > 10_000 {
        Ok(format!(
            "HTTP {} ({})\n--- Content (Truncated to 10KB) ---\n{}",
            status,
            url,
            &text[..10_000]
        ))
    } else {
        Ok(format!(
            "HTTP {} ({})\n--- Content ---\n{}",
            status, url, text
        ))
    }
}

pub fn get_web_tools_specs() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "fetch_url".to_string(),
        description: "Fetch web content or documentation API endpoint via HTTP GET request."
            .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "Absolute URL to fetch (http/https)" }
            },
            "required": ["url"]
        }),
    }]
}
