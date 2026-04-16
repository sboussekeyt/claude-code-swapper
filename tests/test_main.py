import os
import pytest
import yaml
from pathlib import Path
from unittest.mock import patch

from claude_code_swapper.main import load_config, load_last, save_last

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
