import os
import pytest
import yaml
from pathlib import Path
from unittest.mock import patch

from claude_code_swapper.main import load_config, load_last, save_last, select_provider_and_model, launch_claude, main

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
