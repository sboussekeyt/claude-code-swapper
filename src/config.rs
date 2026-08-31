use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Provider {
    pub base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub providers: IndexMap<String, Provider>,
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
    match serde_yaml_ng::from_str::<Config>(&text) {
        Ok(config) => LoadConfigOutcome::Loaded(config),
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
    fn missing_config_bootstraps_from_example_and_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("config.yaml");

        match load_config(&path) {
            LoadConfigOutcome::Bootstrapped(written) => assert_eq!(written, path),
            _ => panic!("expected Bootstrapped"),
        }
        assert!(path.exists());
        assert!(fs::read_to_string(&path).unwrap().contains("lmstudio"));
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
        let mut providers = IndexMap::new();
        providers.insert(
            "openrouter".to_string(),
            Provider {
                base_url: "https://openrouter.ai/api".to_string(),
                api_key: "sk-or-test".to_string(),
                models: vec!["model-a".to_string()],
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
        let last = Last {
            provider: Some("groq".to_string()),
            model: Some("llama-3.1-8b-instant".to_string()),
            rtk_enabled: true,
            auto_accept: false,
        };

        save_last(&last, &path);

        assert_eq!(load_last(&path), last);
    }
}
