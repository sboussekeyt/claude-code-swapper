use crate::config::{Config, Last};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Providers,
    Models,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Modal {
    AddModel { provider: String, input: String },
    SetApiKey { provider: String, input: String },
}

pub struct AppState {
    pub providers: Vec<String>,
    pub focused_panel: Panel,
    pub provider_cursor: usize,
    pub model_cursor: usize,
    pub models_for_focused_provider: Vec<String>,
    pub current_provider: Option<String>,
    pub current_model: Option<String>,
    pub rtk_enabled: bool,
    pub auto_accept: bool,
    pub config: Config,
    pub modal: Option<Modal>,
    pub status_message: Option<String>,
}

impl AppState {
    pub fn new(config: Config, last: &Last) -> Self {
        let providers: Vec<String> = config.providers.keys().cloned().collect();
        let provider_cursor = last
            .provider
            .as_ref()
            .and_then(|p| providers.iter().position(|x| x == p))
            .unwrap_or(0);
        let mut state = AppState {
            providers,
            focused_panel: Panel::Providers,
            provider_cursor,
            model_cursor: 0,
            models_for_focused_provider: Vec::new(),
            current_provider: last.provider.clone(),
            current_model: last.model.clone(),
            rtk_enabled: last.rtk_enabled,
            auto_accept: last.auto_accept,
            config,
            modal: None,
            status_message: None,
        };
        state.refresh_focused_provider_models();
        state
    }

    pub fn focused_provider(&self) -> Option<&str> {
        self.providers.get(self.provider_cursor).map(|s| s.as_str())
    }

    pub fn switch_focus(&mut self) {
        self.focused_panel = match self.focused_panel {
            Panel::Providers => Panel::Models,
            Panel::Models => Panel::Providers,
        };
    }

    pub fn move_cursor(&mut self, delta: i32) {
        match self.focused_panel {
            Panel::Providers => {
                let len = self.providers.len();
                if len > 0 {
                    self.provider_cursor = clamp_cursor(self.provider_cursor, delta, len);
                }
            }
            Panel::Models => {
                let len = self.models_for_focused_provider.len();
                if len > 0 {
                    self.model_cursor = clamp_cursor(self.model_cursor, delta, len);
                }
            }
        }
    }

    pub fn set_focused_provider_models(&mut self, models: Vec<String>) {
        self.model_cursor = 0;
        if let Some(current) = &self.current_model {
            if self.focused_provider() == self.current_provider.as_deref() {
                if let Some(idx) = models.iter().position(|m| m == current) {
                    self.model_cursor = idx;
                }
            }
        }
        self.models_for_focused_provider = models;
    }

    pub fn refresh_focused_provider_models(&mut self) {
        let models = self
            .focused_provider()
            .and_then(|p| self.config.providers.get(p))
            .map(|p| p.models.clone())
            .unwrap_or_default();
        self.set_focused_provider_models(models);
    }

    pub fn apply_selection(&mut self) -> bool {
        let Some(provider) = self.providers.get(self.provider_cursor).cloned() else {
            self.status_message = Some("No providers configured.".to_string());
            return false;
        };
        let Some(model) = self.models_for_focused_provider.get(self.model_cursor).cloned() else {
            self.status_message = Some("No models available for this provider.".to_string());
            return false;
        };
        self.current_provider = Some(provider);
        self.current_model = Some(model);
        true
    }

    pub fn can_launch(&self) -> bool {
        self.current_provider.is_some() && self.current_model.is_some()
    }

    pub fn add_model(&mut self, provider: &str, model_name: &str) -> bool {
        let Some(p) = self.config.providers.get_mut(provider) else {
            return false;
        };
        if p.models.iter().any(|m| m == model_name) {
            self.status_message = Some(format!("Model '{model_name}' already exists in {provider}."));
            return false;
        }
        p.models.push(model_name.to_string());
        self.refresh_focused_provider_models();
        true
    }

    pub fn remove_model(&mut self, provider: &str, model_name: &str) {
        if let Some(p) = self.config.providers.get_mut(provider) {
            p.models.retain(|m| m != model_name);
        }
        self.refresh_focused_provider_models();
    }

    pub fn remove_focused_model(&mut self) {
        let Some(provider) = self.focused_provider().map(|s| s.to_string()) else {
            return;
        };
        let Some(model) = self.models_for_focused_provider.get(self.model_cursor).cloned() else {
            return;
        };
        self.remove_model(&provider, &model);
    }

    pub fn set_api_key(&mut self, provider: &str, key: &str) -> bool {
        let Some(p) = self.config.providers.get_mut(provider) else {
            return false;
        };
        p.api_key = key.to_string();
        true
    }

    pub fn toggle_rtk(&mut self) {
        self.rtk_enabled = !self.rtk_enabled;
    }

    pub fn toggle_auto_accept(&mut self) {
        self.auto_accept = !self.auto_accept;
    }

    pub fn open_add_model_modal(&mut self) {
        if let Some(provider) = self.focused_provider() {
            self.modal = Some(Modal::AddModel {
                provider: provider.to_string(),
                input: String::new(),
            });
        }
    }

    pub fn open_set_api_key_modal(&mut self) {
        if let Some(provider) = self.focused_provider() {
            self.modal = Some(Modal::SetApiKey {
                provider: provider.to_string(),
                input: String::new(),
            });
        }
    }

    pub fn modal_input_char(&mut self, c: char) {
        match &mut self.modal {
            Some(Modal::AddModel { input, .. }) | Some(Modal::SetApiKey { input, .. }) => input.push(c),
            None => {}
        }
    }

    pub fn modal_backspace(&mut self) {
        match &mut self.modal {
            Some(Modal::AddModel { input, .. }) | Some(Modal::SetApiKey { input, .. }) => {
                input.pop();
            }
            None => {}
        }
    }

    pub fn close_modal(&mut self) {
        self.modal = None;
    }

    pub fn confirm_modal(&mut self) {
        match self.modal.take() {
            Some(Modal::AddModel { provider, input }) if !input.trim().is_empty() => {
                self.add_model(&provider, input.trim());
            }
            Some(Modal::SetApiKey { provider, input }) if !input.trim().is_empty() => {
                self.set_api_key(&provider, &input);
            }
            _ => {}
        }
    }
}

fn clamp_cursor(cursor: usize, delta: i32, len: usize) -> usize {
    let new = cursor as i32 + delta;
    new.clamp(0, len as i32 - 1) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Provider;
    use indexmap::IndexMap;

    fn config_with(providers: &[(&str, &[&str])]) -> Config {
        let mut map = IndexMap::new();
        for (name, models) in providers {
            map.insert(
                name.to_string(),
                Provider {
                    base_url: format!("https://{name}.example.com"),
                    api_key: "key".to_string(),
                    models: models.iter().map(|m| m.to_string()).collect(),
                },
            );
        }
        Config { providers: map }
    }

    #[test]
    fn new_preserves_provider_order_and_defaults_cursor_to_zero() {
        let config = config_with(&[("openrouter", &["a"]), ("groq", &["b"])]);
        let state = AppState::new(config, &Last::default());
        assert_eq!(state.providers, vec!["openrouter", "groq"]);
        assert_eq!(state.provider_cursor, 0);
        assert_eq!(state.focused_provider(), Some("openrouter"));
    }

    #[test]
    fn new_preselects_last_provider_cursor() {
        let config = config_with(&[("openrouter", &["a"]), ("groq", &["b"])]);
        let last = Last {
            provider: Some("groq".to_string()),
            ..Default::default()
        };
        let state = AppState::new(config, &last);
        assert_eq!(state.provider_cursor, 1);
    }

    #[test]
    fn new_loads_static_models_for_initial_provider() {
        let config = config_with(&[("openrouter", &["model-a", "model-b"])]);
        let state = AppState::new(config, &Last::default());
        assert_eq!(state.models_for_focused_provider, vec!["model-a", "model-b"]);
    }

    #[test]
    fn switch_focus_toggles_between_panels() {
        let config = config_with(&[("openrouter", &["a"])]);
        let mut state = AppState::new(config, &Last::default());
        assert_eq!(state.focused_panel, Panel::Providers);
        state.switch_focus();
        assert_eq!(state.focused_panel, Panel::Models);
        state.switch_focus();
        assert_eq!(state.focused_panel, Panel::Providers);
    }

    #[test]
    fn move_cursor_clamps_within_providers_panel() {
        let config = config_with(&[("a", &[]), ("b", &[]), ("c", &[])]);
        let mut state = AppState::new(config, &Last::default());
        state.move_cursor(-1);
        assert_eq!(state.provider_cursor, 0);
        state.move_cursor(1);
        state.move_cursor(1);
        state.move_cursor(1);
        assert_eq!(state.provider_cursor, 2);
    }

    #[test]
    fn move_cursor_operates_on_models_panel_when_focused() {
        let config = config_with(&[("a", &["m1", "m2"])]);
        let mut state = AppState::new(config, &Last::default());
        state.switch_focus();
        state.move_cursor(1);
        assert_eq!(state.model_cursor, 1);
        assert_eq!(state.provider_cursor, 0);
    }

    #[test]
    fn set_focused_provider_models_preselects_current_model_when_present() {
        let config = config_with(&[("a", &["m1", "m2", "m3"])]);
        let last = Last {
            provider: Some("a".to_string()),
            model: Some("m3".to_string()),
            ..Default::default()
        };
        let mut state = AppState::new(config, &last);
        state.set_focused_provider_models(vec!["m1".to_string(), "m2".to_string(), "m3".to_string()]);
        assert_eq!(state.model_cursor, 2);
    }

    #[test]
    fn apply_selection_sets_current_provider_and_model() {
        let config = config_with(&[("a", &["m1", "m2"])]);
        let mut state = AppState::new(config, &Last::default());
        state.switch_focus();
        state.move_cursor(1);
        assert!(state.apply_selection());
        assert_eq!(state.current_provider.as_deref(), Some("a"));
        assert_eq!(state.current_model.as_deref(), Some("m2"));
    }

    #[test]
    fn apply_selection_fails_when_no_models_available() {
        let config = config_with(&[("a", &[])]);
        let mut state = AppState::new(config, &Last::default());
        assert!(!state.apply_selection());
        assert!(state.status_message.is_some());
    }

    #[test]
    fn can_launch_requires_both_provider_and_model() {
        let config = config_with(&[("a", &["m1"])]);
        let mut state = AppState::new(config, &Last::default());
        assert!(!state.can_launch());
        state.apply_selection();
        assert!(state.can_launch());
    }

    #[test]
    fn add_model_appends_and_refreshes_models_list() {
        let config = config_with(&[("a", &["m1"])]);
        let mut state = AppState::new(config, &Last::default());
        assert!(state.add_model("a", "m2"));
        assert_eq!(state.config.providers["a"].models, vec!["m1", "m2"]);
        assert_eq!(state.models_for_focused_provider, vec!["m1", "m2"]);
    }

    #[test]
    fn add_model_rejects_duplicate() {
        let config = config_with(&[("a", &["m1"])]);
        let mut state = AppState::new(config, &Last::default());
        assert!(!state.add_model("a", "m1"));
        assert_eq!(state.config.providers["a"].models, vec!["m1"]);
        assert!(state.status_message.is_some());
    }

    #[test]
    fn remove_focused_model_removes_highlighted_model() {
        let config = config_with(&[("a", &["m1", "m2"])]);
        let mut state = AppState::new(config, &Last::default());
        state.switch_focus();
        state.move_cursor(1); // highlight m2
        state.remove_focused_model();
        assert_eq!(state.config.providers["a"].models, vec!["m1"]);
        assert_eq!(state.models_for_focused_provider, vec!["m1"]);
    }

    #[test]
    fn set_api_key_updates_provider_key() {
        let config = config_with(&[("a", &["m1"])]);
        let mut state = AppState::new(config, &Last::default());
        assert!(state.set_api_key("a", "new-key"));
        assert_eq!(state.config.providers["a"].api_key, "new-key");
    }

    #[test]
    fn toggle_rtk_and_auto_accept_flip_booleans() {
        let config = config_with(&[("a", &["m1"])]);
        let mut state = AppState::new(config, &Last::default());
        assert!(!state.rtk_enabled);
        state.toggle_rtk();
        assert!(state.rtk_enabled);
        assert!(!state.auto_accept);
        state.toggle_auto_accept();
        assert!(state.auto_accept);
    }

    #[test]
    fn add_model_modal_flow_types_and_confirms() {
        let config = config_with(&[("a", &["m1"])]);
        let mut state = AppState::new(config, &Last::default());
        state.open_add_model_modal();
        assert_eq!(
            state.modal,
            Some(Modal::AddModel { provider: "a".to_string(), input: String::new() })
        );
        state.modal_input_char('m');
        state.modal_input_char('2');
        state.modal_backspace();
        state.modal_input_char('2');
        state.confirm_modal();
        assert_eq!(state.modal, None);
        assert_eq!(state.config.providers["a"].models, vec!["m1", "m2"]);
    }

    #[test]
    fn close_modal_discards_without_applying() {
        let config = config_with(&[("a", &["m1"])]);
        let mut state = AppState::new(config, &Last::default());
        state.open_add_model_modal();
        state.modal_input_char('x');
        state.close_modal();
        assert_eq!(state.modal, None);
        assert_eq!(state.config.providers["a"].models, vec!["m1"]);
    }

    #[test]
    fn set_api_key_modal_flow_confirms() {
        let config = config_with(&[("a", &["m1"])]);
        let mut state = AppState::new(config, &Last::default());
        state.open_set_api_key_modal();
        state.modal_input_char('k');
        state.confirm_modal();
        assert_eq!(state.config.providers["a"].api_key, "k");
    }

    #[test]
    fn confirm_modal_ignores_empty_add_model_input() {
        let config = config_with(&[("a", &["m1"])]);
        let mut state = AppState::new(config, &Last::default());
        state.open_add_model_modal();
        state.confirm_modal();
        assert_eq!(state.config.providers["a"].models, vec!["m1"]);
    }

    #[test]
    fn confirm_modal_ignores_whitespace_only_api_key_input() {
        let config = config_with(&[("a", &["m1"])]);
        let mut state = AppState::new(config, &Last::default());
        let original_key = state.config.providers["a"].api_key.clone();
        state.open_set_api_key_modal();
        state.modal_input_char(' ');
        state.confirm_modal();
        assert_eq!(state.modal, None);
        assert_eq!(state.config.providers["a"].api_key, original_key);
    }
}
