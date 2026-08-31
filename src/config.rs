use crate::discovery::ProviderKind;
use crate::known_providers;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Provider {
    pub base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
    /// How to discover this provider's available models — see
    /// `discovery::ProviderKind`. Defaults to `Generic` (a plain
    /// OpenAI-compatible `/v1/models`), so existing configs need no change;
    /// set `kind: lmstudio` / `kind: ollama` to use that provider's richer
    /// native API instead.
    #[serde(default)]
    pub kind: ProviderKind,
    /// Optional per-model context window override, keyed by model name.
    /// Absent/unlisted models fall back to Claude Code's own detection.
    /// Set at launch via CLAUDE_CODE_MAX_CONTEXT_TOKENS (see launcher.rs).
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub context_windows: IndexMap<String, u64>,
}

/// Deserialize-only mirror of `Provider` where `base_url`/`kind` are left
/// unset (`None`) rather than defaulted, so `resolve` can tell "the user
/// didn't say" apart from "the user explicitly chose the default" before
/// filling gaps from `known_providers::lookup`. `Provider` itself stays the
/// single post-resolution type everything else in the app reads — this
/// struct exists only inside `load_config`.
#[derive(Debug, Clone, Default, Deserialize)]
struct RawProvider {
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    models: Vec<String>,
    #[serde(default)]
    kind: Option<ProviderKind>,
    #[serde(default)]
    context_windows: IndexMap<String, u64>,
}

impl RawProvider {
    fn resolve(self, name: &str) -> Provider {
        let known = known_providers::lookup(name);
        let base_url = self
            .base_url
            .filter(|s| !s.is_empty())
            .or_else(|| known.map(|k| k.base_url.to_string()))
            .unwrap_or_default();
        let api_key = if self.api_key.is_empty() {
            known.and_then(|k| k.default_api_key).unwrap_or_default().to_string()
        } else {
            self.api_key
        };
        let kind = self.kind.or_else(|| known.map(|k| k.kind)).unwrap_or_default();
        Provider { base_url, api_key, models: self.models, kind, context_windows: self.context_windows }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub providers: IndexMap<String, Provider>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawConfig {
    #[serde(default)]
    providers: IndexMap<String, RawProvider>,
}

impl From<RawConfig> for Config {
    fn from(raw: RawConfig) -> Self {
        let providers = raw.providers.into_iter().map(|(name, p)| {
            let resolved = p.resolve(&name);
            (name, resolved)
        });
        Config { providers: providers.collect() }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Last {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub rtk_enabled: bool,
    #[serde(default)]
    pub auto_accept: bool,
    /// Up to 10 most-recently-selected model names per provider, newest
    /// first. Usage history only — never affects config::Provider.models.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub recent_models: IndexMap<String, Vec<String>>,
}

pub enum LoadConfigOutcome {
    Loaded(Config),
    Bootstrapped(PathBuf),
    ParseError(String),
}

pub const CONFIG_DIR_NAME: &str = "claude-code-swapper";

pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .expect("home directory could not be resolved")
        .join(".config")
        .join(CONFIG_DIR_NAME)
}

const EXAMPLE_CONFIG: &str = include_str!("../assets/config.example.yaml");

pub fn load_config(path: &Path) -> LoadConfigOutcome {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, EXAMPLE_CONFIG);
        return LoadConfigOutcome::Bootstrapped(path.to_path_buf());
    }
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => return LoadConfigOutcome::ParseError(e.to_string()),
    };
    match serde_yaml_ng::from_str::<RawConfig>(&text) {
        Ok(raw) => LoadConfigOutcome::Loaded(raw.into()),
        Err(e) => LoadConfigOutcome::ParseError(format!(
            "Invalid YAML in {}:\n{}",
            path.display(),
            e
        )),
    }
}

pub fn save_config(config: &Config, path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(yaml) = serde_yaml_ng::to_string(config) {
        let _ = fs::write(path, yaml);
    }
}

pub fn load_last(path: &Path) -> Last {
    let Ok(text) = fs::read_to_string(path) else {
        return Last::default();
    };
    serde_yaml_ng::from_str(&text).unwrap_or_default()
}

pub fn save_last(last: &Last, path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(yaml) = serde_yaml_ng::to_string(last) {
        let _ = fs::write(path, yaml);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_config_yaml() -> &'static str {
        r#"
providers:
  openrouter:
    base_url: https://openrouter.ai/api
    api_key: sk-or-test
    models:
      - anthropic/claude-sonnet-4-6
      - meta-llama/llama-3.1-8b
"#
    }

    #[test]
    fn loads_existing_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, sample_config_yaml()).unwrap();

        match load_config(&path) {
            LoadConfigOutcome::Loaded(config) => {
                assert_eq!(
                    config.providers["openrouter"].api_key,
                    "sk-or-test"
                );
                assert_eq!(
                    config.providers["openrouter"].models,
                    vec!["anthropic/claude-sonnet-4-6", "meta-llama/llama-3.1-8b"]
                );
            }
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn known_provider_fills_in_base_url_and_kind_from_just_an_api_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(
            &path,
            r#"
providers:
  openrouter:
    api_key: sk-or-test
"#,
        )
        .unwrap();

        match load_config(&path) {
            LoadConfigOutcome::Loaded(config) => {
                let p = &config.providers["openrouter"];
                assert_eq!(p.base_url, "https://openrouter.ai/api");
                assert_eq!(p.kind, ProviderKind::Generic);
                assert_eq!(p.api_key, "sk-or-test");
            }
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn known_local_providers_need_no_config_at_all() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(
            &path,
            r#"
providers:
  lmstudio: {}
  ollama: {}
"#,
        )
        .unwrap();

        match load_config(&path) {
            LoadConfigOutcome::Loaded(config) => {
                let lmstudio = &config.providers["lmstudio"];
                assert_eq!(lmstudio.base_url, "http://localhost:1234");
                assert_eq!(lmstudio.kind, ProviderKind::LmStudio);
                assert_eq!(lmstudio.api_key, "lm-studio");

                let ollama = &config.providers["ollama"];
                assert_eq!(ollama.base_url, "http://localhost:11434");
                assert_eq!(ollama.kind, ProviderKind::Ollama);
                assert_eq!(ollama.api_key, "ollama");
            }
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn explicit_values_override_the_known_provider_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(
            &path,
            r#"
providers:
  lmstudio:
    base_url: http://192.168.1.50:1234
    api_key: custom-key
    kind: generic
"#,
        )
        .unwrap();

        match load_config(&path) {
            LoadConfigOutcome::Loaded(config) => {
                let p = &config.providers["lmstudio"];
                assert_eq!(p.base_url, "http://192.168.1.50:1234", "explicit base_url must win");
                assert_eq!(p.api_key, "custom-key", "explicit api_key must win");
                assert_eq!(p.kind, ProviderKind::Generic, "explicit kind must win, even overriding to Generic");
            }
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn unknown_provider_needs_its_own_full_block() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(
            &path,
            r#"
providers:
  glm:
    base_url: https://open.bigmodel.cn/api/paas/v4
    api_key: my-key
"#,
        )
        .unwrap();

        match load_config(&path) {
            LoadConfigOutcome::Loaded(config) => {
                let p = &config.providers["glm"];
                assert_eq!(p.base_url, "https://open.bigmodel.cn/api/paas/v4");
                assert_eq!(p.kind, ProviderKind::Generic, "no known-provider entry, so plain default");
            }
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn missing_config_bootstraps_from_example_and_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("config.yaml");

        match load_config(&path) {
            LoadConfigOutcome::Bootstrapped(written) => assert_eq!(written, path),
            _ => panic!("expected Bootstrapped"),
        }
        assert!(path.exists());
        assert!(fs::read_to_string(&path).unwrap().contains("lmstudio"));

        // The bundled example itself must actually parse (kind: lmstudio/
        // ollama included) — not just exist on disk.
        match load_config(&path) {
            LoadConfigOutcome::Loaded(config) => {
                assert_eq!(config.providers["lmstudio"].kind, ProviderKind::LmStudio);
                assert_eq!(config.providers["ollama"].kind, ProviderKind::Ollama);
                assert_eq!(config.providers["openrouter"].kind, ProviderKind::Generic);
            }
            _ => panic!("expected the bootstrapped example config to reload as Loaded"),
        }
    }

    #[test]
    fn invalid_yaml_returns_parse_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "providers: [invalid: yaml: :").unwrap();

        match load_config(&path) {
            LoadConfigOutcome::ParseError(msg) => assert!(msg.contains("Invalid YAML")),
            _ => panic!("expected ParseError"),
        }
    }

    #[test]
    fn save_config_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let mut context_windows = IndexMap::new();
        context_windows.insert("model-a".to_string(), 1_000_000u64);
        let mut providers = IndexMap::new();
        providers.insert(
            "openrouter".to_string(),
            Provider {
                base_url: "https://openrouter.ai/api".to_string(),
                api_key: "sk-or-test".to_string(),
                models: vec!["model-a".to_string()],
                context_windows,
                ..Default::default()
            },
        );
        let config = Config { providers };

        save_config(&config, &path);

        match load_config(&path) {
            LoadConfigOutcome::Loaded(reloaded) => assert_eq!(reloaded.providers, config.providers),
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn load_last_returns_default_when_file_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last.yaml");
        assert_eq!(load_last(&path), Last::default());
    }

    #[test]
    fn load_last_returns_default_on_corrupt_yaml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("last.yaml");
        fs::write(&path, ": invalid :").unwrap();
        assert_eq!(load_last(&path), Last::default());
    }

    #[test]
    fn save_last_round_trips_and_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("last.yaml");
        let mut recent_models = IndexMap::new();
        recent_models.insert("openrouter".to_string(), vec!["model-a".to_string(), "model-b".to_string()]);
        let last = Last {
            provider: Some("groq".to_string()),
            model: Some("llama-3.1-8b-instant".to_string()),
            rtk_enabled: true,
            auto_accept: false,
            recent_models,
        };

        save_last(&last, &path);

        assert_eq!(load_last(&path), last);
    }
}
