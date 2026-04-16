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
