# claude-code-swapper

A full-screen TUI for launching Claude Code with different LLM providers (OpenRouter, Groq, LM Studio, Ollama, etc.) via `ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN`. Single self-contained Rust binary, no runtime dependencies.

## Install

```bash
cargo install --path .
```

## Usage

```bash
claude-code-swapper
```

- `Tab` — switch focus between the Providers and Models panels
- `↑`/`↓` — move the cursor (also triggers live model discovery when moving in the Providers panel)
- `Enter` — select the highlighted provider/model
- `l` — launch Claude with the selected provider (proxy mode)
- `n` — launch Claude natively (no provider override — uses your normal login/config)
- `a` — add a model to the focused provider
- `x` — remove the highlighted model
- `s` — set the API key for the focused provider
- `/` — search/filter the Models panel (useful for providers with a large catalog, like OpenRouter)
- `r` — toggle RTK mode
- `p` — toggle auto-accept mode (`--dangerously-skip-permissions`)
- `q` / `Esc` / `Ctrl+C` — quit
- Inside a modal (add model / set API key): `Enter` confirm, `Esc` cancel, `Backspace` delete last character
- While searching: type to filter (case-insensitive substring match), `↑`/`↓` to move within the filtered list, `Enter` to select the highlighted model and close the search, `Esc` to cancel the search and restore the full list

## Config

On first run, a config template is created at `~/.config/claude-code-swapper/config.yaml`. Edit it to add your API keys:

```yaml
providers:
  openrouter:
    base_url: https://openrouter.ai/api
    api_key: sk-or-YOUR_KEY
    models:
      - anthropic/claude-sonnet-4-6
      - meta-llama/llama-3.1-8b-instruct

  lmstudio:
    base_url: http://localhost:1234
    api_key: lm-studio
    models:
      - local-model

  ollama:
    base_url: http://localhost:11434
    api_key: ollama
    models:
      - llama3.1
```

Adding a new provider = adding a new block. No code changes needed. This also covers local servers like [LM Studio](https://lmstudio.ai) and [Ollama](https://ollama.com) — start the local server, then set `models` to whatever model identifier is loaded (`api_key` can be any non-empty string for these). Moving the cursor onto a provider also auto-discovers what's actually loaded via `{base_url}/v1/models` when the provider exposes it, falling back to the static `models:` list otherwise.

**`base_url` must NOT include a trailing `/v1`.** Claude Code appends `/v1/messages` itself; if `base_url` already ends in `/v1` you get a `/v1/v1/messages` request, which most providers won't recognize (LM Studio silently returns a malformed 200 for it, which shows up as `API Error: ... not a Message`).

## RTK

[RTK](https://www.rtk-ai.app) compresses command output before it reaches Claude's context, cutting token usage. If it's not installed, claude-code-swapper offers to install it on startup. When RTK mode is on, `rtk init --global --auto-patch` is run automatically before every proxy-mode Claude launch, to keep the hook active.

## Dev install

```bash
cargo build --release
cargo test
```
