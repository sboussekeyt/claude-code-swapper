# claude-code-swapper (Rust + TUI)

Full-screen `ratatui` TUI, single self-contained Rust binary, for launching Claude Code
against different LLM providers (OpenRouter, Groq, LM Studio, Ollama, etc.) via
`ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN`. This is the Rust rewrite of the original
Python `claude_code_swapper` package; the Python implementation has been removed.

## Commands

```bash
cargo build --release   # build the binary (target/release/claude-code-swapper)
cargo test               # run the full test suite
cargo clippy --all-targets   # lint
cargo install --path .   # install for personal use
```

## Modules (`src/`)

| Module | Responsibility |
|---|---|
| `config.rs` | Load/save `config.yaml` + `last.yaml`; bootstraps `config.yaml` from `assets/config.example.yaml` on first run; merges a provider's config against `known_providers.rs` |
| `known_providers.rs` | The built-in registry (`base_url`/`kind`/`default_api_key` per well-known provider name) so a config only needs to state what's actually the user's own — see Known providers below |
| `discovery.rs` | `ProviderKind` (`Generic`/`LmStudio`/`Ollama`) + the `ModelSource` trait impls behind it — how each provider kind's available models are fetched (`ureq`, 1.5s timeout, silent fallback on any failure) |
| `launcher.rs` | `build_env`/`build_command`, `check_claude`, RTK install/hook helpers — all process/PATH I/O |
| `app.rs` | `AppState` and all state transitions — pure, no I/O, no terminal handle |
| `ui.rs` | `ratatui` rendering — pure functions of `AppState` -> widgets |
| `event.rs` | Translates crossterm key events into `AppState` mutations / `Action`s |
| `main.rs` | Wiring only: load config, terminal setup/teardown, panic hook, main loop, `exec()` into `claude` |

## Global constraints

- All I/O (filesystem, network, process spawn, terminal) is confined to `main.rs`, `event.rs`,
  `launcher.rs`, `config.rs`, and `discovery.rs`.
- `app.rs` stays pure: no I/O, no terminal handle, only state transitions — this is what makes
  it unit-testable without a terminal or mocks.

## Config paths

- `~/.config/claude-code-swapper/config.yaml` (providers + api_keys + models)
- `~/.config/claude-code-swapper/last.yaml` (last provider/model/RTK/auto-accept)

Resolved via `dirs::home_dir()` joined with `.config/claude-code-swapper`, **not**
`dirs::config_dir()` — on macOS `dirs::config_dir()` resolves to
`~/Library/Application Support`, which is not where this tool's config lives (matches the
Python original's `Path.home() / ".config" / ...`).

## Known providers

`config.rs` deserializes YAML into a private `RawProvider` (`base_url`/`kind` as `Option`,
so "absent" is distinguishable from "explicitly set to the default") before resolving each
one into the public `Provider` type everything else reads. `RawProvider::resolve(name)` looks
`name` (the provider's key under `providers:`) up in `known_providers::KNOWN_PROVIDERS`: any
field the user left unset is filled from there; anything the user did set always wins. This
is what lets a config say just `openrouter: {api_key: ...}` or even `lmstudio: {}` instead of
repeating a `base_url`/`kind` that's the same on every machine — the point being a config file
you can hand to a teammate without them retyping infrastructure details, only their own key.

Adding a provider to the built-in list is one `KnownProvider` entry (name, `base_url`, `kind`,
optionally a `default_api_key` for services that don't check it, like LM Studio/Ollama) —
nothing else changes. This is orthogonal to adding a new discovery *strategy*: a `kind` still
needs a matching `ModelSource` impl in `discovery.rs` (see below) before a `KnownProvider` can
reference it; `known_providers.rs` only supplies the connection details, not the fetch logic.

Because `save_config` always serializes the fully-resolved `Provider` (not the raw/minimal
form), any action that saves config (`a`/`x`/`s` in the TUI) will write the resolved
`base_url`/`kind` back into the file on disk the first time it runs — a hand-written minimal
config stays minimal only until then. That's an accepted trade-off: the value is a fast start
for a new user or a config worth sharing, not a config that stays terse forever.

## Keybindings

| Key | Action |
|---|---|
| `Tab` | Switch focus between Providers and Models panels |
| `↑` / `↓` | Move cursor in the focused panel (also re-runs model discovery when moving in Providers) |
| `Enter` | Apply the focused provider+model as current selection, persists `last.yaml` |
| `l` | Launch Claude (proxy mode) |
| `n` | Launch Claude (native mode — no env override, no `--model`) |
| `a` | Add a model to the focused provider (opens modal) |
| `x` | Remove the focused model |
| `X` (Shift+X) | Unpin the focused model from Recents (no-op if it isn't one) |
| `s` | Set API key for the focused provider (opens masked modal) |
| `/` | Search/filter the Models panel (case-insensitive substring match) |
| `r` | Toggle RTK mode |
| `p` | Toggle auto-accept mode (`--dangerously-skip-permissions`) |
| `q` / `Esc` / `Ctrl+C` | Quit |
| (modal open) `Enter` / `Esc` / `Backspace` | Confirm / cancel / delete-last-char |

Ctrl-modified letter keys (`Ctrl+X`, `Ctrl+A`, `Ctrl+L`, etc.) are intentionally inert —
only the plain, unmodified key fires the action, so accidental chords can't trigger
destructive operations (e.g. `Ctrl+X` does not delete a model).

While search is active (`AppState::search_active`), all keys are intercepted before the
normal shortcut match: typed characters filter, `↑`/`↓` still move within the filtered
list, `Enter` selects and closes the search, `Esc` cancels it (instead of quitting).
`AppState::all_models_for_focused_provider` is the unfiltered source of truth;
`models_for_focused_provider` is what's on screen (filtered or not) — every place that
mutates the model list (`refresh_focused_provider_models`, `set_focused_provider_models`
via discovery) resets and closes any active search, and switching panel focus away from
Models does too, so search state can never point at a stale list.

## Gotchas

- **`base_url` must NOT include a trailing `/v1`.** `claude` appends `/v1/messages` itself;
  a `base_url` ending in `/v1` produces `/v1/v1/messages`, which most providers (including
  LM Studio) answer with a malformed 200 rather than a clean 404.
- `Command::exec()` (via `std::os::unix::process::CommandExt`) replaces the current process —
  there's no return from a successful launch. Unix-only, matches the previous
  `os.execvpe` behavior.
- The custom panic hook is installed **before** `enable_raw_mode()`/`EnterAlternateScreen`,
  so a panic during terminal setup itself still restores the terminal.
- Environment variables are read via `std::env::vars_os()` + lossy UTF-8 conversion rather
  than `std::env::vars()`, so a non-UTF-8 value in the inherited environment can't panic
  the whole program.
- Key events are filtered to `KeyEventKind::Press` — some terminals/platforms also emit
  `Release`/`Repeat` events, which must not double-fire actions.
- Model discovery is a trait, not a hardcoded call: `discovery::ModelSource` (`fn discover(&self,
  base_url, api_key, timeout) -> Option<Vec<DiscoveredModel>>`) has one impl per
  `discovery::ProviderKind` variant (`GenericOpenAi`, `LmStudioSource`, `OllamaSource`).
  `config::Provider.kind` (`#[serde(default)]`, so existing configs default to `Generic`
  unchanged) selects which one `event::refresh_discovery` calls via `cfg.kind.discover(...)`.
  Adding a new provider kind is: one new `ModelSource` impl + one new `ProviderKind` variant +
  one match arm in `ProviderKind::discover` — nothing else in `app.rs`/`event.rs`/`ui.rs`
  changes, since they only ever see the resulting `Vec<DiscoveredModel>`.
  - `GenericOpenAi` — plain `GET {base_url}/v1/models` (OpenRouter, Groq, most hosted APIs).
    OpenRouter's response includes `context_length` even though it's not part of the strict
    OpenAI schema; other generic providers just leave it absent.
  - `LmStudioSource` — LM Studio's native `GET {base_url}/api/v0/models`, which reports every
    *downloaded* model (not just the currently loaded one) plus `max_context_length`. Falls
    back to `GenericOpenAi`'s `/v1/models` on any failure (network error, non-2xx, unparseable
    JSON, or an empty result) — safe even if LM Studio's native schema changes or an older
    version doesn't have that endpoint.
  - `OllamaSource` — Ollama's native `GET {base_url}/api/tags`, which lists every locally
    *pulled* model. No context length in that response, so it's always `None` for Ollama —
    only a manual `config::Provider.context_windows` entry can set one.
- Context window resolution works around Claude Code assuming a 200k window for models it
  doesn't recognize by name — two sources, discovered wins:
  - `discovery::DiscoveredModel.context_length` (see above). `event::refresh_discovery` calls
    `AppState::set_discovered_models`, which captures it into
    `AppState::discovered_context_windows` (`IndexMap<String, u64>`), reset by
    `replace_focused_provider_models` on every provider switch/re-discovery so it can't go
    stale.
  - `config::Provider.context_windows` (`IndexMap<String, u64>`, keyed by model name, empty by
    default via `#[serde(default)]`) — the manual fallback for providers that don't report it.
  - Both are looked up in `main.rs`'s `Action::Launch` arm (`discovered_context_windows` first,
    falling back to `context_windows`) and passed to `launcher::build_env`, which sets
    `CLAUDE_CODE_MAX_CONTEXT_TOKENS` when either resolved. Only applies to proxy-mode launches
    (native mode has no selected model to look up). `ui.rs` uses the same discovered-then-static
    priority to render a `[1M]`/`[200K]`-style badge (`format_context_tokens`) next to any model
    with a known window.
- Recents: `AppState::recent_models` (`IndexMap<String, Vec<String>>`, provider -> up to 10
  model names newest-first, mirrors `config::Last.recent_models`) is pure usage history — it
  never touches `config::Provider.models`. `apply_selection` calls `record_recent_model` on
  every successful `Enter`. `replace_focused_provider_models` calls `reorder_by_recents` so
  recents-that-are-still-present always sort to the front of both
  `all_models_for_focused_provider` and (since search filters from that) `models_for_focused_provider`
  — no separate "section" tracking needed, search and cursor indexing work unmodified.
  `remove_focused_model_from_recents` (bound to `X`) mutates only `recent_models`, then re-runs
  the same reorder/filter to refresh the panel. `ui.rs` marks a recent with a `★ ` prefix by
  checking membership against `state.recent_models[focused_provider]` at render time.
