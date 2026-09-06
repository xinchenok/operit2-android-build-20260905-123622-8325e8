#![allow(non_snake_case)]

use operit_host_api::HostManager::defaultHttpHost;
use operit_host_api::HttpRequestData;
use operit_model::SttConfig::{SttConfig, SttRecognitionResult};
use serde_json::Value;

use crate::stt::SpeechToTextService::SpeechToTextService;

/// Sends containerized audio directly to Deepgram's pre-recorded /v1/listen API.
pub struct DeepgramSpeechToTextProvider;

impl DeepgramSpeechToTextProvider {
    pub fn new() -> Self { Self }
}

impl SpeechToTextService for DeepgramSpeechToTextProvider {
    fn transcribe(
        &self,
        config: &SttConfig,
        audioBytes: &[u8],
        _fileName: &str,
        contentType: &str,
        language: Option<&str>,
    ) -> Result<SttRecognitionResult, String> {
        let request = buildDeepgramRequest(config, audioBytes, contentType, language)?;
        let response = defaultHttpHost().executeHttpRequest(request)
            .map_err(|error| format!("Deepgram STT request failed: {error}"))?;
        if !(200..300).contains(&response.statusCode) {
            let body = String::from_utf8_lossy(&response.body);
            return Err(format!("Deepgram STT HTTP {}: {}", response.statusCode,
                body.chars().take(4096).collect::<String>()));
        }
        decodeDeepgramTranscript(&response.body)
    }
}

/// Retains endpoint options while binding model and language to this recording.
fn buildDeepgramRequest(
    config: &SttConfig,
    audioBytes: &[u8],
    contentType: &str,
    language: Option<&str>,
) -> Result<HttpRequestData, String> {
    if audioBytes.is_empty() { return Err("Deepgram STT audio payload is empty".to_string()); }
    if contentType.trim().is_empty() { return Err("Deepgram STT audio content type is empty".to_string()); }
    if config.model.trim().is_empty() { return Err("Deepgram STT model is empty".to_string()); }
    let mut url = url::Url::parse(config.endpoint.trim())
        .map_err(|error| format!("invalid Deepgram STT endpoint: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Deepgram STT endpoint must use HTTP or HTTPS".to_string());
    }
    let language = language.map(str::trim).filter(|value| !value.is_empty()).map(|value| {
        let value = value.to_ascii_lowercase();
        if value.starts_with("zh") { "zh".to_string() }
        else if value.starts_with("en") { "en".to_string() }
        else { value }
    });
    let automatic = language.as_deref().map(|value|
        matches!(value, "auto" | "auto-detect" | "auto_detect")).unwrap_or(false);
    let mut options = url.query_pairs().map(|(name, value)| (name.into_owned(), value.into_owned()))
        .filter(|(name, _)| name != "model" && !(language.is_some() && matches!(name.as_str(), "language" | "detect_language")))
        .collect::<Vec<_>>();
    options.push(("model".to_string(), config.model.trim().to_string()));
    for option in ["smart_format", "punctuate"] {
        if !options.iter().any(|(name, _)| name == option) {
            options.push((option.to_string(), "true".to_string()));
        }
    }
    if automatic {
        options.push(("detect_language".to_string(), "true".to_string()));
    } else if let Some(language) = language {
        options.push(("language".to_string(), language));
    }
    url.set_query(None);
    url.query_pairs_mut().extend_pairs(options);
    let mut headers = config.headers.iter()
        .filter(|header| !header.name.eq_ignore_ascii_case("Content-Type")
            && !header.name.eq_ignore_ascii_case("Content-Length"))
        .map(|header| (header.name.clone(), header.value.replace("{apiKey}", config.apiKey.trim())))
        .collect::<Vec<_>>();
    if !headers.iter().any(|(name, _)| name.eq_ignore_ascii_case("Authorization")) {
        if config.apiKey.trim().is_empty() { return Err("Deepgram STT API key is empty".to_string()); }
        headers.push(("Authorization".to_string(), format!("Token {}", config.apiKey.trim())));
    }
    headers.push(("Content-Type".to_string(), contentType.trim().to_string()));
    Ok(HttpRequestData {
        url: url.into(), method: "POST".to_string(), headers,
        body: audioBytes.to_vec(), formFields: Vec::new(), fileParts: Vec::new(),
        connectTimeoutSeconds: 30, readTimeoutSeconds: 180, followRedirects: true,
        ignoreSsl: false, proxyHost: String::new(), proxyPort: 0,
    })
}

/// Silence is an empty transcript; malformed responses are errors, never chat text.
fn decodeDeepgramTranscript(body: &[u8]) -> Result<SttRecognitionResult, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|error| format!("Deepgram STT response is not valid JSON: {error}"))?;
    let text = value.pointer("/results/channels/0/alternatives/0/transcript")
        .and_then(Value::as_str)
        .ok_or_else(|| "Deepgram STT response is missing its transcript".to_string())?;
    Ok(SttRecognitionResult { text: text.to_string() })
}
