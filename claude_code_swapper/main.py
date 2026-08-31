import json
import os
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
from importlib.resources import files
from pathlib import Path

import questionary
import yaml

CONFIG_DIR = Path.home() / ".config" / "claude-code-swapper"
CONFIG_PATH = CONFIG_DIR / "config.yaml"
LAST_PATH = CONFIG_DIR / "last.yaml"

BACK = "← Back"


def load_config(config_path: Path = CONFIG_PATH) -> dict:
    if not config_path.exists():
        config_path.parent.mkdir(parents=True, exist_ok=True)
        example = (
            files("claude_code_swapper").joinpath("config.example.yaml").read_text()
        )
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


def save_config(config: dict, config_path: Path = CONFIG_PATH) -> None:
    config_path.parent.mkdir(parents=True, exist_ok=True)
    with open(config_path, "w") as f:
        yaml.dump(config, f, default_flow_style=False)


def load_last(last_path: Path = LAST_PATH) -> dict:
    if not last_path.exists():
        return {}
    try:
        with open(last_path) as f:
            return yaml.safe_load(f) or {}
    except yaml.YAMLError:
        return {}


def save_last(last: dict, last_path: Path = LAST_PATH) -> None:
    last_path.parent.mkdir(parents=True, exist_ok=True)
    with open(last_path, "w") as f:
        yaml.dump(last, f, default_flow_style=False)


def display_status(last: dict) -> None:
    provider = last.get("provider")
    model = last.get("model")
    rtk_enabled = last.get("rtk_enabled", False)
    auto_accept = last.get("auto_accept", False)

    print("\n╭─ Claude Code Swapper ─────────────────────╮")
    if provider and model:
        print(f"│  Provider : {provider:<30}│")
        print(f"│  Model    : {model:<30}│")
    else:
        print("│  No model selected yet                    │")
    print(f"│  RTK      : {'ON' if rtk_enabled else 'OFF':<30}│")
    print(f"│  Auto-accept : {'ON' if auto_accept else 'OFF':<30}│")
    print("╰───────────────────────────────────────────╯\n")


def fetch_remote_models(base_url: str, api_key: str, timeout: float = 1.5) -> list[str] | None:
    url = base_url.rstrip("/") + "/v1/models"
    request = urllib.request.Request(url, headers={"Authorization": f"Bearer {api_key}"})
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            payload = json.loads(response.read())
    except (urllib.error.URLError, TimeoutError, OSError, ValueError):
        return None

    data = payload.get("data") if isinstance(payload, dict) else None
    if not isinstance(data, list):
        return None

    ids = [m["id"] for m in data if isinstance(m, dict) and "id" in m]
    return sorted(ids) if ids else None


def select_provider_and_model(
    config: dict,
    last_provider: str | None = None,
    last_model: str | None = None,
) -> tuple[str, str] | None:
    providers = [p for p, v in config.get("providers", {}).items() if v.get("models")]
    if not providers:
        print("No providers with models configured.")
        print("Edit ~/.config/claude-code-swapper/config.yaml to add providers.")
        return None

    default_provider = last_provider if last_provider in providers else None
    provider = questionary.select(
        "Select a provider:",
        choices=providers + [BACK],
        default=default_provider,
    ).ask()

    if provider is None or provider == BACK:
        return None

    provider_config = config["providers"][provider]
    discovered = fetch_remote_models(provider_config["base_url"], provider_config["api_key"])
    models = discovered if discovered else provider_config["models"]

    default_model = last_model if last_model in models else None
    model = questionary.select(
        "Select a model:",
        choices=models + [BACK],
        default=default_model,
    ).ask()

    if model is None or model == BACK:
        return None

    return provider, model


def add_model(config: dict, config_path: Path = CONFIG_PATH) -> dict:
    providers = list(config.get("providers", {}).keys())
    if not providers:
        print("No providers configured. Add a provider first.")
        return config

    provider = questionary.select("Add model to which provider:", choices=providers + [BACK]).ask()
    if provider is None or provider == BACK:
        return config

    model_name = questionary.text("Model name to add:").ask()
    if not model_name:
        return config

    if model_name in config["providers"][provider].get("models", []):
        print(f"Model '{model_name}' already exists in {provider}.")
        return config

    config["providers"][provider].setdefault("models", []).append(model_name)
    save_config(config, config_path)
    print(f"Added '{model_name}' to {provider}.")
    return config


def remove_model(config: dict, config_path: Path = CONFIG_PATH) -> dict:
    providers = [p for p, v in config.get("providers", {}).items() if v.get("models")]
    if not providers:
        print("No providers with models found.")
        return config

    provider = questionary.select("Remove model from which provider:", choices=providers + [BACK]).ask()
    if provider is None or provider == BACK:
        return config

    models = config["providers"][provider]["models"]
    model = questionary.select("Select model to remove:", choices=models + [BACK]).ask()
    if model is None or model == BACK:
        return config

    models.remove(model)
    save_config(config, config_path)
    print(f"Removed '{model}' from {provider}.")
    return config


def set_api_key(config: dict, config_path: Path = CONFIG_PATH) -> dict:
    providers = list(config.get("providers", {}).keys())
    if not providers:
        print("No providers configured.")
        return config

    provider = questionary.select("Set API key for which provider:", choices=providers + [BACK]).ask()
    if provider is None or provider == BACK:
        return config

    current = config["providers"][provider].get("api_key", "")
    masked = current[:4] + "..." + current[-4:] if len(current) > 8 else "not set"
    print(f"Current key: {masked}")

    new_key = questionary.password("New API key:").ask()
    if not new_key:
        return config

    config["providers"][provider]["api_key"] = new_key
    save_config(config, config_path)
    print(f"API key updated for {provider}.")
    return config


def toggle_option(last: dict, option: str, last_path: Path = LAST_PATH) -> dict:
    current = last.get(option, False)
    last[option] = not current
    save_last(last, last_path)
    label = option.replace("_", "-")
    print(f"{label} is now {'ON' if last[option] else 'OFF'}.")
    return last


def build_env(provider_config: dict) -> dict:
    env = os.environ.copy()
    env["ANTHROPIC_BASE_URL"] = provider_config["base_url"]
    env["ANTHROPIC_AUTH_TOKEN"] = provider_config["api_key"]
    env["ANTHROPIC_API_KEY"] = ""
    return env


def build_args(model: str | None, auto_accept: bool = False) -> list[str]:
    args = ["claude"]
    if model:
        args += ["--model", model]
    if auto_accept:
        args.append("--dangerously-skip-permissions")
    return args


def check_claude() -> None:
    if shutil.which("claude") is None:
        print("Error: 'claude' not found in PATH — is Claude Code installed?")
        sys.exit(1)


RTK_INSTALL_CMD = (
    "curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh"
)


def check_rtk_installed() -> bool:
    return shutil.which("rtk") is not None


def install_rtk() -> None:
    print("Installing RTK...")
    subprocess.run(RTK_INSTALL_CMD, shell=True)


def prompt_rtk_install() -> None:
    if check_rtk_installed():
        return
    install = questionary.confirm(
        "RTK is not installed (compresses tool output to save tokens). Install it now?",
        default=False,
    ).ask()
    if install:
        install_rtk()


def ensure_rtk_hook() -> None:
    subprocess.run(
        ["rtk", "init", "--global", "--auto-patch"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def launch_claude(
    provider_config: dict,
    model: str,
    auto_accept: bool = False,
    rtk_enabled: bool = False,
) -> None:
    check_claude()

    if rtk_enabled:
        ensure_rtk_hook()

    env = build_env(provider_config)
    args = build_args(model, auto_accept)

    print(f"Launching claude with model {model}...")
    os.execvpe("claude", args, env)


def launch_claude_native(
    auto_accept: bool = False,
    rtk_enabled: bool = False,
) -> None:
    check_claude()

    if rtk_enabled:
        ensure_rtk_hook()

    env = os.environ.copy()
    args = build_args(None, auto_accept)

    print("Launching claude natively (no provider override)...")
    os.execvpe("claude", args, env)


MENU_LAUNCH = "Launch Claude"
MENU_LAUNCH_NATIVE = "Launch Claude (native)"
MENU_SELECT_MODEL = "Select model"
MENU_ADD_MODEL = "Add a model"
MENU_REMOVE_MODEL = "Remove a model"
MENU_API_KEY = "Set API key"
MENU_TOGGLE_RTK = "Toggle RTK mode"
MENU_TOGGLE_AUTO = "Toggle auto-accept mode"
MENU_QUIT = "Quit"

MENU_CHOICES = [
    MENU_LAUNCH,
    MENU_LAUNCH_NATIVE,
    MENU_SELECT_MODEL,
    MENU_ADD_MODEL,
    MENU_REMOVE_MODEL,
    MENU_API_KEY,
    MENU_TOGGLE_RTK,
    MENU_TOGGLE_AUTO,
    MENU_QUIT,
]


def main() -> None:
    config = load_config(CONFIG_PATH)
    last = load_last(LAST_PATH)
    prompt_rtk_install()

    while True:
        display_status(last)

        action = questionary.select("What do you want to do?", choices=MENU_CHOICES).ask()
        if action is None or action == MENU_QUIT:
            sys.exit(0)

        if action == MENU_LAUNCH:
            provider = last.get("provider")
            model = last.get("model")
            if not provider or not model or provider not in config.get("providers", {}):
                print("No model selected. Please select a model first.")
                continue
            save_last(last, LAST_PATH)
            launch_claude(
                config["providers"][provider],
                model,
                auto_accept=last.get("auto_accept", False),
                rtk_enabled=last.get("rtk_enabled", False),
            )
        elif action == MENU_LAUNCH_NATIVE:
            save_last(last, LAST_PATH)
            launch_claude_native(
                auto_accept=last.get("auto_accept", False),
                rtk_enabled=last.get("rtk_enabled", False),
            )
        elif action == MENU_SELECT_MODEL:
            result = select_provider_and_model(
                config, last.get("provider"), last.get("model")
            )
            if result is not None:
                last["provider"] = result[0]
                last["model"] = result[1]
                save_last(last, LAST_PATH)
        elif action == MENU_ADD_MODEL:
            config = add_model(config, CONFIG_PATH)
        elif action == MENU_REMOVE_MODEL:
            config = remove_model(config, CONFIG_PATH)
        elif action == MENU_API_KEY:
            config = set_api_key(config, CONFIG_PATH)
        elif action == MENU_TOGGLE_RTK:
            last = toggle_option(last, "rtk_enabled", LAST_PATH)
        elif action == MENU_TOGGLE_AUTO:
            last = toggle_option(last, "auto_accept", LAST_PATH)


if __name__ == "__main__":
    main()
