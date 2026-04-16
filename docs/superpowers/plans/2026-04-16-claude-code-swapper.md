# claude-code-swapper Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a minimal Python CLI that interactively selects an LLM provider + model and launches Claude Code with the appropriate environment variables.

**Architecture:** Single `main.py` with four pure functions (`load_config`, `load_last`, `save_last`, `select_provider_and_model`, `launch_claude`) wired together by `main()`. Config and last-selection stored in `~/.config/claude-code-swapper/`. Process replaced by `claude` via `os.execvpe`.

**Tech Stack:** Python 3.10+, `questionary` (TUI), `PyYAML`, `pytest` (tests), `hatchling` (build), `pipx` (distribution).

---

## File Map

| File | Responsibility |
|---|---|
| `pyproject.toml` | Package metadata, deps, entry point, build config |
| `claude_code_swapper/__init__.py` | Empty package marker |
| `claude_code_swapper/main.py` | All logic: config loading, last-selection memory, TUI, launcher, `main()` |
| `claude_code_swapper/config.example.yaml` | Bundled template copied on first launch |
| `tests/test_main.py` | All unit tests |
| `README.md` | Install + usage instructions |

---

## Task 1: Project Scaffold

**Files:**
- Create: `pyproject.toml`
- Create: `claude_code_swapper/__init__.py`
- Create: `claude_code_swapper/main.py`

- [ ] **Step 1: Create `pyproject.toml`**

```toml
[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[project]
name = "claude-code-swapper"
version = "0.1.0"
requires-python = ">=3.10"
dependencies = [
    "questionary>=2.0",
    "pyyaml>=6.0",
]

[project.scripts]
claude-code-swapper = "claude_code_swapper.main:main"

[tool.hatch.build.targets.wheel]
packages = ["claude_code_swapper"]

[tool.pytest.ini_options]
testpaths = ["tests"]
```

- [ ] **Step 2: Create `claude_code_swapper/__init__.py`**

```python
```

(empty file)

- [ ] **Step 3: Create stub `claude_code_swapper/main.py`**

```python
def main() -> None:
    pass
```

- [ ] **Step 4: Install in dev mode with test deps**

```bash
pip install -e . && pip install pytest
```

Expected: no errors, `claude-code-swapper` command available.

- [ ] **Step 5: Commit**

```bash
git add pyproject.toml claude_code_swapper/__init__.py claude_code_swapper/main.py
git commit -m "feat: scaffold project structure"
```

---

## Task 2: Bundled Example Config

**Files:**
- Create: `claude_code_swapper/config.example.yaml`

- [ ] **Step 1: Create `claude_code_swapper/config.example.yaml`**

```yaml
providers:
  openrouter:
    base_url: https://openrouter.ai/api/v1
    api_key: sk-or-REPLACE_ME
    models:
      - anthropic/claude-sonnet-4-6
      - meta-llama/llama-3.1-8b-instruct
      - mistralai/mistral-7b-instruct

  anthropic:
    base_url: https://api.anthropic.com
    api_key: sk-ant-REPLACE_ME
    models:
      - claude-opus-4-6
      - claude-sonnet-4-6
      - claude-haiku-4-5

  minimax:
    base_url: https://api.minimax.chat/v1
    api_key: REPLACE_ME
    models:
      - abab6.5s-chat

  glm:
    base_url: https://open.bigmodel.cn/api/paas/v4
    api_key: REPLACE_ME
    models:
      - glm-4
      - glm-4-flash

  groq:
    base_url: https://api.groq.com/openai/v1
    api_key: gsk_REPLACE_ME
    models:
      - llama-3.1-8b-instant
      - mixtral-8x7b-32768
```

- [ ] **Step 2: Verify the file is included in the package**

```bash
python -c "from importlib.resources import files; print(files('claude_code_swapper').joinpath('config.example.yaml').read_text()[:50])"
```

Expected: first 50 chars of the YAML printed without error.

- [ ] **Step 3: Commit**

```bash
git add claude_code_swapper/config.example.yaml
git commit -m "feat: add bundled example config"
```

---

## Task 3: Config Loading

**Files:**
- Create: `tests/test_main.py`
- Modify: `claude_code_swapper/main.py`

- [ ] **Step 1: Create `tests/__init__.py`**

```python
```

(empty file)

- [ ] **Step 2: Write failing tests for `load_config`**

Create `tests/test_main.py`:

```python
import os
import pytest
import yaml
from pathlib import Path
from unittest.mock import patch

from claude_code_swapper.main import load_config

SAMPLE_CONFIG = {
    "providers": {
        "openrouter": {
            "base_url": "https://openrouter.ai/api/v1",
            "api_key": "sk-or-test",
            "models": ["anthropic/claude-sonnet-4-6", "meta-llama/llama-3.1-8b"],
        },
        "anthropic": {
            "base_url": "https://api.anthropic.com",
            "api_key": "sk-ant-test",
            "models": ["claude-opus-4-6"],
        },
        "empty_provider": {
            "base_url": "https://example.com",
            "api_key": "key",
            "models": [],
        },
    }
}


class TestLoadConfig:
    def test_loads_existing_config(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))
        result = load_config(config_path=config_file)
        assert result["providers"]["openrouter"]["api_key"] == "sk-or-test"

    def test_missing_config_creates_file_and_exits(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        with patch(
            "claude_code_swapper.main.files"
        ) as mock_files, pytest.raises(SystemExit) as exc:
            mock_files.return_value.joinpath.return_value.read_text.return_value = (
                "providers: {}"
            )
            load_config(config_path=config_file)
        assert exc.value.code == 0
        assert config_file.exists()

    def test_missing_config_prints_path(self, tmp_path, capsys):
        config_file = tmp_path / "config.yaml"
        with patch(
            "claude_code_swapper.main.files"
        ) as mock_files, pytest.raises(SystemExit):
            mock_files.return_value.joinpath.return_value.read_text.return_value = (
                "providers: {}"
            )
            load_config(config_path=config_file)
        out = capsys.readouterr().out
        assert str(config_file) in out

    def test_invalid_yaml_exits_with_error(self, tmp_path, capsys):
        config_file = tmp_path / "config.yaml"
        config_file.write_text("providers: [invalid: yaml: :")
        with pytest.raises(SystemExit) as exc:
            load_config(config_path=config_file)
        assert exc.value.code == 1
        assert "Invalid YAML" in capsys.readouterr().out
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
pytest tests/test_main.py::TestLoadConfig -v
```

Expected: `ImportError` or `AttributeError` — `load_config` not yet implemented.

- [ ] **Step 4: Implement `load_config` in `main.py`**

Replace stub `main.py` with:

```python
import os
import sys
import shutil
from pathlib import Path
from importlib.resources import files

import questionary
import yaml

CONFIG_DIR = Path.home() / ".config" / "claude-code-swapper"
CONFIG_PATH = CONFIG_DIR / "config.yaml"
LAST_PATH = CONFIG_DIR / "last.yaml"


def load_config(config_path: Path = CONFIG_PATH) -> dict:
    if not config_path.exists():
        config_path.parent.mkdir(parents=True, exist_ok=True)
        example = files("claude_code_swapper").joinpath("config.example.yaml").read_text()
        config_path.write_text(example)
        print(f"Config created at {config_path}")
        print("Edit it to add your API keys, then run claude-code-swapper again.")
        sys.exit(0)
    try:
        with open(config_path) as f:
            return yaml.safe_load(f) or {}
    except yaml.YAMLError as e:
        print(f"Invalid YAML in {config_path}:\n{e}")
        sys.exit(1)


def main() -> None:
    pass
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
pytest tests/test_main.py::TestLoadConfig -v
```

Expected: 4 PASSED.

- [ ] **Step 6: Commit**

```bash
git add claude_code_swapper/main.py tests/__init__.py tests/test_main.py
git commit -m "feat: implement config loading"
```

---

## Task 4: Last Selection Memory

**Files:**
- Modify: `tests/test_main.py` (add `TestLoadLast`, `TestSaveLast`)
- Modify: `claude_code_swapper/main.py` (add `load_last`, `save_last`)

- [ ] **Step 1: Write failing tests for `load_last` and `save_last`**

Append to `tests/test_main.py` (after `TestLoadConfig`):

```python
from claude_code_swapper.main import load_last, save_last


class TestLoadLast:
    def test_returns_none_when_file_missing(self, tmp_path):
        result = load_last(last_path=tmp_path / "last.yaml")
        assert result == (None, None)

    def test_returns_saved_values(self, tmp_path):
        last_file = tmp_path / "last.yaml"
        last_file.write_text(yaml.dump({"provider": "openrouter", "model": "claude-3"}))
        assert load_last(last_path=last_file) == ("openrouter", "claude-3")

    def test_returns_none_on_corrupt_yaml(self, tmp_path):
        last_file = tmp_path / "last.yaml"
        last_file.write_text(": invalid :")
        assert load_last(last_path=last_file) == (None, None)


class TestSaveLast:
    def test_writes_provider_and_model(self, tmp_path):
        last_file = tmp_path / "last.yaml"
        save_last("openrouter", "claude-3", last_path=last_file)
        data = yaml.safe_load(last_file.read_text())
        assert data == {"model": "claude-3", "provider": "openrouter"}

    def test_creates_parent_directories(self, tmp_path):
        last_file = tmp_path / "nested" / "dir" / "last.yaml"
        save_last("openrouter", "model", last_path=last_file)
        assert last_file.exists()
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
pytest tests/test_main.py::TestLoadLast tests/test_main.py::TestSaveLast -v
```

Expected: `ImportError` — `load_last` and `save_last` not yet defined.

- [ ] **Step 3: Add `load_last` and `save_last` to `main.py`**

Add after `load_config`:

```python
def load_last(last_path: Path = LAST_PATH) -> tuple[str | None, str | None]:
    if not last_path.exists():
        return None, None
    try:
        with open(last_path) as f:
            data = yaml.safe_load(f) or {}
        return data.get("provider"), data.get("model")
    except yaml.YAMLError:
        return None, None


def save_last(provider: str, model: str, last_path: Path = LAST_PATH) -> None:
    last_path.parent.mkdir(parents=True, exist_ok=True)
    with open(last_path, "w") as f:
        yaml.dump({"provider": provider, "model": model}, f)
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
pytest tests/test_main.py::TestLoadLast tests/test_main.py::TestSaveLast -v
```

Expected: 5 PASSED.

- [ ] **Step 5: Commit**

```bash
git add claude_code_swapper/main.py tests/test_main.py
git commit -m "feat: implement last selection memory"
```

---

## Task 5: TUI Selection

**Files:**
- Modify: `tests/test_main.py` (add `TestSelectProviderAndModel`)
- Modify: `claude_code_swapper/main.py` (add `select_provider_and_model`)

- [ ] **Step 1: Write failing tests for `select_provider_and_model`**

Append to `tests/test_main.py`:

```python
from claude_code_swapper.main import select_provider_and_model


class TestSelectProviderAndModel:
    def test_selects_provider_and_model(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.side_effect = [
                "openrouter",
                "meta-llama/llama-3.1-8b",
            ]
            provider, model = select_provider_and_model(SAMPLE_CONFIG)
        assert provider == "openrouter"
        assert model == "meta-llama/llama-3.1-8b"

    def test_excludes_providers_with_no_models(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.side_effect = [
                "openrouter",
                "anthropic/claude-sonnet-4-6",
            ]
            select_provider_and_model(SAMPLE_CONFIG)
            choices = mock_select.call_args_list[0][1]["choices"]
        assert "empty_provider" not in choices

    def test_preselects_last_provider(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.side_effect = ["anthropic", "claude-opus-4-6"]
            select_provider_and_model(
                SAMPLE_CONFIG, last_provider="anthropic", last_model="claude-opus-4-6"
            )
            provider_call_kwargs = mock_select.call_args_list[0][1]
        assert provider_call_kwargs["default"] == "anthropic"

    def test_preselects_last_model(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.side_effect = [
                "openrouter",
                "meta-llama/llama-3.1-8b",
            ]
            select_provider_and_model(
                SAMPLE_CONFIG,
                last_provider="openrouter",
                last_model="meta-llama/llama-3.1-8b",
            )
            model_call_kwargs = mock_select.call_args_list[1][1]
        assert model_call_kwargs["default"] == "meta-llama/llama-3.1-8b"

    def test_exits_zero_when_user_cancels_provider(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.return_value = None
            with pytest.raises(SystemExit) as exc:
                select_provider_and_model(SAMPLE_CONFIG)
        assert exc.value.code == 0

    def test_exits_zero_when_user_cancels_model(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.side_effect = ["openrouter", None]
            with pytest.raises(SystemExit) as exc:
                select_provider_and_model(SAMPLE_CONFIG)
        assert exc.value.code == 0

    def test_exits_when_no_providers_configured(self):
        with pytest.raises(SystemExit) as exc:
            select_provider_and_model({"providers": {}})
        assert exc.value.code == 1

    def test_ignores_unknown_last_provider(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.side_effect = [
                "openrouter",
                "anthropic/claude-sonnet-4-6",
            ]
            select_provider_and_model(
                SAMPLE_CONFIG, last_provider="nonexistent", last_model=None
            )
            provider_call_kwargs = mock_select.call_args_list[0][1]
        assert provider_call_kwargs["default"] is None
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
pytest tests/test_main.py::TestSelectProviderAndModel -v
```

Expected: `ImportError` — `select_provider_and_model` not yet defined.

- [ ] **Step 3: Add `select_provider_and_model` to `main.py`**

Add after `save_last`:

```python
def select_provider_and_model(
    config: dict,
    last_provider: str | None = None,
    last_model: str | None = None,
) -> tuple[str, str]:
    providers = [
        p
        for p, v in config.get("providers", {}).items()
        if v.get("models")
    ]
    if not providers:
        print("No providers with models configured.")
        print("Edit ~/.config/claude-code-swapper/config.yaml to add providers.")
        sys.exit(1)

    default_provider = last_provider if last_provider in providers else None
    provider = questionary.select(
        "Select a provider:",
        choices=providers,
        default=default_provider,
    ).ask()

    if provider is None:
        sys.exit(0)

    models = config["providers"][provider]["models"]
    default_model = last_model if last_model in models else None
    model = questionary.select(
        "Select a model:",
        choices=models,
        default=default_model,
    ).ask()

    if model is None:
        sys.exit(0)

    return provider, model
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
pytest tests/test_main.py::TestSelectProviderAndModel -v
```

Expected: 8 PASSED.

- [ ] **Step 5: Commit**

```bash
git add claude_code_swapper/main.py tests/test_main.py
git commit -m "feat: implement TUI provider and model selection"
```

---

## Task 6: Claude Launcher

**Files:**
- Modify: `tests/test_main.py` (add `TestLaunchClaude`)
- Modify: `claude_code_swapper/main.py` (add `launch_claude`)

- [ ] **Step 1: Write failing tests for `launch_claude`**

Append to `tests/test_main.py`:

```python
from claude_code_swapper.main import launch_claude


class TestLaunchClaude:
    def test_execs_claude_with_model_flag(self):
        provider_config = {
            "api_key": "sk-test",
            "base_url": "https://openrouter.ai/api/v1",
        }
        with patch("shutil.which", return_value="/usr/bin/claude"), patch(
            "os.execvpe"
        ) as mock_exec:
            launch_claude(provider_config, "anthropic/claude-sonnet-4-6")
        args = mock_exec.call_args[0]
        assert args[0] == "claude"
        assert args[1] == ["claude", "--model", "anthropic/claude-sonnet-4-6"]

    def test_injects_api_key_env_var(self):
        provider_config = {
            "api_key": "sk-test",
            "base_url": "https://openrouter.ai/api/v1",
        }
        with patch("shutil.which", return_value="/usr/bin/claude"), patch(
            "os.execvpe"
        ) as mock_exec:
            launch_claude(provider_config, "some-model")
        env = mock_exec.call_args[0][2]
        assert env["ANTHROPIC_API_KEY"] == "sk-test"

    def test_injects_base_url_env_var(self):
        provider_config = {
            "api_key": "sk-test",
            "base_url": "https://openrouter.ai/api/v1",
        }
        with patch("shutil.which", return_value="/usr/bin/claude"), patch(
            "os.execvpe"
        ) as mock_exec:
            launch_claude(provider_config, "some-model")
        env = mock_exec.call_args[0][2]
        assert env["ANTHROPIC_BASE_URL"] == "https://openrouter.ai/api/v1"

    def test_preserves_existing_env_vars(self):
        provider_config = {
            "api_key": "sk-test",
            "base_url": "https://example.com",
        }
        with patch("shutil.which", return_value="/usr/bin/claude"), patch(
            "os.execvpe"
        ) as mock_exec, patch.dict(os.environ, {"MY_VAR": "my-value"}):
            launch_claude(provider_config, "model")
        env = mock_exec.call_args[0][2]
        assert env["MY_VAR"] == "my-value"

    def test_exits_when_claude_not_found(self, capsys):
        with patch("shutil.which", return_value=None), pytest.raises(
            SystemExit
        ) as exc:
            launch_claude({"api_key": "k", "base_url": "u"}, "model")
        assert exc.value.code == 1
        assert "claude" in capsys.readouterr().out.lower()
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
pytest tests/test_main.py::TestLaunchClaude -v
```

Expected: `ImportError` — `launch_claude` not yet defined.

- [ ] **Step 3: Add `launch_claude` to `main.py`**

Add after `select_provider_and_model`:

```python
def launch_claude(provider_config: dict, model: str) -> None:
    if shutil.which("claude") is None:
        print("Error: 'claude' not found in PATH — is Claude Code installed?")
        sys.exit(1)

    env = os.environ.copy()
    env["ANTHROPIC_API_KEY"] = provider_config["api_key"]
    env["ANTHROPIC_BASE_URL"] = provider_config["base_url"]

    print(f"Launching claude with model {model}...")
    os.execvpe("claude", ["claude", "--model", model], env)
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
pytest tests/test_main.py::TestLaunchClaude -v
```

Expected: 5 PASSED.

- [ ] **Step 5: Commit**

```bash
git add claude_code_swapper/main.py tests/test_main.py
git commit -m "feat: implement claude launcher"
```

---

## Task 7: Wire `main()` and Full Test Suite

**Files:**
- Modify: `claude_code_swapper/main.py` (implement `main()`)
- Modify: `tests/test_main.py` (add `TestMain`)

- [ ] **Step 1: Write failing test for `main()`**

Append to `tests/test_main.py`:

```python
from claude_code_swapper.main import main


class TestMain:
    def test_full_flow(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("questionary.select") as mock_select, \
             patch("shutil.which", return_value="/usr/bin/claude"), \
             patch("os.execvpe") as mock_exec:
            mock_select.return_value.ask.side_effect = [
                "openrouter",
                "anthropic/claude-sonnet-4-6",
            ]
            main()

        env = mock_exec.call_args[0][2]
        assert env["ANTHROPIC_API_KEY"] == "sk-or-test"
        assert env["ANTHROPIC_BASE_URL"] == "https://openrouter.ai/api/v1"
        assert mock_exec.call_args[0][1] == ["claude", "--model", "anthropic/claude-sonnet-4-6"]

    def test_saves_last_selection(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("questionary.select") as mock_select, \
             patch("shutil.which", return_value="/usr/bin/claude"), \
             patch("os.execvpe"):
            mock_select.return_value.ask.side_effect = ["anthropic", "claude-opus-4-6"]
            main()

        data = yaml.safe_load(last_file.read_text())
        assert data == {"provider": "anthropic", "model": "claude-opus-4-6"}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
pytest tests/test_main.py::TestMain -v
```

Expected: FAIL — `main()` is a stub.

- [ ] **Step 3: Implement `main()` in `main.py`**

Replace stub `main()`:

```python
def main() -> None:
    config = load_config()
    last_provider, last_model = load_last()
    provider, model = select_provider_and_model(config, last_provider, last_model)
    save_last(provider, model)
    launch_claude(config["providers"][provider], model)
```

- [ ] **Step 4: Run the full test suite**

```bash
pytest tests/test_main.py -v
```

Expected: all tests PASSED (26 total).

- [ ] **Step 5: Commit**

```bash
git add claude_code_swapper/main.py tests/test_main.py
git commit -m "feat: wire main() and complete test suite"
```

---

## Task 8: README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Write `README.md`**

```markdown
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
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add README with install and usage instructions"
```

---

## Task 9: Smoke Test (manual)

- [ ] **Step 1: Verify install works**

```bash
pip install -e .
claude-code-swapper --help 2>&1 || claude-code-swapper
```

Expected: TUI appears with provider list (or first-launch config message if no config exists).

- [ ] **Step 2: Verify config bootstrap**

```bash
rm -f ~/.config/claude-code-swapper/config.yaml
claude-code-swapper
```

Expected: "Config created at ~/.config/claude-code-swapper/config.yaml" message, then exit.

- [ ] **Step 3: Run full test suite one final time**

```bash
pytest tests/ -v
```

Expected: all tests PASSED, no warnings.
