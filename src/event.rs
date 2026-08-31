use crate::app::AppState;
use crate::config;
use crate::config::Last;
use crate::discovery;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
#[cfg(test)]
use crossterm::event::KeyModifiers;
use std::path::Path;
use std::time::Duration;

pub enum Action {
    Continue,
    Launch,
    LaunchNative,
    Quit,
}

pub fn crossterm_read() -> Event {
    loop {
        if let Ok(ev) = event::read() {
            return ev;
        }
    }
}

pub fn refresh_discovery(state: &mut AppState) {
    let Some(provider) = state.focused_provider().map(|s| s.to_string()) else {
        return;
    };
    let Some(cfg) = state.config.providers.get(&provider) else {
        return;
    };
    let discovered = discovery::fetch_remote_models(&cfg.base_url, &cfg.api_key, Duration::from_millis(1500));
    match discovered {
        Some(models) if !models.is_empty() => state.set_focused_provider_models(models),
        _ => state.refresh_focused_provider_models(),
    }
}

pub fn handle_key(state: &mut AppState, key: KeyEvent, config_path: &Path, last_path: &Path) -> Action {
    if state.modal.is_some() {
        match key.code {
            KeyCode::Char(c) => state.modal_input_char(c),
            KeyCode::Backspace => state.modal_backspace(),
            KeyCode::Enter => {
                state.confirm_modal();
                config::save_config(&state.config, config_path);
            }
            KeyCode::Esc => state.close_modal(),
            _ => {}
        }
        return Action::Continue;
    }

    match key.code {
        KeyCode::Tab => state.switch_focus(),
        KeyCode::Up => {
            state.move_cursor(-1);
            if state.focused_panel == crate::app::Panel::Providers {
                refresh_discovery(state);
            }
        }
        KeyCode::Down => {
            state.move_cursor(1);
            if state.focused_panel == crate::app::Panel::Providers {
                refresh_discovery(state);
            }
        }
        KeyCode::Char('a') => state.open_add_model_modal(),
        KeyCode::Char('x') => {
            state.remove_focused_model();
            config::save_config(&state.config, config_path);
        }
        KeyCode::Char('s') => state.open_set_api_key_modal(),
        KeyCode::Char('r') => {
            state.toggle_rtk();
            save_current_last(state, last_path);
        }
        KeyCode::Char('p') => {
            state.toggle_auto_accept();
            save_current_last(state, last_path);
        }
        KeyCode::Enter => {
            if state.apply_selection() {
                save_current_last(state, last_path);
            }
        }
        KeyCode::Char('l') if state.can_launch() => return Action::Launch,
        KeyCode::Char('n') => return Action::LaunchNative,
        KeyCode::Char('q') | KeyCode::Esc => return Action::Quit,
        _ => {}
    }
    Action::Continue
}

fn save_current_last(state: &AppState, last_path: &Path) {
    let last = Last {
        provider: state.current_provider.clone(),
        model: state.current_model.clone(),
        rtk_enabled: state.rtk_enabled,
        auto_accept: state.auto_accept,
    };
    config::save_last(&last, last_path);
}

#[cfg(test)]
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::config::{Config, Last, Provider};
    use indexmap::IndexMap;
    use tempfile::tempdir;

    fn state_with(providers: &[(&str, &[&str])]) -> AppState {
        let mut map = IndexMap::new();
        for (name, models) in providers {
            map.insert(
                name.to_string(),
                Provider {
                    base_url: "http://127.0.0.1:1".to_string(), // nothing listens here
                    api_key: "key".to_string(),
                    models: models.iter().map(|m| m.to_string()).collect(),
                },
            );
        }
        AppState::new(Config { providers: map }, &Last::default())
    }

    #[test]
    fn tab_switches_focus() {
        let mut state = state_with(&[("a", &["m1"])]);
        let dir = tempdir().unwrap();
        handle_key(&mut state, key(KeyCode::Tab), &dir.path().join("c.yaml"), &dir.path().join("l.yaml"));
        assert_eq!(state.focused_panel, crate::app::Panel::Models);
    }

    #[test]
    fn q_returns_quit_action() {
        let mut state = state_with(&[("a", &["m1"])]);
        let dir = tempdir().unwrap();
        let action = handle_key(&mut state, key(KeyCode::Char('q')), &dir.path().join("c.yaml"), &dir.path().join("l.yaml"));
        assert!(matches!(action, Action::Quit));
    }

    #[test]
    fn l_returns_launch_only_when_selection_applied() {
        let mut state = state_with(&[("a", &["m1"])]);
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("c.yaml");
        let last_path = dir.path().join("l.yaml");

        let action = handle_key(&mut state, key(KeyCode::Char('l')), &config_path, &last_path);
        assert!(matches!(action, Action::Continue));

        handle_key(&mut state, key(KeyCode::Enter), &config_path, &last_path);
        let action = handle_key(&mut state, key(KeyCode::Char('l')), &config_path, &last_path);
        assert!(matches!(action, Action::Launch));
    }

    #[test]
    fn enter_applies_selection_and_persists_last_yaml() {
        let mut state = state_with(&[("a", &["m1"])]);
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("c.yaml");
        let last_path = dir.path().join("l.yaml");

        handle_key(&mut state, key(KeyCode::Enter), &config_path, &last_path);

        let saved = config::load_last(&last_path);
        assert_eq!(saved.provider.as_deref(), Some("a"));
        assert_eq!(saved.model.as_deref(), Some("m1"));
    }

    #[test]
    fn r_toggles_rtk_and_persists_last_yaml() {
        let mut state = state_with(&[("a", &["m1"])]);
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("c.yaml");
        let last_path = dir.path().join("l.yaml");

        handle_key(&mut state, key(KeyCode::Char('r')), &config_path, &last_path);

        assert!(state.rtk_enabled);
        let saved = config::load_last(&last_path);
        assert!(saved.rtk_enabled);
    }

    #[test]
    fn p_toggles_auto_accept_and_persists_last_yaml() {
        let mut state = state_with(&[("a", &["m1"])]);
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("c.yaml");
        let last_path = dir.path().join("l.yaml");

        handle_key(&mut state, key(KeyCode::Char('p')), &config_path, &last_path);

        assert!(state.auto_accept);
        let saved = config::load_last(&last_path);
        assert!(saved.auto_accept);
    }

    #[test]
    fn x_removes_focused_model_and_persists_config() {
        let mut state = state_with(&[("a", &["m1", "m2"])]);
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("c.yaml");
        let last_path = dir.path().join("l.yaml");
        config::save_config(&state.config, &config_path);

        handle_key(&mut state, key(KeyCode::Char('x')), &config_path, &last_path);

        assert_eq!(state.config.providers["a"].models, vec!["m2"]);
        match config::load_config(&config_path) {
            config::LoadConfigOutcome::Loaded(reloaded) => {
                assert_eq!(reloaded.providers["a"].models, vec!["m2"]);
            }
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn modal_keys_are_routed_to_modal_input_when_open() {
        let mut state = state_with(&[("a", &["m1"])]);
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("c.yaml");
        let last_path = dir.path().join("l.yaml");

        handle_key(&mut state, key(KeyCode::Char('a')), &config_path, &last_path); // open add-model modal
        handle_key(&mut state, key(KeyCode::Char('m')), &config_path, &last_path);
        handle_key(&mut state, key(KeyCode::Char('2')), &config_path, &last_path);
        // 'q' while a modal is open must NOT quit — it's routed as modal text input
        let action = handle_key(&mut state, key(KeyCode::Char('q')), &config_path, &last_path);
        assert!(matches!(action, Action::Continue));
        handle_key(&mut state, key(KeyCode::Enter), &config_path, &last_path);

        assert_eq!(state.config.providers["a"].models, vec!["m1", "m2q"]);
    }

    #[test]
    fn refresh_discovery_falls_back_to_static_models_when_unreachable() {
        let mut state = state_with(&[("a", &["m1", "m2"])]);
        // Prove the fallback path actually runs, rather than the constructor's
        // initial population happening to already match.
        state.models_for_focused_provider = vec![];
        refresh_discovery(&mut state);
        assert_eq!(state.models_for_focused_provider, vec!["m1", "m2"]);
    }
}
