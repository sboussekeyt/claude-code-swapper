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


def launch_claude(provider_config: dict, model: str) -> None:
    if shutil.which("claude") is None:
        print("Error: 'claude' not found in PATH — is Claude Code installed?")
        sys.exit(1)

    env = os.environ.copy()
    env["ANTHROPIC_API_KEY"] = provider_config["api_key"]
    env["ANTHROPIC_BASE_URL"] = provider_config["base_url"]

    print(f"Launching claude with model {model}...")
    os.execvpe("claude", ["claude", "--model", model], env)


def main() -> None:
    pass
