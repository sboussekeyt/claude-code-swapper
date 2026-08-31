import json
import urllib.error

import pytest
import yaml
from pathlib import Path
from unittest.mock import MagicMock, patch

from claude_code_swapper.main import (
    load_config,
    load_last,
    save_last,
    save_config,
    display_status,
    select_provider_and_model,
    fetch_remote_models,
    add_model,
    remove_model,
    set_api_key,
    toggle_option,
    build_env,
    build_args,
    check_rtk_installed,
    install_rtk,
    prompt_rtk_install,
    ensure_rtk_hook,
    launch_claude,
    launch_claude_native,
    main,
    MENU_LAUNCH,
    MENU_LAUNCH_NATIVE,
    MENU_SELECT_MODEL,
    MENU_ADD_MODEL,
    MENU_REMOVE_MODEL,
    MENU_API_KEY,
    MENU_TOGGLE_RTK,
    MENU_TOGGLE_AUTO,
    MENU_QUIT,
    BACK,
)

SAMPLE_CONFIG = {
    "providers": {
        "openrouter": {
            "base_url": "https://openrouter.ai/api/v1",
            "api_key": "sk-or-test",
            "models": ["anthropic/claude-sonnet-4-6", "meta-llama/llama-3.1-8b"],
        },
        "groq": {
            "base_url": "https://api.groq.com/openai/v1",
            "api_key": "gsk-test",
            "models": ["llama-3.1-8b-instant"],
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


class TestSaveConfig:
    def test_writes_config(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        save_config(SAMPLE_CONFIG, config_path=config_file)
        data = yaml.safe_load(config_file.read_text())
        assert data["providers"]["openrouter"]["api_key"] == "sk-or-test"

    def test_creates_parent_dirs(self, tmp_path):
        config_file = tmp_path / "nested" / "dir" / "config.yaml"
        save_config(SAMPLE_CONFIG, config_path=config_file)
        assert config_file.exists()


class TestLoadLast:
    def test_returns_empty_dict_when_file_missing(self, tmp_path):
        result = load_last(last_path=tmp_path / "last.yaml")
        assert result == {}

    def test_returns_saved_values(self, tmp_path):
        last_file = tmp_path / "last.yaml"
        last_file.write_text(yaml.dump({"provider": "openrouter", "model": "claude-3"}))
        result = load_last(last_path=last_file)
        assert result["provider"] == "openrouter"
        assert result["model"] == "claude-3"

    def test_returns_empty_dict_on_corrupt_yaml(self, tmp_path):
        last_file = tmp_path / "last.yaml"
        last_file.write_text(": invalid :")
        assert load_last(last_path=last_file) == {}


class TestSaveLast:
    def test_writes_last_data(self, tmp_path):
        last_file = tmp_path / "last.yaml"
        save_last({"provider": "openrouter", "model": "claude-3"}, last_path=last_file)
        data = yaml.safe_load(last_file.read_text())
        assert data == {"model": "claude-3", "provider": "openrouter"}

    def test_creates_parent_directories(self, tmp_path):
        last_file = tmp_path / "nested" / "dir" / "last.yaml"
        save_last({"provider": "openrouter"}, last_path=last_file)
        assert last_file.exists()

    def test_saves_options(self, tmp_path):
        last_file = tmp_path / "last.yaml"
        save_last({"provider": "a", "rtk_enabled": True, "auto_accept": True}, last_path=last_file)
        data = yaml.safe_load(last_file.read_text())
        assert data["rtk_enabled"] is True
        assert data["auto_accept"] is True


class TestDisplayStatus:
    def test_shows_provider_and_model(self, capsys):
        display_status({"provider": "openrouter", "model": "claude-3"})
        out = capsys.readouterr().out
        assert "openrouter" in out
        assert "claude-3" in out

    def test_shows_no_selection(self, capsys):
        display_status({})
        out = capsys.readouterr().out
        assert "No model selected" in out

    def test_shows_rtk_on(self, capsys):
        display_status({"rtk_enabled": True})
        out = capsys.readouterr().out
        assert "ON" in out

    def test_shows_auto_accept_off(self, capsys):
        display_status({"auto_accept": False})
        out = capsys.readouterr().out
        assert "OFF" in out


class TestFetchRemoteModels:
    def _mock_response(self, payload: dict):
        mock_response = MagicMock()
        mock_response.read.return_value = json.dumps(payload).encode()
        mock_response.__enter__.return_value = mock_response
        return mock_response

    def test_returns_sorted_model_ids_on_success(self):
        with patch("urllib.request.urlopen") as mock_urlopen:
            mock_urlopen.return_value = self._mock_response(
                {"data": [{"id": "b-model"}, {"id": "a-model"}]}
            )
            result = fetch_remote_models("http://localhost:1234", "lm-studio")
        assert result == ["a-model", "b-model"]

    def test_sends_authorization_header_with_api_key(self):
        with patch("urllib.request.urlopen") as mock_urlopen:
            mock_urlopen.return_value = self._mock_response({"data": [{"id": "m"}]})
            fetch_remote_models("http://localhost:1234", "my-key")
            request = mock_urlopen.call_args[0][0]
        assert request.get_header("Authorization") == "Bearer my-key"

    def test_requests_models_endpoint(self):
        with patch("urllib.request.urlopen") as mock_urlopen:
            mock_urlopen.return_value = self._mock_response({"data": [{"id": "m"}]})
            fetch_remote_models("http://localhost:1234", "key")
            request = mock_urlopen.call_args[0][0]
        assert request.full_url == "http://localhost:1234/v1/models"

    def test_strips_trailing_slash_from_base_url(self):
        with patch("urllib.request.urlopen") as mock_urlopen:
            mock_urlopen.return_value = self._mock_response({"data": [{"id": "m"}]})
            fetch_remote_models("http://localhost:1234/", "key")
            request = mock_urlopen.call_args[0][0]
        assert request.full_url == "http://localhost:1234/v1/models"

    def test_returns_none_on_connection_error(self):
        with patch(
            "urllib.request.urlopen",
            side_effect=urllib.error.URLError("connection refused"),
        ):
            result = fetch_remote_models("http://localhost:1234", "key")
        assert result is None

    def test_returns_none_on_invalid_json(self):
        mock_response = MagicMock()
        mock_response.read.return_value = b"not json"
        mock_response.__enter__.return_value = mock_response
        with patch("urllib.request.urlopen", return_value=mock_response):
            result = fetch_remote_models("http://localhost:1234", "key")
        assert result is None

    def test_returns_none_when_data_missing(self):
        with patch("urllib.request.urlopen") as mock_urlopen:
            mock_urlopen.return_value = self._mock_response({"unexpected": []})
            result = fetch_remote_models("http://localhost:1234", "key")
        assert result is None

    def test_returns_none_when_data_empty(self):
        with patch("urllib.request.urlopen") as mock_urlopen:
            mock_urlopen.return_value = self._mock_response({"data": []})
            result = fetch_remote_models("http://localhost:1234", "key")
        assert result is None


class TestSelectProviderAndModel:
    @pytest.fixture(autouse=True)
    def no_remote_discovery(self):
        with patch("claude_code_swapper.main.fetch_remote_models", return_value=None):
            yield

    def test_selects_provider_and_model(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.side_effect = [
                "openrouter",
                "meta-llama/llama-3.1-8b",
            ]
            provider, model = select_provider_and_model(SAMPLE_CONFIG)
        assert provider == "openrouter"
        assert model == "meta-llama/llama-3.1-8b"

    def test_uses_discovered_models_when_available(self):
        with patch("questionary.select") as mock_select, \
             patch(
                 "claude_code_swapper.main.fetch_remote_models",
                 return_value=["discovered-model"],
             ):
            mock_select.return_value.ask.side_effect = ["openrouter", "discovered-model"]
            select_provider_and_model(SAMPLE_CONFIG)
            model_choices = mock_select.call_args_list[1][1]["choices"]
        assert model_choices == ["discovered-model", BACK]

    def test_falls_back_to_configured_models_when_discovery_fails(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.side_effect = [
                "openrouter",
                "anthropic/claude-sonnet-4-6",
            ]
            select_provider_and_model(SAMPLE_CONFIG)
            model_choices = mock_select.call_args_list[1][1]["choices"]
        assert model_choices == [
            "anthropic/claude-sonnet-4-6",
            "meta-llama/llama-3.1-8b",
            BACK,
        ]

    def test_passes_provider_base_url_and_api_key_to_discovery(self):
        with patch("questionary.select") as mock_select, \
             patch("claude_code_swapper.main.fetch_remote_models") as mock_fetch:
            mock_fetch.return_value = None
            mock_select.return_value.ask.side_effect = [
                "openrouter",
                "anthropic/claude-sonnet-4-6",
            ]
            select_provider_and_model(SAMPLE_CONFIG)
        mock_fetch.assert_called_once_with("https://openrouter.ai/api/v1", "sk-or-test")

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
            mock_select.return_value.ask.side_effect = ["groq", "llama-3.1-8b-instant"]
            select_provider_and_model(
                SAMPLE_CONFIG, last_provider="groq", last_model="llama-3.1-8b-instant"
            )
            provider_call_kwargs = mock_select.call_args_list[0][1]
        assert provider_call_kwargs["default"] == "groq"

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

    def test_returns_none_when_user_cancels_provider(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.return_value = None
            result = select_provider_and_model(SAMPLE_CONFIG)
        assert result is None

    def test_returns_none_when_user_cancels_model(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.side_effect = ["openrouter", None]
            result = select_provider_and_model(SAMPLE_CONFIG)
        assert result is None

    def test_returns_none_when_back_on_provider(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.return_value = BACK
            result = select_provider_and_model(SAMPLE_CONFIG)
        assert result is None

    def test_returns_none_when_back_on_model(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.side_effect = ["openrouter", BACK]
            result = select_provider_and_model(SAMPLE_CONFIG)
        assert result is None

    def test_back_is_in_provider_choices(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.return_value = BACK
            select_provider_and_model(SAMPLE_CONFIG)
            choices = mock_select.call_args_list[0][1]["choices"]
        assert BACK in choices

    def test_back_is_in_model_choices(self):
        with patch("questionary.select") as mock_select:
            mock_select.return_value.ask.side_effect = ["openrouter", BACK]
            select_provider_and_model(SAMPLE_CONFIG)
            choices = mock_select.call_args_list[1][1]["choices"]
        assert BACK in choices

    def test_returns_none_when_no_providers_configured(self):
        result = select_provider_and_model({"providers": {}})
        assert result is None

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


class TestAddModel:
    def test_adds_model_to_provider(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        config = {"providers": {"openrouter": {"models": ["model-a"]}}}
        with patch("questionary.select") as mock_sel, \
             patch("questionary.text") as mock_txt:
            mock_sel.return_value.ask.return_value = "openrouter"
            mock_txt.return_value.ask.return_value = "model-b"
            result = add_model(config, config_path=config_file)
        assert "model-b" in result["providers"]["openrouter"]["models"]

    def test_saves_config_after_add(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        config = {"providers": {"openrouter": {"models": ["model-a"]}}}
        with patch("questionary.select") as mock_sel, \
             patch("questionary.text") as mock_txt:
            mock_sel.return_value.ask.return_value = "openrouter"
            mock_txt.return_value.ask.return_value = "model-b"
            add_model(config, config_path=config_file)
        saved = yaml.safe_load(config_file.read_text())
        assert "model-b" in saved["providers"]["openrouter"]["models"]

    def test_does_not_add_duplicate(self, tmp_path, capsys):
        config_file = tmp_path / "config.yaml"
        config = {"providers": {"openrouter": {"models": ["model-a"]}}}
        with patch("questionary.select") as mock_sel, \
             patch("questionary.text") as mock_txt:
            mock_sel.return_value.ask.return_value = "openrouter"
            mock_txt.return_value.ask.return_value = "model-a"
            result = add_model(config, config_path=config_file)
        assert result["providers"]["openrouter"]["models"].count("model-a") == 1
        assert "already exists" in capsys.readouterr().out

    def test_cancel_provider_returns_unchanged(self, tmp_path):
        config = {"providers": {"openrouter": {"models": ["model-a"]}}}
        with patch("questionary.select") as mock_sel:
            mock_sel.return_value.ask.return_value = None
            result = add_model(config, config_path=tmp_path / "c.yaml")
        assert result == config

    def test_empty_model_name_returns_unchanged(self, tmp_path):
        config = {"providers": {"openrouter": {"models": ["model-a"]}}}
        with patch("questionary.select") as mock_sel, \
             patch("questionary.text") as mock_txt:
            mock_sel.return_value.ask.return_value = "openrouter"
            mock_txt.return_value.ask.return_value = ""
            result = add_model(config, config_path=tmp_path / "c.yaml")
        assert result["providers"]["openrouter"]["models"] == ["model-a"]

    def test_no_providers_prints_message(self, capsys):
        result = add_model({"providers": {}})
        assert "No providers" in capsys.readouterr().out

    def test_back_on_provider_returns_unchanged(self, tmp_path):
        config = {"providers": {"openrouter": {"models": ["model-a"]}}}
        with patch("questionary.select") as mock_sel:
            mock_sel.return_value.ask.return_value = BACK
            result = add_model(config, config_path=tmp_path / "c.yaml")
        assert result["providers"]["openrouter"]["models"] == ["model-a"]


class TestRemoveModel:
    def test_removes_model_from_provider(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        config = {"providers": {"openrouter": {"models": ["model-a", "model-b"]}}}
        with patch("questionary.select") as mock_sel:
            mock_sel.return_value.ask.side_effect = ["openrouter", "model-a"]
            result = remove_model(config, config_path=config_file)
        assert "model-a" not in result["providers"]["openrouter"]["models"]
        assert "model-b" in result["providers"]["openrouter"]["models"]

    def test_saves_config_after_remove(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        config = {"providers": {"openrouter": {"models": ["model-a", "model-b"]}}}
        with patch("questionary.select") as mock_sel:
            mock_sel.return_value.ask.side_effect = ["openrouter", "model-a"]
            remove_model(config, config_path=config_file)
        saved = yaml.safe_load(config_file.read_text())
        assert "model-a" not in saved["providers"]["openrouter"]["models"]

    def test_cancel_returns_unchanged(self, tmp_path):
        config = {"providers": {"openrouter": {"models": ["model-a"]}}}
        with patch("questionary.select") as mock_sel:
            mock_sel.return_value.ask.return_value = None
            result = remove_model(config, config_path=tmp_path / "c.yaml")
        assert result["providers"]["openrouter"]["models"] == ["model-a"]

    def test_no_providers_with_models(self, capsys):
        result = remove_model({"providers": {"empty": {"models": []}}})
        assert "No providers" in capsys.readouterr().out

    def test_back_on_provider_returns_unchanged(self, tmp_path):
        config = {"providers": {"openrouter": {"models": ["model-a"]}}}
        with patch("questionary.select") as mock_sel:
            mock_sel.return_value.ask.return_value = BACK
            result = remove_model(config, config_path=tmp_path / "c.yaml")
        assert result["providers"]["openrouter"]["models"] == ["model-a"]

    def test_back_on_model_returns_unchanged(self, tmp_path):
        config = {"providers": {"openrouter": {"models": ["model-a"]}}}
        with patch("questionary.select") as mock_sel:
            mock_sel.return_value.ask.side_effect = ["openrouter", BACK]
            result = remove_model(config, config_path=tmp_path / "c.yaml")
        assert result["providers"]["openrouter"]["models"] == ["model-a"]


class TestSetApiKey:
    def test_updates_api_key(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        config = {"providers": {"openrouter": {"api_key": "old-key", "models": []}}}
        with patch("questionary.select") as mock_sel, \
             patch("questionary.password") as mock_pw:
            mock_sel.return_value.ask.return_value = "openrouter"
            mock_pw.return_value.ask.return_value = "new-key"
            result = set_api_key(config, config_path=config_file)
        assert result["providers"]["openrouter"]["api_key"] == "new-key"

    def test_saves_config_after_key_change(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        config = {"providers": {"openrouter": {"api_key": "old-key", "models": []}}}
        with patch("questionary.select") as mock_sel, \
             patch("questionary.password") as mock_pw:
            mock_sel.return_value.ask.return_value = "openrouter"
            mock_pw.return_value.ask.return_value = "new-key"
            set_api_key(config, config_path=config_file)
        saved = yaml.safe_load(config_file.read_text())
        assert saved["providers"]["openrouter"]["api_key"] == "new-key"

    def test_shows_masked_key(self, tmp_path, capsys):
        config = {"providers": {"openrouter": {"api_key": "sk-or-very-long-key-123", "models": []}}}
        with patch("questionary.select") as mock_sel, \
             patch("questionary.password") as mock_pw:
            mock_sel.return_value.ask.return_value = "openrouter"
            mock_pw.return_value.ask.return_value = "new"
            set_api_key(config, config_path=tmp_path / "c.yaml")
        out = capsys.readouterr().out
        assert "sk-o" in out
        assert "-123" in out

    def test_cancel_returns_unchanged(self, tmp_path):
        config = {"providers": {"openrouter": {"api_key": "old", "models": []}}}
        with patch("questionary.select") as mock_sel:
            mock_sel.return_value.ask.return_value = None
            result = set_api_key(config, config_path=tmp_path / "c.yaml")
        assert result["providers"]["openrouter"]["api_key"] == "old"

    def test_empty_key_returns_unchanged(self, tmp_path):
        config = {"providers": {"openrouter": {"api_key": "old", "models": []}}}
        with patch("questionary.select") as mock_sel, \
             patch("questionary.password") as mock_pw:
            mock_sel.return_value.ask.return_value = "openrouter"
            mock_pw.return_value.ask.return_value = ""
            result = set_api_key(config, config_path=tmp_path / "c.yaml")
        assert result["providers"]["openrouter"]["api_key"] == "old"

    def test_back_returns_unchanged(self, tmp_path):
        config = {"providers": {"openrouter": {"api_key": "old", "models": []}}}
        with patch("questionary.select") as mock_sel:
            mock_sel.return_value.ask.return_value = BACK
            result = set_api_key(config, config_path=tmp_path / "c.yaml")
        assert result["providers"]["openrouter"]["api_key"] == "old"


class TestToggleOption:
    def test_toggles_off_to_on(self, tmp_path):
        last_file = tmp_path / "last.yaml"
        last = {"rtk_enabled": False}
        result = toggle_option(last, "rtk_enabled", last_path=last_file)
        assert result["rtk_enabled"] is True

    def test_toggles_on_to_off(self, tmp_path):
        last_file = tmp_path / "last.yaml"
        last = {"rtk_enabled": True}
        result = toggle_option(last, "rtk_enabled", last_path=last_file)
        assert result["rtk_enabled"] is False

    def test_toggles_missing_to_on(self, tmp_path):
        last_file = tmp_path / "last.yaml"
        result = toggle_option({}, "auto_accept", last_path=last_file)
        assert result["auto_accept"] is True

    def test_saves_after_toggle(self, tmp_path):
        last_file = tmp_path / "last.yaml"
        toggle_option({}, "rtk_enabled", last_path=last_file)
        data = yaml.safe_load(last_file.read_text())
        assert data["rtk_enabled"] is True


class TestBuildEnv:
    def test_proxy_mode_sets_auth_token(self):
        env = build_env({"api_key": "sk-or-test", "base_url": "https://openrouter.ai/api"})
        assert env["ANTHROPIC_AUTH_TOKEN"] == "sk-or-test"
        assert env["ANTHROPIC_API_KEY"] == ""
        assert env["ANTHROPIC_BASE_URL"] == "https://openrouter.ai/api"

    def test_preserves_existing_env_vars(self):
        with patch.dict("os.environ", {"MY_VAR": "my-value"}):
            env = build_env({"api_key": "k", "base_url": "u"})
        assert env["MY_VAR"] == "my-value"


class TestRtk:
    def test_check_rtk_installed_true(self):
        with patch("shutil.which", return_value="/usr/local/bin/rtk"):
            assert check_rtk_installed() is True

    def test_check_rtk_installed_false(self):
        with patch("shutil.which", return_value=None):
            assert check_rtk_installed() is False

    def test_install_rtk_runs_install_command(self):
        with patch("subprocess.run") as mock_run:
            install_rtk()
        assert mock_run.call_args[0][0] == (
            "curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh"
        )
        assert mock_run.call_args[1]["shell"] is True

    def test_prompt_skips_when_already_installed(self):
        with patch("claude_code_swapper.main.check_rtk_installed", return_value=True), \
             patch("questionary.confirm") as mock_confirm:
            prompt_rtk_install()
        mock_confirm.assert_not_called()

    def test_prompt_installs_when_user_confirms(self):
        with patch("claude_code_swapper.main.check_rtk_installed", return_value=False), \
             patch("questionary.confirm") as mock_confirm, \
             patch("claude_code_swapper.main.install_rtk") as mock_install:
            mock_confirm.return_value.ask.return_value = True
            prompt_rtk_install()
        mock_install.assert_called_once()

    def test_prompt_skips_install_when_user_declines(self):
        with patch("claude_code_swapper.main.check_rtk_installed", return_value=False), \
             patch("questionary.confirm") as mock_confirm, \
             patch("claude_code_swapper.main.install_rtk") as mock_install:
            mock_confirm.return_value.ask.return_value = False
            prompt_rtk_install()
        mock_install.assert_not_called()

    def test_ensure_rtk_hook_runs_init(self):
        with patch("subprocess.run") as mock_run:
            ensure_rtk_hook()
        assert mock_run.call_args[0][0] == ["rtk", "init", "--global", "--auto-patch"]


class TestLaunchClaude:
    def test_execs_claude_with_model_flag(self):
        provider_config = {
            "api_key": "sk-test",
            "base_url": "https://openrouter.ai/api",
        }
        with patch("shutil.which", return_value="/usr/bin/claude"), patch(
            "os.execvpe"
        ) as mock_exec:
            launch_claude(provider_config, "anthropic/claude-sonnet-4-6")
        args = mock_exec.call_args[0]
        assert args[0] == "claude"
        assert args[1] == ["claude", "--model", "anthropic/claude-sonnet-4-6"]

    def test_proxy_env_vars(self):
        provider_config = {
            "api_key": "sk-or-test",
            "base_url": "https://openrouter.ai/api",
        }
        with patch("shutil.which", return_value="/usr/bin/claude"), patch(
            "os.execvpe"
        ) as mock_exec:
            launch_claude(provider_config, "some-model")
        env = mock_exec.call_args[0][2]
        assert env["ANTHROPIC_AUTH_TOKEN"] == "sk-or-test"
        assert env["ANTHROPIC_API_KEY"] == ""
        assert env["ANTHROPIC_BASE_URL"] == "https://openrouter.ai/api"

    def test_exits_when_claude_not_found(self, capsys):
        with patch("shutil.which", return_value=None), pytest.raises(
            SystemExit
        ) as exc:
            launch_claude({"api_key": "k", "base_url": "u"}, "model")
        assert exc.value.code == 1
        assert "claude" in capsys.readouterr().out.lower()

    def test_adds_auto_accept_flag(self):
        provider_config = {"api_key": "k", "base_url": "u"}
        with patch("shutil.which", return_value="/usr/bin/claude"), patch(
            "os.execvpe"
        ) as mock_exec:
            launch_claude(provider_config, "model", auto_accept=True)
        args = mock_exec.call_args[0][1]
        assert "--dangerously-skip-permissions" in args

    def test_no_flags_by_default(self):
        provider_config = {"api_key": "k", "base_url": "u"}
        with patch("shutil.which", return_value="/usr/bin/claude"), patch(
            "os.execvpe"
        ) as mock_exec:
            launch_claude(provider_config, "model")
        args = mock_exec.call_args[0][1]
        assert "--dangerously-skip-permissions" not in args

    def test_rtk_enabled_runs_hook_before_launch(self):
        provider_config = {"api_key": "k", "base_url": "u"}
        with patch("shutil.which", return_value="/usr/bin/claude"), \
             patch("os.execvpe") as mock_exec, \
             patch("claude_code_swapper.main.ensure_rtk_hook") as mock_hook:
            launch_claude(provider_config, "model", rtk_enabled=True)
        mock_hook.assert_called_once()
        mock_exec.assert_called_once()

    def test_rtk_disabled_skips_hook(self):
        provider_config = {"api_key": "k", "base_url": "u"}
        with patch("shutil.which", return_value="/usr/bin/claude"), \
             patch("os.execvpe"), \
             patch("claude_code_swapper.main.ensure_rtk_hook") as mock_hook:
            launch_claude(provider_config, "model", rtk_enabled=False)
        mock_hook.assert_not_called()


class TestLaunchClaudeNative:
    def test_execs_claude_without_model_flag(self):
        with patch("shutil.which", return_value="/usr/bin/claude"), patch(
            "os.execvpe"
        ) as mock_exec:
            launch_claude_native()
        args = mock_exec.call_args[0]
        assert args[0] == "claude"
        assert args[1] == ["claude"]

    def test_does_not_set_proxy_env_vars(self):
        with patch.dict("os.environ", {"MY_VAR": "my-value"}), \
             patch("shutil.which", return_value="/usr/bin/claude"), \
             patch("os.execvpe") as mock_exec:
            launch_claude_native()
        env = mock_exec.call_args[0][2]
        assert "ANTHROPIC_BASE_URL" not in env
        assert "ANTHROPIC_AUTH_TOKEN" not in env
        assert env["MY_VAR"] == "my-value"

    def test_exits_when_claude_not_found(self, capsys):
        with patch("shutil.which", return_value=None), pytest.raises(
            SystemExit
        ) as exc:
            launch_claude_native()
        assert exc.value.code == 1
        assert "claude" in capsys.readouterr().out.lower()

    def test_adds_auto_accept_flag(self):
        with patch("shutil.which", return_value="/usr/bin/claude"), patch(
            "os.execvpe"
        ) as mock_exec:
            launch_claude_native(auto_accept=True)
        args = mock_exec.call_args[0][1]
        assert "--dangerously-skip-permissions" in args

    def test_no_flags_by_default(self):
        with patch("shutil.which", return_value="/usr/bin/claude"), patch(
            "os.execvpe"
        ) as mock_exec:
            launch_claude_native()
        args = mock_exec.call_args[0][1]
        assert "--dangerously-skip-permissions" not in args

    def test_rtk_enabled_runs_hook_before_launch(self):
        with patch("shutil.which", return_value="/usr/bin/claude"), \
             patch("os.execvpe") as mock_exec, \
             patch("claude_code_swapper.main.ensure_rtk_hook") as mock_hook:
            launch_claude_native(rtk_enabled=True)
        mock_hook.assert_called_once()
        mock_exec.assert_called_once()

    def test_rtk_disabled_skips_hook(self):
        with patch("shutil.which", return_value="/usr/bin/claude"), \
             patch("os.execvpe"), \
             patch("claude_code_swapper.main.ensure_rtk_hook") as mock_hook:
            launch_claude_native(rtk_enabled=False)
        mock_hook.assert_not_called()


class TestMain:
    @pytest.fixture(autouse=True)
    def no_remote_discovery(self):
        with patch("claude_code_swapper.main.fetch_remote_models", return_value=None):
            yield

    def test_select_then_launch(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("claude_code_swapper.main.check_rtk_installed", return_value=True), \
             patch("questionary.select") as mock_select, \
             patch("shutil.which", return_value="/usr/bin/claude"), \
             patch("os.execvpe", side_effect=SystemExit(0)) as mock_exec, \
             pytest.raises(SystemExit):
            mock_select.return_value.ask.side_effect = [
                MENU_SELECT_MODEL,
                "openrouter",
                "anthropic/claude-sonnet-4-6",
                MENU_LAUNCH,
            ]
            main()

        env = mock_exec.call_args[0][2]
        assert env["ANTHROPIC_AUTH_TOKEN"] == "sk-or-test"
        assert env["ANTHROPIC_API_KEY"] == ""
        assert env["ANTHROPIC_BASE_URL"] == "https://openrouter.ai/api/v1"
        assert mock_exec.call_args[0][1] == ["claude", "--model", "anthropic/claude-sonnet-4-6"]

    def test_launch_native_without_provider_or_model(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("claude_code_swapper.main.check_rtk_installed", return_value=True), \
             patch("questionary.select") as mock_select, \
             patch("shutil.which", return_value="/usr/bin/claude"), \
             patch("os.execvpe", side_effect=SystemExit(0)) as mock_exec, \
             pytest.raises(SystemExit):
            mock_select.return_value.ask.side_effect = [MENU_LAUNCH_NATIVE]
            main()

        assert mock_exec.call_args[0][1] == ["claude"]
        env = mock_exec.call_args[0][2]
        assert "ANTHROPIC_BASE_URL" not in env
        assert "ANTHROPIC_AUTH_TOKEN" not in env

    def test_launch_uses_previously_saved_model(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))
        last_file.write_text(yaml.dump({"provider": "groq", "model": "llama-3.1-8b-instant"}))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("claude_code_swapper.main.check_rtk_installed", return_value=True), \
             patch("questionary.select") as mock_select, \
             patch("shutil.which", return_value="/usr/bin/claude"), \
             patch("os.execvpe", side_effect=SystemExit(0)) as mock_exec, \
             pytest.raises(SystemExit):
            mock_select.return_value.ask.return_value = MENU_LAUNCH
            main()

        assert mock_exec.call_args[0][1] == ["claude", "--model", "llama-3.1-8b-instant"]

    def test_launch_without_model_shows_message(self, tmp_path, capsys):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("claude_code_swapper.main.check_rtk_installed", return_value=True), \
             patch("questionary.select") as mock_select, \
             pytest.raises(SystemExit):
            mock_select.return_value.ask.side_effect = [MENU_LAUNCH, MENU_QUIT]
            main()

        assert "No model selected" in capsys.readouterr().out

    def test_saves_last_selection(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("claude_code_swapper.main.check_rtk_installed", return_value=True), \
             patch("questionary.select") as mock_select, \
             pytest.raises(SystemExit):
            mock_select.return_value.ask.side_effect = [
                MENU_SELECT_MODEL,
                "groq",
                "llama-3.1-8b-instant",
                MENU_QUIT,
            ]
            main()

        data = yaml.safe_load(last_file.read_text())
        assert data["provider"] == "groq"
        assert data["model"] == "llama-3.1-8b-instant"

    def test_quit_exits(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("claude_code_swapper.main.check_rtk_installed", return_value=True), \
             patch("questionary.select") as mock_select, \
             pytest.raises(SystemExit) as exc:
            mock_select.return_value.ask.return_value = MENU_QUIT
            main()
        assert exc.value.code == 0

    def test_cancel_menu_exits(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("claude_code_swapper.main.check_rtk_installed", return_value=True), \
             patch("questionary.select") as mock_select, \
             pytest.raises(SystemExit) as exc:
            mock_select.return_value.ask.return_value = None
            main()
        assert exc.value.code == 0

    def test_escape_in_select_model_returns_to_menu(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("claude_code_swapper.main.check_rtk_installed", return_value=True), \
             patch("questionary.select") as mock_select, \
             pytest.raises(SystemExit):
            # Select model -> Escape (None for provider) -> back to menu -> Quit
            mock_select.return_value.ask.side_effect = [
                MENU_SELECT_MODEL,
                None,  # Escape on provider select
                MENU_QUIT,
            ]
            main()

        # last.yaml should not exist (no selection was made)
        assert not last_file.exists()

    def test_escape_on_model_returns_to_menu(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("claude_code_swapper.main.check_rtk_installed", return_value=True), \
             patch("questionary.select") as mock_select, \
             pytest.raises(SystemExit):
            # Select model -> pick provider -> Escape on model -> back to menu -> Quit
            mock_select.return_value.ask.side_effect = [
                MENU_SELECT_MODEL,
                "openrouter",
                None,  # Escape on model select
                MENU_QUIT,
            ]
            main()

        assert not last_file.exists()

    def test_toggle_rtk_menu_option(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("claude_code_swapper.main.check_rtk_installed", return_value=True), \
             patch("questionary.select") as mock_select, \
             pytest.raises(SystemExit):
            mock_select.return_value.ask.side_effect = [MENU_TOGGLE_RTK, MENU_QUIT]
            main()

        data = yaml.safe_load(last_file.read_text())
        assert data["rtk_enabled"] is True

    def test_launch_with_rtk_enabled_runs_hook(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))
        last_file.write_text(yaml.dump({
            "provider": "openrouter",
            "model": "anthropic/claude-sonnet-4-6",
            "rtk_enabled": True,
        }))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("claude_code_swapper.main.check_rtk_installed", return_value=True), \
             patch("claude_code_swapper.main.ensure_rtk_hook") as mock_hook, \
             patch("questionary.select") as mock_select, \
             patch("shutil.which", return_value="/usr/bin/claude"), \
             patch("os.execvpe", side_effect=SystemExit(0)), \
             pytest.raises(SystemExit):
            mock_select.return_value.ask.return_value = MENU_LAUNCH
            main()

        mock_hook.assert_called_once()

    def test_prompts_rtk_install_when_missing(self, tmp_path):
        config_file = tmp_path / "config.yaml"
        last_file = tmp_path / "last.yaml"
        config_file.write_text(yaml.dump(SAMPLE_CONFIG))

        with patch("claude_code_swapper.main.CONFIG_PATH", config_file), \
             patch("claude_code_swapper.main.LAST_PATH", last_file), \
             patch("claude_code_swapper.main.check_rtk_installed", return_value=False), \
             patch("questionary.confirm") as mock_confirm, \
             patch("claude_code_swapper.main.install_rtk") as mock_install, \
             patch("questionary.select") as mock_select, \
             pytest.raises(SystemExit):
            mock_confirm.return_value.ask.return_value = False
            mock_select.return_value.ask.return_value = MENU_QUIT
            main()

        mock_confirm.assert_called_once()
        mock_install.assert_not_called()
