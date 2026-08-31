mod app;
mod config;
mod discovery;
mod event;
mod launcher;
mod ui;

use app::AppState;
use crossterm::event::{Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Write};
use std::os::unix::process::CommandExt;

fn main() {
    let config_dir = config::config_dir();
    let config_path = config_dir.join("config.yaml");
    let last_path = config_dir.join("last.yaml");

    let cfg = match config::load_config(&config_path) {
        config::LoadConfigOutcome::Bootstrapped(path) => {
            println!("Config created at {}", path.display());
            println!("Edit it to add your API keys, then run claude-code-swapper again.");
            std::process::exit(0);
        }
        config::LoadConfigOutcome::ParseError(msg) => {
            eprintln!("{msg}");
            std::process::exit(1);
        }
        config::LoadConfigOutcome::Loaded(cfg) => cfg,
    };

    if let Err(msg) = launcher::check_claude() {
        eprintln!("Error: {msg}");
        std::process::exit(1);
    }

    if !launcher::check_rtk_installed() {
        print!("RTK is not installed (compresses tool output to save tokens). Install it now? [y/N] ");
        io::stdout().flush().ok();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).ok();
        if answer.trim().eq_ignore_ascii_case("y") {
            launcher::install_rtk();
        }
    }

    let last = config::load_last(&last_path);
    let mut state = AppState::new(cfg, &last);
    event::refresh_discovery(&mut state);

    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        default_panic(info);
    }));

    enable_raw_mode().expect("failed to enable raw mode");
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).expect("failed to enter alternate screen");

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("failed to create terminal");

    let action = loop {
        terminal.draw(|frame| ui::render(frame, &state)).expect("failed to draw frame");

        let Event::Key(key) = event::crossterm_read() else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match event::handle_key(&mut state, key, &config_path, &last_path) {
            event::Action::Continue => {}
            action @ (event::Action::Launch | event::Action::LaunchNative | event::Action::Quit) => {
                break action;
            }
        }
    };

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    drop(terminal);

    match action {
        event::Action::Launch => {
            let provider = state.current_provider.clone().unwrap();
            let model = state.current_model.clone().unwrap();
            let provider_cfg = &state.config.providers[&provider];
            if state.rtk_enabled {
                launcher::ensure_rtk_hook();
            }
            let env = launcher::build_env(&provider_cfg.base_url, &provider_cfg.api_key);
            let mut cmd = launcher::build_command(Some(&model), state.auto_accept, &env);
            let err = cmd.exec();
            eprintln!("failed to launch claude: {err}");
            std::process::exit(1);
        }
        event::Action::LaunchNative => {
            if state.rtk_enabled {
                launcher::ensure_rtk_hook();
            }
            // vars_os() (rather than vars()) avoids panicking if any inherited
            // environment variable is not valid UTF-8; a lossy conversion here
            // is a pragmatic middle ground since Command::envs needs owned Strings.
            let env: std::collections::HashMap<String, String> = std::env::vars_os()
                .map(|(k, v)| (k.to_string_lossy().into_owned(), v.to_string_lossy().into_owned()))
                .collect();
            let mut cmd = launcher::build_command(None, state.auto_accept, &env);
            let err = cmd.exec();
            eprintln!("failed to launch claude: {err}");
            std::process::exit(1);
        }
        event::Action::Quit => {}
        event::Action::Continue => unreachable!(),
    }
}
