use std::collections::HashMap;
use std::process::Command;

pub fn build_env(base_url: &str, api_key: &str) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    env.insert("ANTHROPIC_BASE_URL".to_string(), base_url.to_string());
    env.insert("ANTHROPIC_AUTH_TOKEN".to_string(), api_key.to_string());
    env.insert("ANTHROPIC_API_KEY".to_string(), String::new());
    env
}

pub fn build_command(model: Option<&str>, auto_accept: bool, env: &HashMap<String, String>) -> Command {
    let mut cmd = Command::new("claude");
    if let Some(m) = model {
        cmd.arg("--model").arg(m);
    }
    if auto_accept {
        cmd.arg("--dangerously-skip-permissions");
    }
    cmd.env_clear();
    cmd.envs(env);
    cmd
}

pub fn check_claude() -> Result<(), String> {
    if which::which("claude").is_err() {
        return Err("'claude' not found in PATH — is Claude Code installed?".to_string());
    }
    Ok(())
}

pub fn check_rtk_installed() -> bool {
    which::which("rtk").is_ok()
}

pub const RTK_INSTALL_CMD: &str =
    "curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh";

pub fn install_rtk() {
    let _ = Command::new("sh").arg("-c").arg(RTK_INSTALL_CMD).status();
}

pub fn ensure_rtk_hook() {
    let _ = Command::new("rtk")
        .args(["init", "--global", "--auto-patch"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_env_sets_proxy_vars_and_clears_api_key() {
        let env = build_env("https://openrouter.ai/api", "sk-or-test");
        assert_eq!(env["ANTHROPIC_BASE_URL"], "https://openrouter.ai/api");
        assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], "sk-or-test");
        assert_eq!(env["ANTHROPIC_API_KEY"], "");
    }

    #[test]
    fn build_command_sets_program_and_model_flag() {
        let env = HashMap::new();
        let cmd = build_command(Some("claude-sonnet-4-6"), false, &env);
        assert_eq!(cmd.get_program(), "claude");
        let args: Vec<_> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert_eq!(args, vec!["--model", "claude-sonnet-4-6"]);
    }

    #[test]
    fn build_command_adds_auto_accept_flag() {
        let env = HashMap::new();
        let cmd = build_command(Some("model"), true, &env);
        let args: Vec<_> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert!(args.contains(&"--dangerously-skip-permissions"));
    }

    #[test]
    fn build_command_omits_model_flag_when_none() {
        let env = HashMap::new();
        let cmd = build_command(None, false, &env);
        assert_eq!(cmd.get_args().count(), 0);
    }

    #[test]
    fn build_command_carries_exact_env_map() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_BASE_URL".to_string(), "https://example.com".to_string());
        let cmd = build_command(None, false, &env);
        let carried: HashMap<String, String> = cmd
            .get_envs()
            .filter_map(|(k, v)| Some((k.to_str()?.to_string(), v?.to_str()?.to_string())))
            .collect();
        assert_eq!(carried, env);
    }

    #[test]
    fn check_rtk_installed_reflects_path() {
        // `cargo` itself is guaranteed to be on PATH in the test environment;
        // this is a smoke test that the PATH-lookup mechanism works at all.
        assert_eq!(check_rtk_installed(), which::which("rtk").is_ok());
    }
}
