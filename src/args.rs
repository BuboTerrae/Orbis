use crate::agent::runner::parse_provider_flag;
use crate::provider::config::{Config, ProviderType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    pub print: bool,
    pub auto: bool,
    pub readonly: bool,
    pub help: bool,
    pub provider: Option<ProviderType>,
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub prompt: String,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            print: false,
            auto: false,
            readonly: false,
            help: false,
            provider: None,
            model: None,
            endpoint: None,
            prompt: String::new(),
        }
    }
}

pub fn parse_args(args: &[String]) -> Result<CliArgs, String> {
    let mut out = CliArgs::default();
    let mut positional = Vec::new();
    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "-h" | "--help" => out.help = true,
            "-p" | "--print" => {
                out.print = true;
                if let Some(next) = args.get(i + 1) {
                    if !next.starts_with('-') {
                        positional.push(next.clone());
                        i += 1;
                    }
                }
            }
            "--yolo" | "--auto" => out.auto = true,
            "--readonly" | "--read-only" => out.readonly = true,
            "--provider" => {
                let val = args.get(i + 1).ok_or("--provider needs a name")?;
                out.provider = Some(
                    parse_provider_flag(val).ok_or_else(|| format!("unknown provider {val}"))?,
                );
                i += 1;
            }
            "--model" => {
                let val = args.get(i + 1).ok_or("--model needs a value")?;
                out.model = Some(val.clone());
                i += 1;
            }
            "--endpoint" => {
                let val = args.get(i + 1).ok_or("--endpoint needs a URL")?;
                out.endpoint = Some(val.clone());
                i += 1;
            }
            "--" => {
                positional.extend(args[i + 1..].iter().cloned());
                break;
            }
            s if s.starts_with('-') => {
                return Err(format!("unknown flag {s}. Try --help"));
            }
            s => positional.push(s.to_string()),
        }
        i += 1;
    }
    out.prompt = positional.join(" ");
    Ok(out)
}

pub fn apply_to_config(args: &CliArgs, config: &mut Config) {
    if let Some(p) = &args.provider {
        config.active_provider = p.clone();
        if args.model.is_none() {
            config.active_model = p.default_model().to_string();
        }
    }
    if let Some(m) = &args.model {
        config.active_model = m.clone();
    }
    if let Some(url) = &args.endpoint {
        if config.active_provider == ProviderType::Custom
            || config.active_provider == ProviderType::OpenAI
        {
            if config.active_provider == ProviderType::Custom {
                config.custom_base_url = Some(url.clone());
            } else {
                config.openai_base_url = Some(url.clone());
            }
        } else {
            config.custom_base_url = Some(url.clone());
            config.active_provider = ProviderType::Custom;
        }
    }
}

pub fn usage() -> &'static str {
    "Polynia Code — terminal coding agent

Usage:
  polynia                         Interactive TUI
  polynia -p \"fix the tests\"      Headless print mode (agent loop on stdout)
  polynia --print --yolo \"...\"    Headless, auto-approve all tools
  polynia --readonly -p \"...\"     Headless, read-only tools

Flags:
  -p, --print [prompt]   Run without TUI; stream the final answer
  --yolo, --auto         Auto-approve write/shell tools in print mode
  --readonly             Block write/shell/git-commit tools
  --provider NAME        gemini | openai | anthropic | openrouter | deepseek | custom
  --model NAME           Model id
  --endpoint URL         OpenAI-compatible base URL (implies custom if needed)
  -h, --help             Show this help

Keys (TUI): Alt+P provider · Alt+K keys · Alt+S sessions · Alt+Q quit
"
}

#[cfg(test)]
mod tests {
    use super::parse_args;

    fn args(s: &str) -> Vec<String> {
        std::iter::once("polynia".to_string())
            .chain(s.split_whitespace().map(|x| x.to_string()))
            .collect()
    }

    #[test]
    fn parse_print_and_provider() {
        let a = parse_args(&args("--print --provider openai --model gpt-4o fix tests")).unwrap();
        assert!(a.print);
        assert_eq!(a.prompt, "fix tests");
        assert_eq!(a.model.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn parse_print_consumes_next_token() {
        let a = parse_args(&args("-p hello")).unwrap();
        assert!(a.print);
        assert_eq!(a.prompt, "hello");
    }

    #[test]
    fn unknown_flag_errors() {
        assert!(parse_args(&args("--nope")).is_err());
    }
}
