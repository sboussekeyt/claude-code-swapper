use serde::{Deserialize, Serialize};
use std::time::Duration;

/// One entry from a provider's models listing. `context_length` is populated
/// when the source reports it (OpenRouter's `/v1/models`, LM Studio's native
/// API); sources that don't (Ollama's `/api/tags`, a bare OpenAI-compatible
/// server) leave it `None`, and the caller falls back to a manually
/// configured `context_windows` entry.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DiscoveredModel {
    pub id: String,
    #[serde(default)]
    pub context_length: Option<u64>,
}

/// How to ask a provider what models it currently has available. Each kind
/// is a `ModelSource` impl below — add a new provider by adding a struct, an
/// impl, and one match arm in `ProviderKind::discover`; nothing else in the
/// app (config schema aside) needs to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    /// Any OpenAI-compatible `/v1/models` endpoint: OpenRouter, Groq,
    /// Anthropic-compatible proxies, and LM Studio/Ollama if `kind` isn't
    /// set (they both also expose this, just with less detail than their
    /// native APIs).
    #[default]
    Generic,
    /// LM Studio's native REST API (`/api/v0/models`) — reports every
    /// downloaded model (not just the currently loaded one) plus its
    /// context length. Falls back to the generic `/v1/models` path if the
    /// native endpoint isn't reachable or doesn't parse (older LM Studio
    /// versions, or something else running on the same port).
    LmStudio,
    /// Ollama's native API (`/api/tags`) — lists every locally pulled
    /// model, not just whichever one is currently loaded into memory.
    Ollama,
}

impl ProviderKind {
    pub fn discover(self, base_url: &str, api_key: &str, timeout: Duration) -> Option<Vec<DiscoveredModel>> {
        let source: &dyn ModelSource = match self {
            ProviderKind::Generic => &GenericOpenAi,
            ProviderKind::LmStudio => &LmStudioSource,
            ProviderKind::Ollama => &OllamaSource,
        };
        source.discover(base_url, api_key, timeout)
    }
}

trait ModelSource {
    fn discover(&self, base_url: &str, api_key: &str, timeout: Duration) -> Option<Vec<DiscoveredModel>>;
}

struct GenericOpenAi;

impl ModelSource for GenericOpenAi {
    fn discover(&self, base_url: &str, api_key: &str, timeout: Duration) -> Option<Vec<DiscoveredModel>> {
        fetch_remote_models(base_url, api_key, timeout)
    }
}

struct OllamaSource;

impl ModelSource for OllamaSource {
    fn discover(&self, base_url: &str, _api_key: &str, timeout: Duration) -> Option<Vec<DiscoveredModel>> {
        fetch_ollama_tags(base_url, timeout)
    }
}

struct LmStudioSource;

impl ModelSource for LmStudioSource {
    fn discover(&self, base_url: &str, api_key: &str, timeout: Duration) -> Option<Vec<DiscoveredModel>> {
        fetch_lmstudio_native(base_url, timeout).or_else(|| fetch_remote_models(base_url, api_key, timeout))
    }
}

#[derive(Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<DiscoveredModel>,
}

pub fn fetch_remote_models(base_url: &str, api_key: &str, timeout: Duration) -> Option<Vec<DiscoveredModel>> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let response = agent
        .get(&url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .call()
        .ok()?;
    let parsed: ModelsResponse = response.into_json().ok()?;
    if parsed.data.is_empty() {
        return None;
    }
    let mut models = parsed.data;
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Some(models)
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaTag>,
}

#[derive(Deserialize)]
struct OllamaTag {
    name: String,
}

fn fetch_ollama_tags(base_url: &str, timeout: Duration) -> Option<Vec<DiscoveredModel>> {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let response = agent.get(&url).call().ok()?;
    let parsed: OllamaTagsResponse = response.into_json().ok()?;
    if parsed.models.is_empty() {
        return None;
    }
    // Ollama's /api/tags doesn't report a context length, so it's left None
    // — a manually configured context_windows entry is the only way to set
    // one for an Ollama model.
    let mut models: Vec<DiscoveredModel> =
        parsed.models.into_iter().map(|t| DiscoveredModel { id: t.name, context_length: None }).collect();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Some(models)
}

#[derive(Deserialize)]
struct LmStudioModelsResponse {
    #[serde(default)]
    data: Vec<LmStudioModel>,
}

#[derive(Deserialize)]
struct LmStudioModel {
    id: String,
    #[serde(default)]
    max_context_length: Option<u64>,
}

fn fetch_lmstudio_native(base_url: &str, timeout: Duration) -> Option<Vec<DiscoveredModel>> {
    let url = format!("{}/api/v0/models", base_url.trim_end_matches('/'));
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let response = agent.get(&url).call().ok()?;
    let parsed: LmStudioModelsResponse = response.into_json().ok()?;
    if parsed.data.is_empty() {
        return None;
    }
    let mut models: Vec<DiscoveredModel> = parsed
        .data
        .into_iter()
        .map(|m| DiscoveredModel { id: m.id, context_length: m.max_context_length })
        .collect();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Some(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use tiny_http::{Response, Server};

    /// Starts a one-shot local HTTP server, returns (base_url, received_path, received_auth_header)
    /// via a channel once the single expected request has been handled.
    fn serve_once(body: &'static str) -> (String, mpsc::Receiver<(String, Option<String>)>) {
        let server = Server::http("127.0.0.1:0").unwrap();
        let addr = server.server_addr().to_string();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if let Ok(request) = server.recv() {
                let path = request.url().to_string();
                let auth = request
                    .headers()
                    .iter()
                    .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case("Authorization"))
                    .map(|h| h.value.as_str().to_string());
                let _ = tx.send((path, auth));
                let _ = request.respond(Response::from_string(body));
            }
        });
        (format!("http://{addr}"), rx)
    }

    /// Starts a local HTTP server that answers each request by matching its
    /// path against `routes` (checked in order): `(status, body)` on a
    /// match, 404 otherwise. Serves up to `routes.len()` requests, enough
    /// for a source that tries one endpoint then falls back to another.
    fn serve_routes(routes: Vec<(&'static str, u16, &'static str)>) -> String {
        let server = Server::http("127.0.0.1:0").unwrap();
        let addr = server.server_addr().to_string();
        thread::spawn(move || {
            for _ in 0..routes.len() {
                let Ok(request) = server.recv() else { break };
                let path = request.url().to_string();
                match routes.iter().find(|(route_path, ..)| *route_path == path) {
                    Some((_, status, body)) => {
                        let response = Response::from_string(*body).with_status_code(*status);
                        let _ = request.respond(response);
                    }
                    None => {
                        let response = Response::from_string("not found").with_status_code(404);
                        let _ = request.respond(response);
                    }
                }
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn returns_sorted_model_ids_on_success() {
        let (base_url, _rx) = serve_once(r#"{"data":[{"id":"b-model"},{"id":"a-model"}]}"#);
        let result = fetch_remote_models(&base_url, "lm-studio", Duration::from_secs(2));
        assert_eq!(
            result,
            Some(vec![
                DiscoveredModel { id: "a-model".to_string(), context_length: None },
                DiscoveredModel { id: "b-model".to_string(), context_length: None },
            ])
        );
    }

    #[test]
    fn parses_context_length_when_the_provider_reports_it() {
        let (base_url, _rx) =
            serve_once(r#"{"data":[{"id":"big","context_length":1310720},{"id":"small"}]}"#);
        let result = fetch_remote_models(&base_url, "key", Duration::from_secs(2)).unwrap();
        assert_eq!(result[0], DiscoveredModel { id: "big".to_string(), context_length: Some(1_310_720) });
        assert_eq!(result[1], DiscoveredModel { id: "small".to_string(), context_length: None });
    }

    #[test]
    fn requests_v1_models_with_authorization_header() {
        let (base_url, rx) = serve_once(r#"{"data":[{"id":"m"}]}"#);
        fetch_remote_models(&base_url, "my-key", Duration::from_secs(2));
        let (path, auth) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(path, "/v1/models");
        assert_eq!(auth, Some("Bearer my-key".to_string()));
    }

    #[test]
    fn strips_trailing_slash_from_base_url() {
        let (base_url, rx) = serve_once(r#"{"data":[{"id":"m"}]}"#);
        let base_url_with_slash = format!("{base_url}/");
        fetch_remote_models(&base_url_with_slash, "key", Duration::from_secs(2));
        let (path, _) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(path, "/v1/models");
    }

    #[test]
    fn returns_none_on_connection_refused() {
        // Port 1 is reserved and nothing listens there.
        let result = fetch_remote_models("http://127.0.0.1:1", "key", Duration::from_millis(500));
        assert_eq!(result, None);
    }

    #[test]
    fn returns_none_on_invalid_json() {
        let (base_url, _rx) = serve_once("not json");
        let result = fetch_remote_models(&base_url, "key", Duration::from_secs(2));
        assert_eq!(result, None);
    }

    #[test]
    fn returns_none_when_data_empty() {
        let (base_url, _rx) = serve_once(r#"{"data":[]}"#);
        let result = fetch_remote_models(&base_url, "key", Duration::from_secs(2));
        assert_eq!(result, None);
    }

    #[test]
    fn provider_kind_defaults_to_generic() {
        assert_eq!(ProviderKind::default(), ProviderKind::Generic);
    }

    #[test]
    fn provider_kind_serializes_as_lowercase() {
        assert_eq!(serde_yaml_ng::to_string(&ProviderKind::LmStudio).unwrap().trim(), "lmstudio");
        assert_eq!(serde_yaml_ng::to_string(&ProviderKind::Ollama).unwrap().trim(), "ollama");
        assert_eq!(serde_yaml_ng::from_str::<ProviderKind>("ollama").unwrap(), ProviderKind::Ollama);
    }

    #[test]
    fn ollama_kind_parses_api_tags_with_no_context_length() {
        let base_url = serve_routes(vec![(
            "/api/tags",
            200,
            r#"{"models":[{"name":"llama3.1:latest","model":"llama3.1:latest","size":123},{"name":"codellama:7b","model":"codellama:7b","size":456}]}"#,
        )]);
        let result = ProviderKind::Ollama.discover(&base_url, "unused", Duration::from_secs(2)).unwrap();
        assert_eq!(
            result,
            vec![
                DiscoveredModel { id: "codellama:7b".to_string(), context_length: None },
                DiscoveredModel { id: "llama3.1:latest".to_string(), context_length: None },
            ]
        );
    }

    #[test]
    fn ollama_kind_returns_none_when_no_models_pulled() {
        let base_url = serve_routes(vec![("/api/tags", 200, r#"{"models":[]}"#)]);
        assert_eq!(ProviderKind::Ollama.discover(&base_url, "unused", Duration::from_secs(2)), None);
    }

    #[test]
    fn lmstudio_kind_uses_the_native_api_and_its_context_length() {
        let base_url = serve_routes(vec![(
            "/api/v0/models",
            200,
            r#"{"data":[{"id":"qwen2.5-7b-instruct","max_context_length":32768},{"id":"phi-3-mini"}]}"#,
        )]);
        let result = ProviderKind::LmStudio.discover(&base_url, "lm-studio", Duration::from_secs(2)).unwrap();
        assert_eq!(
            result,
            vec![
                DiscoveredModel { id: "phi-3-mini".to_string(), context_length: None },
                DiscoveredModel { id: "qwen2.5-7b-instruct".to_string(), context_length: Some(32768) },
            ]
        );
    }

    #[test]
    fn lmstudio_kind_falls_back_to_generic_v1_models_when_native_api_is_unavailable() {
        // Older LM Studio versions (or a non-LM-Studio OpenAI-compatible
        // server on the same port) 404 on the native endpoint — the source
        // must still return results via the generic path, not just give up.
        let base_url = serve_routes(vec![
            ("/api/v0/models", 404, "not found"),
            ("/v1/models", 200, r#"{"data":[{"id":"local-model"}]}"#),
        ]);
        let result = ProviderKind::LmStudio.discover(&base_url, "lm-studio", Duration::from_secs(2)).unwrap();
        assert_eq!(result, vec![DiscoveredModel { id: "local-model".to_string(), context_length: None }]);
    }
}
