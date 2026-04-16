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


def main() -> None:
    pass
