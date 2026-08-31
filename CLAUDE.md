# claude-code-swapper

CLI interactive (TUI) pour lancer Claude Code avec différents providers LLM (OpenRouter, Groq, etc.) via les variables d'environnement `ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN`.

## Commands

```bash
# Installer en mode développement
pip install -e .

# Lancer l'outil
claude-code-swapper

# Tests
pytest

# Tests avec verbose
pytest -v
```

## Architecture

```
claude_code_swapper/
  main.py               # Tout le code (config, menu, launch)
  config.example.yaml   # Template copié au premier lancement
tests/
  test_main.py          # Suite de tests complète
```

- **Config** : `~/.config/claude-code-swapper/config.yaml` (providers + api_keys + models)
- **État persistant** : `~/.config/claude-code-swapper/last.yaml` (dernier provider/model/options)
- Premier lancement sans config → copie `config.example.yaml` et exit

## Modes de lancement

| Mode | Description |
|------|-------------|
| **Launch** | Lance Claude via proxy (ANTHROPIC_BASE_URL + ANTHROPIC_AUTH_TOKEN) |
| **Launch (native)** | Lance `claude` sans override d'env (`os.environ` inchangé, pas de `--model`) — utilise la config/login natif de Claude Code |

### Proxy

Tous les providers de la config sont traités en mode proxy : `ANTHROPIC_BASE_URL` + `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY=""`.

### Native

`launch_claude_native()` ne touche à aucune variable `ANTHROPIC_*` et n'ajoute pas `--model` : Claude Code se comporte comme s'il était lancé directement (compte/API key natifs). Les toggles RTK et auto-accept restent appliqués.

## RTK

[RTK](https://www.rtk-ai.app) compresse les sorties de commandes avant qu'elles n'atteignent le contexte de Claude, pour réduire la conso de tokens. Pas de démon : c'est un hook (`PreToolUse`) activé via `rtk init --global`.

- Au lancement de `claude-code-swapper`, si `rtk` n'est pas dans le PATH, `prompt_rtk_install()` propose de l'installer (script officiel via `curl | sh`)
- `MENU_TOGGLE_RTK` bascule `last["rtk_enabled"]`
- Si activé, `ensure_rtk_hook()` relance `rtk init --global` avant **chaque** lancement de `claude` (dans `launch_claude()`), pour garantir que le hook reste actif

## Découverte de modèles

`fetch_remote_models(base_url, api_key)` interroge `{base_url}/v1/models` (format OpenAI, header `Authorization: Bearer`) pour lister les modèles réellement disponibles chez un provider local/OpenAI-compatible (LM Studio, Ollama...). En cas d'échec (serveur injoignable, timeout, JSON invalide) → retourne `None` silencieusement, et `select_provider_and_model` retombe sur la liste statique `models:` du config.yaml.

## Gotchas

- **`base_url` ne doit JAMAIS inclure `/v1` en suffixe.** `claude` ajoute lui-même `/v1/messages` à `ANTHROPIC_BASE_URL`. Un `base_url` du style `http://localhost:1234/v1` produit donc `POST /v1/v1/messages` — la plupart des serveurs (dont LM Studio) répondent 200 avec un JSON invalide plutôt qu'une 404 propre, ce qui remonte côté Claude Code comme "API returned an empty or malformed response" / "not a Message". Toujours utiliser la racine du provider (ex: `http://localhost:1234`, pas `http://localhost:1234/v1`).
- `os.execvpe` remplace le processus courant → pas de retour possible après `launch_claude()`
- `auto_accept` passe `--dangerously-skip-permissions` à Claude
- Les tests mockent `questionary.select/text/password/confirm` via `unittest.mock.patch`
- Les tests qui appellent `main()` doivent mocker `claude_code_swapper.main.check_rtk_installed` (sinon `prompt_rtk_install()` fait un vrai `shutil.which` + un `questionary.confirm` non mocké)
- Tous les chemins de config sont passés en paramètre aux fonctions (testabilité avec `tmp_path`)
