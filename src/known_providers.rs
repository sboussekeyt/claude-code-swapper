use crate::discovery::ProviderKind;

/// A provider whose connection details ship with the binary, so a user only
/// needs to supply an `api_key` (and even that's optional for local servers
/// that don't really check it) to use it — nothing to copy-paste between
/// machines or share with a teammate. Matched against a provider's name in
/// `config.yaml` (the map key under `providers:`), not against `base_url`.
pub struct KnownProvider {
    pub name: &'static str,
    pub base_url: &'static str,
    pub kind: ProviderKind,
    /// A conventional placeholder for providers that don't actually check
    /// the key (LM Studio, Ollama) — `None` for anything that needs a real
    /// secret the user must supply themselves.
    pub default_api_key: Option<&'static str>,
}

/// Add a new entry here to make a provider available by name alone. This is
/// intentionally just data — the discovery *behavior* for `kind` lives in
/// `discovery::ProviderKind`/`ModelSource`, so adding a provider that needs
/// a new discovery strategy still means adding a `ModelSource` impl there
/// first, then pointing a `KnownProvider` at it here.
pub const KNOWN_PROVIDERS: &[KnownProvider] = &[
    KnownProvider {
        name: "anthropic",
        base_url: "https://api.anthropic.com",
        kind: ProviderKind::Generic,
        default_api_key: None,
    },
    KnownProvider {
        name: "openrouter",
        base_url: "https://openrouter.ai/api",
        kind: ProviderKind::Generic,
        default_api_key: None,
    },
    KnownProvider {
        name: "groq",
        base_url: "https://api.groq.com/openai/v1",
        kind: ProviderKind::Generic,
        default_api_key: None,
    },
    KnownProvider {
        name: "lmstudio",
        base_url: "http://localhost:1234",
        kind: ProviderKind::LmStudio,
        default_api_key: Some("lm-studio"),
    },
    KnownProvider {
        name: "ollama",
        base_url: "http://localhost:11434",
        kind: ProviderKind::Ollama,
        default_api_key: Some("ollama"),
    },
];

pub fn lookup(name: &str) -> Option<&'static KnownProvider> {
    KNOWN_PROVIDERS.iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_finds_known_providers_by_exact_name() {
        assert!(lookup("openrouter").is_some());
        assert!(lookup("lmstudio").is_some());
        assert!(lookup("ollama").is_some());
        assert!(lookup("OpenRouter").is_none(), "matching is case-sensitive, mirrors the config key");
        assert!(lookup("glm").is_none(), "not every provider is known — custom ones need a full config block");
    }

    #[test]
    fn local_servers_have_a_default_api_key_hosted_ones_dont() {
        assert_eq!(lookup("lmstudio").unwrap().default_api_key, Some("lm-studio"));
        assert_eq!(lookup("ollama").unwrap().default_api_key, Some("ollama"));
        assert_eq!(lookup("openrouter").unwrap().default_api_key, None);
        assert_eq!(lookup("anthropic").unwrap().default_api_key, None);
    }
}
