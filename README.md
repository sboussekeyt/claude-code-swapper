# claude-code-swapper

Minimal CLI wrapper for Claude Code that lets you switch LLM provider and model interactively at startup.

## Install

```bash
pipx install git+https://github.com/sboussekeyt/claude-code-swapper
```

## Usage

```bash
claude-code-swapper
```

Select a provider and model with arrow keys — Claude Code launches with the right environment variables set.

Pick "Launch Claude (native)" instead to skip the provider swap entirely — Claude Code runs with your normal environment, using its own default login/config.

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
```

Adding a new provider = adding a new block. No code changes needed. This also covers local servers like [LM Studio](https://lmstudio.ai) — start its local server, then set `models` to whatever model identifier is loaded (`api_key` can be any non-empty string, LM Studio doesn't check it). Model selection also auto-discovers what's actually loaded via `{base_url}/v1/models` when the provider exposes it, falling back to the static `models:` list otherwise.

**`base_url` must NOT include a trailing `/v1`.** Claude Code appends `/v1/messages` itself; if `base_url` already ends in `/v1` you get a `/v1/v1/messages` request, which most providers won't recognize (LM Studio silently returns a malformed 200 for it, which shows up as `API Error: ... not a Message`).

## RTK

[RTK](https://www.rtk-ai.app) compresses command output before it reaches Claude's context, cutting token usage. If it's not installed, claude-code-swapper offers to install it on startup. Toggle "Toggle RTK mode" in the menu to have it re-activate the RTK hook (`rtk init --global`) automatically before every Claude launch.

## Dev install

```bash
pip install -e .
pytest
```
