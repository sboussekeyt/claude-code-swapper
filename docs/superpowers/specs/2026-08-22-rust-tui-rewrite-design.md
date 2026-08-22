# claude-code-swapper: Rust + TUI rewrite

Date: 2026-08-22
Status: approved for planning

## Motivation

Ship claude-code-swapper as a single self-contained binary — no Python
interpreter, no `pip install -e .`, no venv. Secondary win: a real
full-screen TUI (dashboard-style, ratatui) instead of the current
sequential `questionary` prompts.

## Scope

Full feature parity with the current Python implementation
(`claude_code_swapper/main.py` as of commit `86768ee`). No new
features, no feature cuts. Once the Rust binary reaches parity and is
adopted, the Python package is deleted from the repo (history stays in
git). No dual-maintenance period is planned.

Feature checklist (all required for parity):

- Launch Claude in proxy mode (`ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` /
  `ANTHROPIC_API_KEY=""`, `--model`, optional `--dangerously-skip-permissions`)
- Launch Claude in native mode (no env override, no `--model`)
- Select provider + model, with model discovery (`GET {base_url}/v1/models`,
  `Authorization: Bearer {api_key}`, 1.5s timeout) falling back silently to
  the static `models:` list from config on any failure
- Add a model to a provider
- Remove a model from a provider
- Set/update a provider's API key
- Toggle RTK mode (persisted)
- Toggle auto-accept mode (persisted)
- First-run: if `rtk` is not on PATH, offer to install it
  (`curl -fsSL .../install.sh | sh`)
- If RTK mode is on, run `rtk init --global --auto-patch` before every
  proxy-mode launch
- Persist last provider/model/rtk/auto-accept choice across runs
  (`~/.config/claude-code-swapper/last.yaml`)
- First run with no config: copy the bundled example config to
  `~/.config/claude-code-swapper/config.yaml`, print instructions, exit

Out of scope for this rewrite: changing the config file format,
supporting Windows, packaging/distribution (homebrew, releases) —
`cargo install --path .` is enough for personal use.

## Stack

| Concern | Crate | Why |
|---|---|---|
| TUI | `ratatui` + `crossterm` | De facto standard for Rust TUIs, actively maintained |
| Text input widgets | `tui-input` | Avoids hand-rolling cursor/backspace/editing logic for the add-model / set-api-key modals |
| HTTP (model discovery) | `ureq` | Synchronous; no need to pull in `tokio` for one occasional `GET` |
| YAML | `serde_yaml_ng` | Maintained fork of `serde_yaml` (upstream archived 2024); same on-disk format, existing `config.yaml` needs no migration |
| Config paths | `dirs` | `~/.config/claude-code-swapper/` resolution, matches Python's `Path.home() / ".config" / ...` |
| Process exec | `std::os::unix::process::CommandExt::exec` | stdlib, direct equivalent of Python's `os.execvpe`; Unix-only, matches the user's macOS-only usage |
| Testing | `tempfile`, a minimal local `TcpListener`/`tiny_http` server for discovery tests | Mirrors the Python test suite's approach (real round-trips over mocks where practical) |

## Architecture

Single binary crate. Modules, by responsibility:

```
src/
  main.rs       # wiring: load config, init terminal, run event loop, teardown before exec
  config.rs     # load/save config.yaml + last.yaml
  discovery.rs  # fetch_remote_models via ureq
  launcher.rs   # build_env, build_args, launch_claude, launch_claude_native,
                # check_claude, rtk install/hook
  app.rs        # AppState + all state transitions — pure, no I/O, no terminal
  ui.rs         # rendu ratatui: pure functions of AppState -> widgets
  event.rs      # translates crossterm events into AppState actions
```

`app.rs` holds all business logic as pure methods on `AppState` (no
I/O, no terminal handle). `ui.rs` only reads that state to draw. This
mirrors the Python tests' approach — they never touch the actual
`questionary` rendering, only the answers it returns — and keeps the
Rust logic testable without a terminal.

### AppState (sketch)

```rust
struct AppState {
    providers: Vec<String>,          // from config, order preserved
    focused_panel: Panel,            // Providers | Models
    provider_cursor: usize,
    model_cursor: usize,
    models_for_focused_provider: Vec<String>,   // discovered or static, refreshed on provider change
    current_provider: Option<String>,           // "applied" selection, mirrors last.yaml
    current_model: Option<String>,
    rtk_enabled: bool,
    auto_accept: bool,
    modal: Option<Modal>,            // None | AddModel{ provider, input } | SetApiKey{ provider, input } | ConfirmRtkInstall
    status_message: Option<String>,  // e.g. "claude not found in PATH"
}
```

## Data flow

1. **Before the TUI**: load `config.yaml` (parse error → print to
   stderr, exit 1, exactly like today) and `last.yaml`. Check `claude`
   is on PATH — if not, exit 1 with a message (same as today; no point
   entering a TUI that can never launch anything).
2. **Enter TUI**: enable raw mode + alternate screen (crossterm).
   Install a **custom panic hook** that restores the terminal (raw
   mode off, leave alternate screen) before the default panic handler
   runs — without this, any panic leaves the user's terminal in a
   broken raw-mode state. This is the single most important
   correctness requirement of the rewrite.
   If `rtk` isn't on PATH, show the install-confirm modal first.
3. **Main loop**: render dashboard → block on next crossterm event →
   dispatch to `AppState` → re-render. Moving the Providers-panel
   cursor re-runs `fetch_remote_models` for the newly focused provider
   (blocking call, 1.5s timeout, silent fallback to the static list —
   same contract as the Python `fetch_remote_models`).
4. **On Launch**: leave the TUI cleanly (disable raw mode, leave
   alternate screen) *before* calling `exec` — `claude` must inherit a
   normal terminal, not one left in raw/alternate-screen mode. Then
   `exec` replaces the process, same as `os.execvpe`.
5. **Persistence**: `last.yaml` is rewritten on every applied selection
   (Enter) and on every successful launch, matching today's behavior.

## Keybindings

| Key | Action |
|---|---|
| `Tab` | Switch focus between Providers panel and Models panel |
| `↑` / `↓` | Move cursor within the focused panel |
| `Enter` | Apply the focused provider+model as the current selection (writes `last.yaml`) |
| `l` | Launch Claude (proxy mode) with the current selection |
| `n` | Launch Claude (native mode) |
| `a` | Add a model to the focused provider (opens text input modal) |
| `x` | Remove the focused model |
| `s` | Set API key for the focused provider (opens masked text input modal) |
| `r` | Toggle RTK mode |
| `p` | Toggle auto-accept mode (`--dangerously-skip-permissions`) |
| `q` / `Esc` | Quit (closes an open modal first if one is open) |

The footer always renders this list (abbreviated), same spirit as the
current `display_status` panel — status is always visible, not printed
once per loop iteration.

## Error handling

- Config/YAML parse errors, missing `claude` binary: handled before
  the TUI starts (stderr + exit), unchanged from Python.
- Model discovery failures (unreachable host, timeout, malformed
  JSON): silent, falls back to the configured static list — identical
  contract to the current `fetch_remote_models`.
- No providers configured / no providers with models: rendered as an
  empty-state message inside the Providers/Models panels, not a fatal
  error — matches today's non-fatal `print` + return `None`.
- Panics: caught by the custom hook (see Data flow §2) so the
  terminal is never left broken.

## Testing strategy

- `app.rs`: pure unit tests on `AppState` transitions (`#[test]`, no
  mocking) — direct equivalent of `toggle_option`,
  `select_provider_and_model` tests in Python, simpler here since
  there's no input library to intercept.
- `config.rs`: load/save round-trips using `tempfile`, mirroring the
  `tmp_path` fixture usage in the Python suite.
- `discovery.rs`: a real local HTTP server (`TcpListener`/`tiny_http`)
  serving a canned `/v1/models` response in tests — same approach used
  to manually verify the Python `fetch_remote_models` end-to-end
  during this session, promoted to an actual automated test this time.
- `launcher.rs`: `build_env`/`build_args` are pure and fully tested;
  the final `exec()` call itself is inherently untestable in-process
  (it replaces the process) and is kept as the last, minimal,
  uncovered line — same limitation the Python suite has for
  `os.execvpe` (there it's mocked; in Rust there's no equivalent
  seam, so it's just excluded from coverage).
- `ui.rs`: not a priority for automated testing at this stage: manual
  verification during development is sufficient for a personal tool.
  Ratatui's `TestBackend` is available later if it becomes worth it.

## Migration / cutover

1. Build the Rust binary to full parity per the checklist above.
2. Manually verify against the user's real config (`~/.config/claude-code-swapper/config.yaml`,
   already containing `anthropic`, `glm`, `groq`, `lmstudio`, `minimax`, `openrouter`)
   — in particular the `lmstudio` provider end-to-end (discovery +
   proxy launch), since that flow was just debugged and fixed on the
   Python side this session.
3. Once satisfied, delete `claude_code_swapper/` (Python package) and
   `tests/test_main.py`, update `pyproject.toml`/README accordingly,
   replace with `Cargo.toml` + Rust `README` instructions
   (`cargo install --path .`).
