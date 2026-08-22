# Rust + TUI Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Python `claude-code-swapper` with a single self-contained Rust binary offering a full-screen ratatui dashboard, at full feature parity.

**Architecture:** Single binary crate, six modules by responsibility (`config`, `discovery`, `launcher`, `app`, `ui`, `event`). All business logic lives in pure, I/O-free methods on `AppState` (`app.rs`); `ui.rs` only reads that state to draw; `event.rs` is the only place that performs I/O (config save, HTTP discovery) in response to input, wiring the pure state machine to the outside world.

**Tech Stack:** `ratatui` + `crossterm` (TUI), `ureq` (sync HTTP), `serde` + `serde_yaml_ng` (config), `indexmap` (order-preserving provider map), `which` (PATH lookup), `dirs` (home dir resolution only — see constraint below), `tempfile` + `tiny_http` (dev-dependencies for tests). The spec's suggested `tui-input` crate is dropped — see Task 5's deviation note.

**Spec:** `docs/superpowers/specs/2026-08-22-rust-tui-rewrite-design.md`

## Global Constraints

- Config path is `{home}/.config/claude-code-swapper/` via `dirs::home_dir()` joined manually with `.config/claude-code-swapper` — **do not use `dirs::config_dir()`**, which resolves to `~/Library/Application Support` on macOS and would silently stop reading the user's existing `~/.config/claude-code-swapper/config.yaml` (already populated and just fixed this session for the `lmstudio` provider's `base_url`).
- No function outside `main.rs` calls `std::process::exit` or performs process replacement (`exec`). Every fallible/exit-worthy operation returns a `Result`/enum that `main.rs` interprets — this is what keeps everything except `main.rs` and the terminal-lifecycle parts of it unit-testable, mirroring how the Python suite mocked `sys.exit`/`os.execvpe` at the boundary.
- `app.rs` performs no I/O (no file reads/writes, no network, no process spawning). Every mutation needed by config persistence or HTTP discovery is triggered from `event.rs`, which owns the config/last file paths and calls `config::save_config` / `config::save_last` / `discovery::fetch_remote_models` itself.
- Model discovery uses a 1.5s timeout and silently falls back to the provider's static `models` list on any failure (unreachable host, timeout, malformed JSON, empty `data`) — identical contract to the Python `fetch_remote_models` fixed this session.
- `base_url` values are always the bare provider root (no trailing `/v1`) — this is what the Python-side bugfix this session established; the bundled example config and any docs must reflect it.

---

## Task 1: Crate scaffold + `config` module

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs` (stub — real wiring happens in Task 8)
- Create: `src/config.rs`
- Create: `assets/config.example.yaml`

**Interfaces:**
- Produces: `pub struct Provider { pub base_url: String, pub api_key: String, pub models: Vec<String> }` (derives `Serialize, Deserialize, Clone, Debug, PartialEq`)
- Produces: `pub struct Config { pub providers: indexmap::IndexMap<String, Provider> }` (derives `Serialize, Deserialize, Clone, Debug, Default`, `#[serde(default)]` on `providers`)
- Produces: `pub struct Last { pub provider: Option<String>, pub model: Option<String>, pub rtk_enabled: bool, pub auto_accept: bool }` (derives `Serialize, Deserialize, Clone, Debug, Default, PartialEq`, `#[serde(default)]` on every field)
- Produces: `pub enum LoadConfigOutcome { Loaded(Config), Bootstrapped(PathBuf), ParseError(String) }`
- Produces: `pub fn load_config(path: &Path) -> LoadConfigOutcome`
- Produces: `pub fn save_config(config: &Config, path: &Path)`
- Produces: `pub fn load_last(path: &Path) -> Last`
- Produces: `pub fn save_last(last: &Last, path: &Path)`
- Produces: `pub const CONFIG_DIR_NAME: &str = "claude-code-swapper";` and `pub fn config_dir() -> PathBuf` (uses `dirs::home_dir()` per the Global Constraint)

- [ ] **Step 1: Scaffold the crate**

```bash
cd /Users/sboussekeyt/Projects/claude-code-swapper
cargo init --name claude-code-swapper
cargo add serde --features derive
cargo add serde_yaml_ng
cargo add indexmap --features serde
cargo add dirs
cargo add which
cargo add --dev tempfile
```

- [ ] **Step 2: Create the bundled example config**

Create `assets/config.example.yaml`:

```yaml
providers:
  openrouter:
    base_url: https://openrouter.ai/api
    api_key: sk-or-REPLACE_ME
    models:
      - anthropic/claude-sonnet-4-6
      - deepseek/deepseek-chat-v3-0324
      - meta-llama/llama-3.1-8b-instruct
      - mistralai/mistral-7b-instruct

  groq:
    base_url: https://api.groq.com/openai
    api_key: gsk_REPLACE_ME
    models:
      - llama-3.1-8b-instant
      - mixtral-8x7b-32768

  lmstudio:
    base_url: http://localhost:1234 # no trailing /v1 — claude appends /v1/messages itself
    api_key: lm-studio
    models:
      - local-model # replace with the model identifier loaded in LM Studio
```

- [ ] **Step 3: Write the failing tests**

Create `src/config.rs` starting with just the test module (implementation comes in later steps):

```rust
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

pub fn load_config(_path: &Path) -> LoadConfigOutcome {
    todo!()
}

pub fn save_config(_config: &Config, _path: &Path) {
    todo!()
}

pub fn load_last(_path: &Path) -> Last {
    todo!()
}

pub fn save_last(_last: &Last, _path: &Path) {
    todo!()
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
```

Wire the module into `src/main.rs`:

```rust
mod config;

fn main() {}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test --lib config::`
Expected: compile failure or panics from the `todo!()` bodies (e.g. `not yet implemented`), not `Loaded`/`Bootstrapped`/etc mismatches.

- [ ] **Step 5: Implement `load_config`, `save_config`, `load_last`, `save_last`**

Replace the four `todo!()` bodies in `src/config.rs`:

```rust
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
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --lib config::`
Expected: all 7 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs src/config.rs assets/config.example.yaml
git commit -m "feat: scaffold Rust crate, add config module with YAML load/save"
```

---

## Task 2: `discovery` module

**Files:**
- Create: `src/discovery.rs`

**Interfaces:**
- Consumes: nothing from other modules
- Produces: `pub fn fetch_remote_models(base_url: &str, api_key: &str, timeout: std::time::Duration) -> Option<Vec<String>>`

- [ ] **Step 1: Add the HTTP dependency**

```bash
cargo add ureq --features json
cargo add --dev tiny_http
```

- [ ] **Step 2: Write the failing tests**

Create `src/discovery.rs`:

```rust
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

pub fn fetch_remote_models(_base_url: &str, _api_key: &str, _timeout: Duration) -> Option<Vec<String>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use tiny_http::{Response, Server};

    /// Starts a one-shot local HTTP server, returns (base_url, received_path, received_auth_header)
    /// via a channel once the single expected request has been handled.
    fn serve_once(body: &'static str) -> (String, mpsc::Receiver<(String, Option<String>)>) {
        let server = Server::http("127.0.0.1:0").unwrap();
        let addr = server.server_addr().to_string();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if let Ok(request) = server.recv() {
                let path = request.url().to_string();
                let auth = request
                    .headers()
                    .iter()
                    .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case("Authorization"))
                    .map(|h| h.value.as_str().to_string());
                let _ = tx.send((path, auth));
                let _ = request.respond(Response::from_string(body));
            }
        });
        (format!("http://{addr}"), rx)
    }

    #[test]
    fn returns_sorted_model_ids_on_success() {
        let (base_url, _rx) = serve_once(r#"{"data":[{"id":"b-model"},{"id":"a-model"}]}"#);
        let result = fetch_remote_models(&base_url, "lm-studio", Duration::from_secs(2));
        assert_eq!(result, Some(vec!["a-model".to_string(), "b-model".to_string()]));
    }

    #[test]
    fn requests_v1_models_with_authorization_header() {
        let (base_url, rx) = serve_once(r#"{"data":[{"id":"m"}]}"#);
        fetch_remote_models(&base_url, "my-key", Duration::from_secs(2));
        let (path, auth) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(path, "/v1/models");
        assert_eq!(auth, Some("Bearer my-key".to_string()));
    }

    #[test]
    fn strips_trailing_slash_from_base_url() {
        let (base_url, rx) = serve_once(r#"{"data":[{"id":"m"}]}"#);
        let base_url_with_slash = format!("{base_url}/");
        fetch_remote_models(&base_url_with_slash, "key", Duration::from_secs(2));
        let (path, _) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(path, "/v1/models");
    }

    #[test]
    fn returns_none_on_connection_refused() {
        // Port 1 is reserved and nothing listens there.
        let result = fetch_remote_models("http://127.0.0.1:1", "key", Duration::from_millis(500));
        assert_eq!(result, None);
    }

    #[test]
    fn returns_none_on_invalid_json() {
        let (base_url, _rx) = serve_once("not json");
        let result = fetch_remote_models(&base_url, "key", Duration::from_secs(2));
        assert_eq!(result, None);
    }

    #[test]
    fn returns_none_when_data_empty() {
        let (base_url, _rx) = serve_once(r#"{"data":[]}"#);
        let result = fetch_remote_models(&base_url, "key", Duration::from_secs(2));
        assert_eq!(result, None);
    }
}
```

Add `mod discovery;` to `src/main.rs`.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib discovery::`
Expected: panics from `todo!()`.

- [ ] **Step 4: Implement `fetch_remote_models`**

```rust
pub fn fetch_remote_models(base_url: &str, api_key: &str, timeout: Duration) -> Option<Vec<String>> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let response = agent
        .get(&url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .call()
        .ok()?;
    let parsed: ModelsResponse = response.into_json().ok()?;
    if parsed.data.is_empty() {
        return None;
    }
    let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    Some(ids)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib discovery::`
Expected: all 6 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs src/discovery.rs
git commit -m "feat: add discovery module for OpenAI-compatible /v1/models lookup"
```

---

## Task 3: `launcher` module

**Files:**
- Create: `src/launcher.rs`

**Interfaces:**
- Consumes: nothing from other modules
- Produces: `pub fn build_env(base_url: &str, api_key: &str) -> HashMap<String, String>`
- Produces: `pub fn build_command(model: Option<&str>, auto_accept: bool, env: &HashMap<String, String>) -> std::process::Command`
- Produces: `pub fn check_claude() -> Result<(), String>`
- Produces: `pub fn check_rtk_installed() -> bool`
- Produces: `pub const RTK_INSTALL_CMD: &str`
- Produces: `pub fn install_rtk()`
- Produces: `pub fn ensure_rtk_hook()`

- [ ] **Step 1: Write the failing tests**

Create `src/launcher.rs`:

```rust
use std::collections::HashMap;
use std::process::Command;

pub fn build_env(_base_url: &str, _api_key: &str) -> HashMap<String, String> {
    todo!()
}

pub fn build_command(_model: Option<&str>, _auto_accept: bool, _env: &HashMap<String, String>) -> Command {
    todo!()
}

pub fn check_claude() -> Result<(), String> {
    todo!()
}

pub fn check_rtk_installed() -> bool {
    todo!()
}

pub const RTK_INSTALL_CMD: &str =
    "curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh";

pub fn install_rtk() {
    todo!()
}

pub fn ensure_rtk_hook() {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_env_sets_proxy_vars_and_clears_api_key() {
        let env = build_env("https://openrouter.ai/api", "sk-or-test");
        assert_eq!(env["ANTHROPIC_BASE_URL"], "https://openrouter.ai/api");
        assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], "sk-or-test");
        assert_eq!(env["ANTHROPIC_API_KEY"], "");
    }

    #[test]
    fn build_command_sets_program_and_model_flag() {
        let env = HashMap::new();
        let cmd = build_command(Some("claude-sonnet-4-6"), false, &env);
        assert_eq!(cmd.get_program(), "claude");
        let args: Vec<_> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert_eq!(args, vec!["--model", "claude-sonnet-4-6"]);
    }

    #[test]
    fn build_command_adds_auto_accept_flag() {
        let env = HashMap::new();
        let cmd = build_command(Some("model"), true, &env);
        let args: Vec<_> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert!(args.contains(&"--dangerously-skip-permissions"));
    }

    #[test]
    fn build_command_omits_model_flag_when_none() {
        let env = HashMap::new();
        let cmd = build_command(None, false, &env);
        assert_eq!(cmd.get_args().count(), 0);
    }

    #[test]
    fn build_command_carries_exact_env_map() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_BASE_URL".to_string(), "https://example.com".to_string());
        let cmd = build_command(None, false, &env);
        let carried: HashMap<String, String> = cmd
            .get_envs()
            .filter_map(|(k, v)| Some((k.to_str()?.to_string(), v?.to_str()?.to_string())))
            .collect();
        assert_eq!(carried, env);
    }

    #[test]
    fn check_rtk_installed_reflects_path() {
        // `cargo` itself is guaranteed to be on PATH in the test environment;
        // this is a smoke test that the PATH-lookup mechanism works at all.
        assert_eq!(check_rtk_installed(), which::which("rtk").is_ok());
    }
}
```

Add `mod launcher;` to `src/main.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib launcher::`
Expected: panics from `todo!()`.

- [ ] **Step 3: Implement the launcher functions**

```rust
pub fn build_env(base_url: &str, api_key: &str) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    env.insert("ANTHROPIC_BASE_URL".to_string(), base_url.to_string());
    env.insert("ANTHROPIC_AUTH_TOKEN".to_string(), api_key.to_string());
    env.insert("ANTHROPIC_API_KEY".to_string(), String::new());
    env
}

pub fn build_command(model: Option<&str>, auto_accept: bool, env: &HashMap<String, String>) -> Command {
    let mut cmd = Command::new("claude");
    if let Some(m) = model {
        cmd.arg("--model").arg(m);
    }
    if auto_accept {
        cmd.arg("--dangerously-skip-permissions");
    }
    cmd.env_clear();
    cmd.envs(env);
    cmd
}

pub fn check_claude() -> Result<(), String> {
    if which::which("claude").is_err() {
        return Err("'claude' not found in PATH — is Claude Code installed?".to_string());
    }
    Ok(())
}

pub fn check_rtk_installed() -> bool {
    which::which("rtk").is_ok()
}

pub fn install_rtk() {
    let _ = Command::new("sh").arg("-c").arg(RTK_INSTALL_CMD).status();
}

pub fn ensure_rtk_hook() {
    let _ = Command::new("rtk")
        .args(["init", "--global", "--auto-patch"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib launcher::`
Expected: all 6 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs src/launcher.rs
git commit -m "feat: add launcher module (env/command building, claude/rtk PATH checks)"
```

---

## Task 4: `app` module — navigation & selection

**Files:**
- Create: `src/app.rs`

**Interfaces:**
- Consumes: `config::{Config, Provider, Last}` (Task 1)
- Produces: `pub enum Panel { Providers, Models }`
- Produces: `pub struct AppState { pub providers: Vec<String>, pub focused_panel: Panel, pub provider_cursor: usize, pub model_cursor: usize, pub models_for_focused_provider: Vec<String>, pub current_provider: Option<String>, pub current_model: Option<String>, pub rtk_enabled: bool, pub auto_accept: bool, pub config: Config, pub modal: Option<Modal>, pub status_message: Option<String> }`
- Produces: `AppState::new(config: Config, last: &Last) -> AppState`
- Produces: `AppState::focused_provider(&self) -> Option<&str>`
- Produces: `AppState::switch_focus(&mut self)`
- Produces: `AppState::move_cursor(&mut self, delta: i32)`
- Produces: `AppState::set_focused_provider_models(&mut self, models: Vec<String>)`
- Produces: `AppState::refresh_focused_provider_models(&mut self)`
- Produces: `AppState::apply_selection(&mut self) -> bool`
- Produces: `AppState::can_launch(&self) -> bool`
- Note: `Modal` is declared here as an empty placeholder-free enum with the two variants used starting Task 5 — declaring it now (rather than `todo!`) keeps `AppState`'s field list stable across tasks.

- [ ] **Step 1: Write the failing tests**

Create `src/app.rs`:

```rust
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
        todo!()
    }

    pub fn focused_provider(&self) -> Option<&str> {
        todo!()
    }

    pub fn switch_focus(&mut self) {
        todo!()
    }

    pub fn move_cursor(&mut self, delta: i32) {
        todo!()
    }

    pub fn set_focused_provider_models(&mut self, models: Vec<String>) {
        todo!()
    }

    pub fn refresh_focused_provider_models(&mut self) {
        todo!()
    }

    pub fn apply_selection(&mut self) -> bool {
        todo!()
    }

    pub fn can_launch(&self) -> bool {
        todo!()
    }
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
}
```

Add `mod app;` to `src/main.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib app::`
Expected: panics from `todo!()`.

- [ ] **Step 3: Implement the navigation/selection methods**

```rust
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
}

fn clamp_cursor(cursor: usize, delta: i32, len: usize) -> usize {
    let new = cursor as i32 + delta;
    new.clamp(0, len as i32 - 1) as usize
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib app::`
Expected: all 10 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/app.rs
git commit -m "feat: add AppState navigation and selection logic"
```

---

## Task 5: `app` module — model management & modals

**Files:**
- Modify: `src/app.rs`

**Interfaces:**
- Consumes: `AppState` fields from Task 4
- Produces: `AppState::add_model(&mut self, provider: &str, model_name: &str) -> bool`
- Produces: `AppState::remove_model(&mut self, provider: &str, model_name: &str)`
- Produces: `AppState::remove_focused_model(&mut self)`
- Produces: `AppState::set_api_key(&mut self, provider: &str, key: &str) -> bool`
- Produces: `AppState::toggle_rtk(&mut self)`
- Produces: `AppState::toggle_auto_accept(&mut self)`
- Produces: `AppState::open_add_model_modal(&mut self)`
- Produces: `AppState::open_set_api_key_modal(&mut self)`
- Produces: `AppState::modal_input_char(&mut self, c: char)`
- Produces: `AppState::modal_backspace(&mut self)`
- Produces: `AppState::close_modal(&mut self)`
- Produces: `AppState::confirm_modal(&mut self)`
- Deviation from spec: the spec's stack table suggested `tui-input` to avoid hand-rolling cursor/editing logic. Both modal inputs here (a model name, an API key) are append-only single-line entries with no need for mid-string cursor movement, so `modal_input_char`/`modal_backspace` hand-roll a plain `String::push`/`String::pop` instead — simpler than wiring up a whole crate for editing behavior that's never exercised. `tui-input` is not added as a dependency in this plan.

- [ ] **Step 1: Write the failing tests**

Append to the `#[cfg(test)] mod tests` block in `src/app.rs` (keep the existing tests from Task 4 in place):

```rust
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
```

Add the method stubs (all `todo!()`) inside the existing `impl AppState` block from Task 4.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib app::`
Expected: panics from `todo!()` on the new tests; Task 4 tests still pass.

- [ ] **Step 3: Implement the model management and modal methods**

Add to the `impl AppState` block:

```rust
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
            Some(Modal::SetApiKey { provider, input }) if !input.is_empty() => {
                self.set_api_key(&provider, &input);
            }
            _ => {}
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib app::`
Expected: all 19 tests (10 from Task 4 + 9 new) PASS.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: add model management and modal input handling to AppState"
```

---

## Task 6: `ui` module

**Files:**
- Create: `src/ui.rs`

**Interfaces:**
- Consumes: `app::{AppState, Panel, Modal}` (Tasks 4-5)
- Produces: `pub fn render(frame: &mut ratatui::Frame, state: &AppState)`

- [ ] **Step 1: Add the TUI dependencies**

```bash
cargo add ratatui
cargo add crossterm
```

- [ ] **Step 2: Write the smoke test**

Create `src/ui.rs`:

```rust
use crate::app::{AppState, Modal, Panel};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, state: &AppState) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Last, Provider};
    use indexmap::IndexMap;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_does_not_panic() {
        let mut providers = IndexMap::new();
        providers.insert(
            "lmstudio".to_string(),
            Provider {
                base_url: "http://localhost:1234".to_string(),
                api_key: "lm-studio".to_string(),
                models: vec!["local-model".to_string()],
            },
        );
        let state = AppState::new(Config { providers }, &Last::default());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
    }

    #[test]
    fn render_with_open_modal_does_not_panic() {
        let mut providers = IndexMap::new();
        providers.insert(
            "lmstudio".to_string(),
            Provider {
                base_url: "http://localhost:1234".to_string(),
                api_key: "lm-studio".to_string(),
                models: vec!["local-model".to_string()],
            },
        );
        let mut state = AppState::new(Config { providers }, &Last::default());
        state.open_add_model_modal();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
    }
}
```

Add `mod ui;` to `src/main.rs`.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib ui::`
Expected: panics from `todo!()`.

- [ ] **Step 4: Implement `render`**

```rust
pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    render_status(frame, state, chunks[0]);
    render_panels(frame, state, chunks[1]);
    render_footer(frame, chunks[2]);

    if let Some(modal) = &state.modal {
        render_modal(frame, modal, area);
    }
}

fn render_status(frame: &mut Frame, state: &AppState, area: Rect) {
    let provider = state.current_provider.as_deref().unwrap_or("-");
    let model = state.current_model.as_deref().unwrap_or("-");
    let text = format!(
        "Provider: {provider}   Model: {model}   RTK: {}   Auto-accept: {}",
        if state.rtk_enabled { "ON" } else { "OFF" },
        if state.auto_accept { "ON" } else { "OFF" }
    );
    let block = Block::default().title("Claude Code Swapper").borders(Borders::ALL);
    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn render_panels(frame: &mut Frame, state: &AppState, area: Rect) {
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let provider_items: Vec<ListItem> = state
        .providers
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let style = if i == state.provider_cursor {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(p.as_str())).style(style)
        })
        .collect();
    let providers_border = if state.focused_panel == Panel::Providers {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    frame.render_widget(
        List::new(provider_items).block(
            Block::default()
                .title("Providers")
                .borders(Borders::ALL)
                .border_style(providers_border),
        ),
        panels[0],
    );

    let model_items: Vec<ListItem> = state
        .models_for_focused_provider
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let style = if i == state.model_cursor {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(m.as_str())).style(style)
        })
        .collect();
    let models_border = if state.focused_panel == Panel::Models {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    frame.render_widget(
        List::new(model_items).block(
            Block::default()
                .title("Models")
                .borders(Borders::ALL)
                .border_style(models_border),
        ),
        panels[1],
    );
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let text = "[Tab] switch  [↑/↓] move  [Enter] select  [l] launch  [n] native  [a] add  [x] remove  [s] api key  [r] rtk  [p] auto-accept  [q] quit";
    frame.render_widget(Paragraph::new(text), area);
}

fn render_modal(frame: &mut Frame, modal: &Modal, area: Rect) {
    let (title, provider, input) = match modal {
        Modal::AddModel { provider, input } => ("Add model", provider, input),
        Modal::SetApiKey { provider, input } => ("Set API key", provider, input),
    };
    let width = area.width.min(60);
    let height = 3;
    let popup = Rect {
        x: (area.width.saturating_sub(width)) / 2,
        y: (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    let masked = match modal {
        Modal::SetApiKey { .. } => "*".repeat(input.len()),
        _ => input.clone(),
    };
    let block = Block::default()
        .title(format!("{title} ({provider})"))
        .borders(Borders::ALL);
    frame.render_widget(Paragraph::new(masked).block(block), popup);
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib ui::`
Expected: both tests PASS.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs src/ui.rs
git commit -m "feat: add ratatui dashboard rendering"
```

---

## Task 7: `event` module

**Files:**
- Create: `src/event.rs`

**Interfaces:**
- Consumes: `app::AppState` (Tasks 4-5), `config::{save_config, save_last, Last}` (Task 1), `discovery::fetch_remote_models` (Task 2)
- Produces: `pub enum Action { Continue, Launch, LaunchNative, Quit }`
- Produces: `pub fn refresh_discovery(state: &mut AppState)`
- Produces: `pub fn handle_key(state: &mut AppState, key: crossterm::event::KeyEvent, config_path: &Path, last_path: &Path) -> Action`

- [ ] **Step 1: Write the failing tests**

Create `src/event.rs`:

```rust
use crate::app::AppState;
use crate::config;
use crate::discovery;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::Path;
use std::time::Duration;

pub enum Action {
    Continue,
    Launch,
    LaunchNative,
    Quit,
}

pub fn refresh_discovery(_state: &mut AppState) {
    todo!()
}

pub fn handle_key(_state: &mut AppState, _key: KeyEvent, _config_path: &Path, _last_path: &Path) -> Action {
    todo!()
}

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
```

Add `mod event;` to `src/main.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib event::`
Expected: panics from `todo!()`.

- [ ] **Step 3: Implement `refresh_discovery` and `handle_key`**

```rust
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
        KeyCode::Char('r') => state.toggle_rtk(),
        KeyCode::Char('p') => state.toggle_auto_accept(),
        KeyCode::Enter => {
            if state.apply_selection() {
                let last = Last {
                    provider: state.current_provider.clone(),
                    model: state.current_model.clone(),
                    rtk_enabled: state.rtk_enabled,
                    auto_accept: state.auto_accept,
                };
                config::save_last(&last, last_path);
            }
        }
        KeyCode::Char('l') if state.can_launch() => return Action::Launch,
        KeyCode::Char('n') => return Action::LaunchNative,
        KeyCode::Char('q') | KeyCode::Esc => return Action::Quit,
        _ => {}
    }
    Action::Continue
}
```

Add the missing import at the top of `src/event.rs`: `use crate::config::Last;`

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib event::`
Expected: all 7 tests PASS.

- [ ] **Step 5: Run the full test suite**

Run: `cargo test`
Expected: all tests across `config`, `discovery`, `launcher`, `app`, `ui`, `event` PASS (should be 47 tests total: 7 config + 6 discovery + 6 launcher + 19 app + 2 ui + 7 event — count what's actually there and confirm none are skipped).

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/event.rs
git commit -m "feat: add event module wiring keyboard input to AppState, config persistence, and discovery"
```

---

## Task 8: `main.rs` wiring

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: everything from Tasks 1-7
- Produces: the runnable binary

- [ ] **Step 1: Write `main.rs`**

Replace the contents of `src/main.rs`:

```rust
mod app;
mod config;
mod discovery;
mod event;
mod launcher;
mod ui;

use app::AppState;
use crossterm::event::Event;
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

    enable_raw_mode().expect("failed to enable raw mode");
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).expect("failed to enter alternate screen");

    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        default_panic(info);
    }));

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("failed to create terminal");

    let action = loop {
        terminal.draw(|frame| ui::render(frame, &state)).expect("failed to draw frame");

        let Event::Key(key) = event::crossterm_read() else {
            continue;
        };
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
            let env: std::collections::HashMap<String, String> = std::env::vars().collect();
            let mut cmd = launcher::build_command(None, state.auto_accept, &env);
            let err = cmd.exec();
            eprintln!("failed to launch claude: {err}");
            std::process::exit(1);
        }
        event::Action::Quit => {}
        event::Action::Continue => unreachable!(),
    }
}
```

- [ ] **Step 2: Add the blocking key-read helper to `event.rs`**

`crossterm::event::read()` blocks until an event arrives and returns `Result<Event, io::Error>`; wrap it so `main.rs` stays simple. This is also why `main.rs` imports only `crossterm::event::Event` and never `crossterm::event::{self, ...}` — our own crate already declares `mod event;` (`src/event.rs`), so importing the crossterm `event` module under the same local name in `main.rs` would collide with it (`the name 'event' is defined multiple times`). Inside `src/event.rs` itself there's no such collision (the file *is* the `event` module), so it can import `crossterm::event::{self, ...}` freely. Add to `src/event.rs`:

```rust
pub fn crossterm_read() -> Event {
    loop {
        if let Ok(ev) = event::read() {
            return ev;
        }
    }
}
```

Add `use crossterm::event::{self, Event};` to the top of `src/event.rs` (alongside the existing `crossterm::event::{KeyCode, KeyEvent, KeyModifiers}` import — merge into one `use` line).

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: builds with no errors. Warnings about unused items are acceptable at this point only if they point at genuinely dead code to be wired up in a later step — otherwise fix them.

- [ ] **Step 4: Run the full automated test suite**

Run: `cargo test`
Expected: all tests still PASS (the `main.rs` wiring itself has no automated tests — the terminal lifecycle and `exec` are only verified manually in Task 9, matching the spec's testing strategy).

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/event.rs
git commit -m "feat: wire main.rs — terminal lifecycle, panic hook, RTK prompt, launch"
```

---

## Task 9: Manual end-to-end verification

**Files:** none (verification only)

- [ ] **Step 1: Build the release binary**

```bash
cargo build --release
```

- [ ] **Step 2: Run against the real config**

```bash
./target/release/claude-code-swapper
```

Verify manually, against the user's real `~/.config/claude-code-swapper/config.yaml` (already containing `anthropic`, `glm`, `groq`, `lmstudio`, `minimax`, `openrouter`):

- Dashboard renders with all 6 providers listed, in the same order as the YAML file.
- Moving the Providers-panel cursor to `lmstudio` populates the Models panel with `google/gemma-4-12b` and `text-embedding-nomic-embed-text-v1.5` (requires LM Studio's local server running on port 1234, per this session's earlier fix) — falls back to the static `local-model` entry if the server isn't running.
- `a` on `lmstudio` opens an add-model modal; typing a name and pressing Enter adds it to both the in-memory state and `~/.config/claude-code-swapper/config.yaml` on disk.
- `x` removes the focused model from the same file.
- `s` on a provider opens a masked API-key input; confirming updates `config.yaml`.
- `r` and `p` toggle RTK/auto-accept and are reflected in the status bar immediately.
- `Enter` on `lmstudio` + a model, then `l`, exits the TUI cleanly and launches `claude` with `ANTHROPIC_BASE_URL=http://localhost:1234`, `--model <selected>` — confirm no `API returned an empty or malformed response` error, i.e. that the double-`/v1` bug fixed on the Python side this session has no equivalent in the new binary (verified structurally: `build_env` never appends `/v1`, and the bundled example config's `lmstudio` entry has no trailing `/v1`).
- `n` launches `claude` natively (no `ANTHROPIC_*` env vars set) — check with `echo $ANTHROPIC_BASE_URL` inside the launched session, expect empty.
- Kill the process mid-render (e.g. `kill -TERM` from another terminal) or force a panic by temporarily inserting `panic!()` in `event::handle_key` and rebuilding — confirm the terminal is left in a normal, usable state afterward (not stuck in raw mode). Revert the temporary panic before continuing.
- Quitting with `q` returns to a normal shell prompt with the terminal in its original state.

- [ ] **Step 2: Record the outcome**

If any check fails, treat it as a bug against the specific task that owns the broken behavior — fix there via its existing test suite, don't patch around it in `main.rs`. Do not proceed to Task 10 until every check above passes.

---

## Task 10: Cutover — remove the Python implementation

**Files:**
- Delete: `claude_code_swapper/` (entire directory)
- Delete: `tests/test_main.py`
- Modify: `pyproject.toml` → delete
- Modify: `README.md`

- [ ] **Step 1: Remove the Python package and its tests**

```bash
git rm -r claude_code_swapper tests pyproject.toml
```

- [ ] **Step 2: Rewrite `README.md` for the Rust binary**

Replace the "Install", "Usage", "Dev install" sections to reference `cargo install --path .` and `cargo test`, keeping the "Config" section's YAML example and the `base_url`-without-`/v1` note from this session's fix. Keep the RTK section as-is (behavior unchanged).

- [ ] **Step 3: Verify the crate still builds and tests still pass after the removal**

```bash
cargo build --release && cargo test
```

Expected: unaffected — the Python files were never referenced by the Rust build.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "chore: remove Python implementation, Rust binary is now the only implementation"
```
