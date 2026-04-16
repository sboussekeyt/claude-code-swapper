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

## Config

On first run, a config template is created at `~/.config/claude-code-swapper/config.yaml`. Edit it to add your API keys:

```yaml
providers:
  openrouter:
    base_url: https://openrouter.ai/api/v1
    api_key: sk-or-YOUR_KEY
    models:
      - anthropic/claude-sonnet-4-6
      - meta-llama/llama-3.1-8b-instruct
```

Adding a new provider = adding a new block. No code changes needed.

## Dev install

```bash
pip install -e .
pytest
```
