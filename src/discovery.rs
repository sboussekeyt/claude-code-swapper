use serde::Deserialize;
use std::time::Duration;

/// One entry from a provider's `/v1/models` listing. `context_length` is not
/// part of the strict OpenAI schema, but OpenRouter's OpenAI-compatible
/// endpoint includes it anyway; providers that don't report it (LM Studio,
/// Ollama, ...) simply leave it `None`, and the caller falls back to a
/// manually configured `context_windows` entry.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DiscoveredModel {
    pub id: String,
    #[serde(default)]
    pub context_length: Option<u64>,
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
}
