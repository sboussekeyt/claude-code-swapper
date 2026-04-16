# claude-code-swapper — Design Spec

Date: 2026-04-16

## Summary

`claude-code-swapper` is a minimal Python CLI that lets developers interactively select an LLM provider and model at startup, then launches Claude Code (`claude`) with the appropriate environment variables injected. It is a wrapper, not an orchestrator — its sole job is model selection + process handoff.

## Philosophy

- Unix-style: simple, fast, predictable
- No complex logic, no routing, no fallback
- Adding a provider = editing a YAML file, nothing else
- Process replaces itself with `claude` via `os.execvp` (no persistent wrapper)

## Project Structure

```
claude-code-swapper/
├── claude_code_swapper/
│   └── main.py          # single entry point: config loading + TUI + launcher
├── config.example.yaml  # template shipped with the project
├── pyproject.toml
└── README.md
```

User config lives at `~/.config/claude-code-swapper/config.yaml` (XDG standard).
On first launch, if absent, the tool copies `config.example.yaml` and informs the user.

## Dependencies

- `questionary` — interactive TUI (arrow-key menus)
- `PyYAML` — config parsing

No other dependencies.

## Config Format

`~/.config/claude-code-swapper/config.yaml`:

```yaml
providers:
  openrouter:
    base_url: https://openrouter.ai/api/v1
    api_key: sk-or-xxxx
    models:
      - anthropic/claude-sonnet-4-6
      - meta-llama/llama-3.1-8b-instruct
      - mistralai/mistral-7b-instruct

  anthropic:
    base_url: https://api.anthropic.com
    api_key: sk-ant-xxxx
    models:
      - claude-opus-4-6
      - claude-sonnet-4-6
      - claude-haiku-4-5

  minimax:
    base_url: https://api.minimax.chat/v1
    api_key: xxxx
    models:
      - abab6.5s-chat

  glm:
    base_url: https://open.bigmodel.cn/api/paas/v4
    api_key: xxxx
    models:
      - glm-4
      - glm-4-flash

  groq:
    base_url: https://api.groq.com/openai/v1
    api_key: gsk_xxxx
    models:
      - llama-3.1-8b-instant
      - mixtral-8x7b-32768
```

Each provider has exactly 3 fields: `base_url`, `api_key`, `models`. All providers are treated identically — no provider-specific logic.

## User Flow

```
$ claude-code-swapper

? Select a provider:
 ❯ openrouter
   anthropic
   minimax
   glm
   groq

? Select a model:
 ❯ anthropic/claude-sonnet-4-6
   meta-llama/llama-3.1-8b-instruct
   mistralai/mistral-7b-instruct

Launching claude with openrouter / anthropic/claude-sonnet-4-6...
```

## Environment Variables Injected

| Variable            | Source                    |
|---------------------|---------------------------|
| `ANTHROPIC_API_KEY` | `api_key` of chosen provider |
| `ANTHROPIC_BASE_URL`| `base_url` of chosen provider |

Model is passed as a CLI flag: `claude --model <model>`.

All existing environment variables are preserved. Only these two are overridden.

## Process Launch

```python
os.execvp("claude", ["claude", "--model", model])
```

`claude-code-swapper` replaces itself with `claude`. No subprocess, no persistent process. Behavior is identical to calling `claude --model <model>` directly with the env vars set.

## Last Selection Memory

Stored in `~/.config/claude-code-swapper/last.yaml`:

```yaml
provider: openrouter
model: anthropic/claude-sonnet-4-6
```

Written after each successful launch. On next run, the cursor is pre-positioned on the last used provider and model. No extra prompt — just cursor positioning.

## Error Handling

| Situation | Behavior |
|---|---|
| Config file missing | Clear message with expected path, copies example config |
| `claude` not in PATH | Error message: "claude not found — is Claude Code installed?" |
| Provider has no models | Silently excluded from menu |
| Invalid YAML | Error message with line/column from PyYAML |

## Installation

```toml
# pyproject.toml
[project]
name = "claude-code-swapper"
version = "0.1.0"
dependencies = ["questionary", "pyyaml"]

[project.scripts]
claude-code-swapper = "claude_code_swapper.main:main"
```

```bash
# From PyPI (future)
pipx install claude-code-swapper

# From GitHub
pipx install git+https://github.com/sboussekeyt/claude-code-swapper

# Dev
pip install -e .
```

## Out of Scope (v1)

- Automatic routing or fallback between providers
- Per-project configs
- Model presets (fast / cheap / smart)
- Any LLM abstraction layer (no LiteLLM, no LangChain)
