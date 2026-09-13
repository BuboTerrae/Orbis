use anyhow::Result;
use crossterm::event::{self as ct_event, Event as CtEvent};
use std::io::IsTerminal;
use std::time::Duration;
use tokio::sync::mpsc;

mod agent;
mod app;
mod args;
mod event;
mod provider;
mod ui;

use app::App;
use event::AppEvent;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install().map_err(|e| anyhow::anyhow!("{e}"))?;

    let raw: Vec<String> = std::env::args().collect();
    let cli = match args::parse_args(&raw) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };

    if cli.help {
        print!("{}", args::usage());
        return Ok(());
    }

    let mut config = provider::config::Config::load();
    args::apply_to_config(&cli, &mut config);

    let mut prompt = cli.prompt.clone();
    if prompt.is_empty() && !std::io::stdin().is_terminal() {
        prompt = std::io::read_to_string(std::io::stdin())?;
        prompt = prompt.trim().to_string();
    }

    if cli.print || (!prompt.is_empty() && !std::io::stdin().is_terminal()) {
        let options = agent::runner::RunOptions {
            max_turns: 40,
            readonly: cli.readonly,
            depth: 0,
            print_tokens: true,
        };
        match agent::runner::run_prompt(&config, &prompt, options).await {
            Ok(text) => {
                if !text.is_empty() && !cli.print {
                    println!("{text}");
                } else if !text.ends_with('\n') {
                    println!();
                }
            }
            Err(e) => {
                eprintln!("{e:#}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    let mut terminal = ratatui::init();
    let mut app = App::new();
    app.config = config;
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();

    let res = run_app(&mut terminal, &mut app, tx, &mut rx).await;
    app.persist_session();
    ratatui::restore();

    if let Err(err) = res {
        eprintln!("{err:?}");
    }
    Ok(())
}

async fn run_app(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    tx: mpsc::UnboundedSender<AppEvent>,
    rx: &mut mpsc::UnboundedReceiver<AppEvent>,
) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::render(frame, app))?;

        if ct_event::poll(Duration::from_millis(20))? {
            if let CtEvent::Key(key) = ct_event::read()? {
                if key.kind == ct_event::KeyEventKind::Press {
                    event::handle_key_event(app, key, &tx)?;
                }
            }
        }

        while let Ok(event) = rx.try_recv() {
            event::apply_event(app, event, &tx);
        }
    }
    Ok(())
}
