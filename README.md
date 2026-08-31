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
- `X` (Shift+X) — unpin the highlighted model from Recents (see below); does nothing if it isn't one
- `s` — set the API key for the focused provider
- `/` — search/filter the Models panel (useful for providers with a large catalog, like OpenRouter)
- `r` — toggle RTK mode
- `p` — toggle auto-accept mode (`--dangerously-skip-permissions`)
- `q` / `Esc` / `Ctrl+C` — quit
- Inside a modal (add model / set API key): `Enter` confirm, `Esc` cancel, `Backspace` delete last character
- While searching: type to filter (case-insensitive substring match), `↑`/`↓` to move within the filtered list, `Enter` to select the highlighted model and close the search, `Esc` to cancel the search and restore the full list

## Config

On first run, a config template is created at `~/.config/claude-code-swapper/config.yaml`:

```yaml
providers:
  openrouter:
    api_key: sk-or-REPLACE_ME

  groq:
    api_key: gsk_REPLACE_ME

  lmstudio: {}
  ollama: {}
```

That's the whole file — short enough to hand to a teammate as-is. `openrouter`, `groq`, `lmstudio`, and `ollama` are **known providers** (see `known_providers.rs`): their `base_url` and discovery strategy ship with the binary, so you only supply what's actually yours (an `api_key`) rather than typing out connection details that don't change from one machine to the next. LM Studio and Ollama don't check the key at all, so they need nothing beyond the provider name — `{}` is an empty YAML mapping.

Any field you *do* set explicitly — `base_url`, `api_key`, `kind` — overrides the built-in default, so pointing at a self-hosted or non-default port still works:

```yaml
providers:
  lmstudio:
    base_url: http://192.168.1.50:1234 # a different machine on the LAN
```

A provider that isn't known needs its own full block, same as before:

```yaml
providers:
  glm:
    base_url: https://open.bigmodel.cn/api/paas/v4
    api_key: REPLACE_ME
    models:
      - glm-4.6
```

`models:` is only a fallback — moving the cursor onto a provider auto-discovers what's actually available and replaces it live. The optional `kind` field picks *how* discovery works for that provider:

- Unset (default) — a plain OpenAI-compatible `{base_url}/v1/models`. Covers OpenRouter, Groq, and most hosted APIs.
- `kind: lmstudio` — LM Studio's own richer API (`/api/v0/models`), which lists every **downloaded** model (not just the one currently loaded) along with its context length. Falls back to the plain `/v1/models` path automatically if that endpoint isn't available (older LM Studio versions).
- `kind: ollama` — Ollama's own API (`/api/tags`), which lists every locally **pulled** model, not just whichever one Ollama currently has loaded into memory.

Adding a new known provider is one entry in `known_providers.rs` (name, base_url, kind). Adding support for a new provider's *native discovery API* is one small trait impl (`discovery::ModelSource`) plus one new `ProviderKind` variant — see `CLAUDE.md` if you want to add one.

**`base_url` must NOT include a trailing `/v1`.** Claude Code appends `/v1/messages` itself; if `base_url` already ends in `/v1` you get a `/v1/v1/messages` request, which most providers won't recognize (LM Studio silently returns a malformed 200 for it, which shows up as `API Error: ... not a Message`).

### Context window for unrecognized models

Claude Code assumes a 200k-token context window for any model it doesn't recognize by name, which makes auto-compact trigger too early for models with a larger real window (e.g. a 1M-token model).

**Auto-discovered (no config needed):** OpenRouter's `/v1/models` and LM Studio's native API (`kind: lmstudio`) both report `context_length` — claude-code-swapper picks it up automatically every time you browse to that provider, no manual step required. The Models panel shows a `[1M]`/`[200K]`-style badge next to any model with a known window, and launching it (`l`) sets `CLAUDE_CODE_MAX_CONTEXT_TOKENS` for you.

**Manual fallback:** for providers that don't report a context length (Ollama's `/api/tags` doesn't, Groq's `/v1/models` doesn't, some direct provider APIs don't), declare it yourself under an optional `context_windows` map — it's only used when nothing was auto-discovered for that model:

```yaml
providers:
  openrouter:
    base_url: https://openrouter.ai/api
    api_key: sk-or-YOUR_KEY
    models:
      - deepseek/deepseek-v4-flash-0731
    context_windows:
      deepseek/deepseek-v4-flash-0731: 1000000
```

Models without either an auto-discovered or a configured entry are unaffected — Claude Code falls back to its own detection (or the 200k default). This only applies to proxy-mode launches (`l`); native mode (`n`) uses your own Claude Code config as-is.

### Recents

Every time you select a model (`Enter`), it's remembered per-provider — up to the 10 most recent, newest first — and shown at the top of the Models panel with a `★` marker, ahead of the rest of the list. Handy for a large catalog like OpenRouter's, where you'd otherwise have to scroll or search for a model you already use regularly.

This is pure usage history: it's stored in `~/.config/claude-code-swapper/last.yaml`, never written to `config.yaml`, and doesn't require adding anything to a provider's static `models:` list — it works the same whether a model came from live discovery or the config file. Press `X` (Shift+X) on a highlighted recent to unpin it; this only forgets it, it doesn't remove the model from anywhere else.

## RTK

[RTK](https://www.rtk-ai.app) compresses command output before it reaches Claude's context, cutting token usage. If it's not installed, claude-code-swapper offers to install it on startup. When RTK mode is on, `rtk init --global --auto-patch` is run automatically before every proxy-mode Claude launch, to keep the hook active.

## Dev install

```bash
cargo build --release
cargo test
```
