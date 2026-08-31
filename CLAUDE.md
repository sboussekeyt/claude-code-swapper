# claude-code-swapper (Rust + TUI)

Full-screen `ratatui` TUI, single self-contained Rust binary, for launching Claude Code
against different LLM providers (OpenRouter, Groq, LM Studio, Ollama, etc.) via
`ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN`. This is the Rust rewrite of the original
Python `claude_code_swapper` package — the Python code has been (or will be) removed once
this reaches parity.

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
| `config.rs` | Load/save `config.yaml` + `last.yaml`; bootstraps `config.yaml` from `assets/config.example.yaml` on first run |
| `discovery.rs` | `fetch_remote_models` — `GET {base_url}/v1/models` via `ureq`, 1.5s timeout, silent fallback on any failure |
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
| `s` | Set API key for the focused provider (opens masked modal) |
| `r` | Toggle RTK mode |
| `p` | Toggle auto-accept mode (`--dangerously-skip-permissions`) |
| `q` / `Esc` / `Ctrl+C` | Quit |
| (modal open) `Enter` / `Esc` / `Backspace` | Confirm / cancel / delete-last-char |

Ctrl-modified letter keys (`Ctrl+X`, `Ctrl+A`, `Ctrl+L`, etc.) are intentionally inert —
only the plain, unmodified key fires the action, so accidental chords can't trigger
destructive operations (e.g. `Ctrl+X` does not delete a model).

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

## Merge note

This file does not exist on `main` yet — `main`'s `CLAUDE.md` documents the now-superseded
Python implementation. Expect a real conflict when this branch merges; reconcile by keeping
this Rust-focused version (the Python package is being removed as part of the rewrite).
