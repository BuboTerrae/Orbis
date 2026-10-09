use crate::provider::config::{Config, ProviderType};
use anyhow::Result;
use std::io::{self, Write};

pub async fn run_setup_wizard() -> Result<()> {
    println!("┌─────────────────────────────────────────────┐");
    println!("│         Welcome to Orbis Setup Wizard       │");
    println!("└─────────────────────────────────────────────┘");
    println!();

    let mut config = Config::load();

    // Check if already configured
    let has_env_key = config.is_key_from_env(&config.active_provider);
    let has_config_key = match config.active_provider {
        ProviderType::Gemini => config.gemini_api_key.is_some(),
        ProviderType::OpenAI => config.openai_api_key.is_some(),
        ProviderType::Anthropic => config.anthropic_api_key.is_some(),
        ProviderType::OpenRouter => config.openrouter_api_key.is_some(),
        ProviderType::DeepSeek => config.deepseek_api_key.is_some(),
        ProviderType::Custom => config.custom_api_key.is_some(),
    };

    if has_env_key || has_config_key {
        println!(
            "✓ API key already configured for {}",
            config.active_provider
        );
        print!("Run setup anyway? [y/N]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            return Ok(());
        }
    }

    println!("Available providers:");
    for (i, provider) in ProviderType::all().iter().enumerate() {
        println!("  {} - {} ({})", i + 1, provider, provider.default_model());
    }
    println!();

    print!("Select provider [1-{}]: ", ProviderType::all().len());
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let idx = input
        .trim()
        .parse::<usize>()
        .unwrap_or(1)
        .clamp(1, ProviderType::all().len())
        - 1;
    let provider = ProviderType::all()[idx].clone();
    config.active_provider = provider.clone();
    config.active_model = provider.default_model().to_string();

    println!("\nSelected: {} ({})", provider, provider.default_model());
    println!("\nEnvironment variables checked (in order):");
    let env_vars = match provider {
        ProviderType::Gemini => vec!["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        ProviderType::OpenAI => vec!["OPENAI_API_KEY"],
        ProviderType::Anthropic => vec!["ANTHROPIC_API_KEY"],
        ProviderType::OpenRouter => vec!["OPENROUTER_API_KEY"],
        ProviderType::DeepSeek => vec!["DEEPSEEK_API_KEY"],
        ProviderType::Custom => vec!["POLYNIA_API_KEY", "CUSTOM_API_KEY", "OPENAI_API_KEY"],
    };
    for var in &env_vars {
        let exists = std::env::var(var).is_ok();
        println!("  {} - {}", if exists { "✓" } else { "✗" }, var);
    }

    print!("\nEnter API key (or press Enter to skip): ");
    io::stdout().flush()?;
    let mut api_key = String::new();
    io::stdin().read_line(&mut api_key)?;
    let api_key = api_key.trim().to_string();

    if !api_key.is_empty() {
        match provider {
            ProviderType::Gemini => config.gemini_api_key = Some(api_key),
            ProviderType::OpenAI => config.openai_api_key = Some(api_key),
            ProviderType::Anthropic => config.anthropic_api_key = Some(api_key),
            ProviderType::OpenRouter => config.openrouter_api_key = Some(api_key),
            ProviderType::DeepSeek => config.deepseek_api_key = Some(api_key),
            ProviderType::Custom => config.custom_api_key = Some(api_key),
        }
        config.save()?;
        println!("✓ API key saved to config");
    } else if !has_env_key && !has_config_key {
        println!("⚠ No API key provided. You can set it later with Alt+K in TUI.");
    }

    // Test connection
    println!("\nTesting connection...");
    match test_provider(&config).await {
        Ok(_) => println!("✓ Connection successful!"),
        Err(e) => {
            println!("✗ Connection failed: {}", e);
            println!("  You can still use Orbis - it will retry on each request.");
        }
    }

    println!("\nSetup complete! Run 'orbis' to start.");
    Ok(())
}

async fn test_provider(config: &Config) -> Result<()> {
    let provider = crate::agent::build_provider(config)?;
    let messages = vec![crate::provider::ChatMessage::user("Say 'OK'")];
    let tools = vec![];
    let _ = provider.stream_chat(&messages, &tools).await?;
    Ok(())
}

pub async fn run_doctor() -> Result<()> {
    println!("┌─────────────────────────────────────────────┐");
    println!("│           Orbis Doctor                      │");
    println!("└─────────────────────────────────────────────┘");
    println!();

    let config = Config::load();

    // Check config file
    if let Some(path) = Config::config_file_path() {
        println!("Config file: {}", path.display());
        if path.exists() {
            println!("  ✓ Exists");
        } else {
            println!("  ✗ Not found (will use defaults)");
        }
    } else {
        println!("Config file: ✗ Could not determine config directory");
    }

    // Check .env
    let env_path = std::env::current_dir()?.join(".env");
    println!(".env file: {}", env_path.display());
    if env_path.exists() {
        println!("  ✓ Exists");
        let content = std::fs::read_to_string(&env_path)?;
        for line in content.lines() {
            if line.contains("API_KEY") || line.contains("api_key") {
                let parts: Vec<&str> = line.split('=').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    let value = parts[1].trim();
                    println!(
                        "  Found: {}={}",
                        key,
                        if value.len() > 8 { &value[..8] } else { value }
                    );
                }
            }
        }
    } else {
        println!("  ✗ Not found in current directory");
    }

    // Check provider
    println!("\nActive provider: {}", config.active_provider);
    println!("Active model: {}", config.active_model);

    let has_key = config.is_key_from_env(&config.active_provider)
        || match config.active_provider {
            ProviderType::Gemini => config.gemini_api_key.is_some(),
            ProviderType::OpenAI => config.openai_api_key.is_some(),
            ProviderType::Anthropic => config.anthropic_api_key.is_some(),
            ProviderType::OpenRouter => config.openrouter_api_key.is_some(),
            ProviderType::DeepSeek => config.deepseek_api_key.is_some(),
            ProviderType::Custom => config.custom_api_key.is_some(),
        };

    println!(
        "API key: {}",
        if has_key {
            "✓ Configured"
        } else {
            "✗ Missing"
        }
    );

    // Test network
    println!("\nTesting network connectivity...");
    let client = reqwest::Client::new();
    match client
        .get("https://generativelanguage.googleapis.com")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => println!("  ✓ Internet reachable (status: {})", resp.status()),
        Err(e) => println!("  ✗ Network error: {}", e),
    }

    // Check binary
    println!("\nBinary info:");
    println!("  Version: {}", env!("CARGO_PKG_VERSION"));

    Ok(())
}
