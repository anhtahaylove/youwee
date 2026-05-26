use super::*;
use reqwest::Client;

pub async fn generate_with_gemini(
    api_key: &str,
    model: &str,
    transcript: &str,
    style: &SummaryStyle,
    language: &str,
    title: Option<&str>,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let prompt = build_prompt(transcript, style, language, title);
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );

    let is_thinking_model =
        model.contains("flash-preview") || model.contains("2.5") || model.contains("3-");

    let body = if is_thinking_model {
        serde_json::json!({
            "contents": [{ "parts": [{ "text": prompt }] }]
        })
    } else {
        serde_json::json!({
            "contents": [{ "parts": [{ "text": prompt }] }],
            "generationConfig": {
                "temperature": 0.7,
                "maxOutputTokens": 2048
            }
        })
    };

    #[cfg(debug_assertions)]
    {
        println!("[GEMINI] URL: {}", url);
        println!(
            "[GEMINI] Model: {}, Is thinking model: {}",
            model, is_thinking_model
        );
        println!(
            "[GEMINI] Request body: {}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
    }

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-goog-api-key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(e.to_string()))?;

    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();

    #[cfg(debug_assertions)]
    {
        println!("[GEMINI] Response status: {}", status);
        println!(
            "[GEMINI] Response body: {}",
            &response_text[..response_text.len().min(1000)]
        );
    }

    if !status.is_success() {
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            let error_msg = error_json
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or(&response_text);
            return Err(AIError::ApiError(format!(
                "Gemini API error: {}",
                error_msg
            )));
        }
        return Err(AIError::ApiError(format!(
            "Status {}: {}",
            status, response_text
        )));
    }

    let json: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| AIError::ParseError(format!("Failed to parse response: {}", e)))?;

    if let Some(error) = json.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(AIError::ApiError(format!("Gemini error: {}", msg)));
    }

    if let Some(feedback) = json.get("promptFeedback") {
        if let Some(block_reason) = feedback.get("blockReason") {
            return Err(AIError::ApiError(format!(
                "Content blocked: {:?}",
                block_reason
            )));
        }
    }

    let summary = json
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| {
            AIError::ParseError(format!(
                "Could not extract text from response. Response: {}",
                &response_text[..response_text.len().min(500)]
            ))
        })?;

    Ok(SummaryResult {
        summary: summary.trim().to_string(),
        provider: "Gemini".to_string(),
        model: model.to_string(),
    })
}

pub async fn generate_with_openai(
    api_key: &str,
    model: &str,
    transcript: &str,
    style: &SummaryStyle,
    language: &str,
    title: Option<&str>,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let prompt = build_prompt(transcript, style, language, title);

    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.7,
        "max_tokens": 1024,
    });

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(AIError::ApiError(format!("Status {}: {}", status, text)));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AIError::ParseError(e.to_string()))?;

    let summary = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError("No content in response".to_string()))?;

    Ok(SummaryResult {
        summary: summary.trim().to_string(),
        provider: "OpenAI".to_string(),
        model: model.to_string(),
    })
}

pub async fn generate_with_ollama(
    ollama_url: &str,
    model: &str,
    transcript: &str,
    style: &SummaryStyle,
    language: &str,
    title: Option<&str>,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let prompt = build_prompt(transcript, style, language, title);
    let url = format!("{}/api/generate", ollama_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "options": { "temperature": 0.7 }
    });

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(format!(
            "Failed to connect to Ollama at {}: {}",
            ollama_url, e
        )))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(AIError::ApiError(format!("Status {}: {}", status, text)));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AIError::ParseError(e.to_string()))?;

    let summary = json
        .get("response")
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError("No response in Ollama output".to_string()))?;

    Ok(SummaryResult {
        summary: summary.trim().to_string(),
        provider: "Ollama".to_string(),
        model: model.to_string(),
    })
}

pub async fn generate_with_deepseek(
    api_key: &str,
    model: &str,
    transcript: &str,
    style: &SummaryStyle,
    language: &str,
    title: Option<&str>,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let prompt = build_prompt(transcript, style, language, title);

    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.7,
        "max_tokens": 2048,
    });

    let response = client
        .post("https://api.deepseek.com/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(AIError::ApiError(format!(
            "DeepSeek API error ({}): {}",
            status, text
        )));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AIError::ParseError(e.to_string()))?;

    let summary = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError("No content in response".to_string()))?;

    Ok(SummaryResult {
        summary: summary.trim().to_string(),
        provider: "DeepSeek".to_string(),
        model: model.to_string(),
    })
}

pub async fn generate_with_qwen(
    api_key: &str,
    model: &str,
    transcript: &str,
    style: &SummaryStyle,
    language: &str,
    title: Option<&str>,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let prompt = build_prompt(transcript, style, language, title);

    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.7,
        "max_tokens": 2048,
    });

    let response = client
        .post("https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(AIError::ApiError(format!(
            "Qwen API error ({}): {}",
            status, text
        )));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AIError::ParseError(e.to_string()))?;

    let summary = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError("No content in response".to_string()))?;

    Ok(SummaryResult {
        summary: summary.trim().to_string(),
        provider: "Qwen".to_string(),
        model: model.to_string(),
    })
}

pub async fn generate_with_proxy(
    proxy_url: &str,
    api_key: &str,
    model: &str,
    transcript: &str,
    style: &SummaryStyle,
    language: &str,
    title: Option<&str>,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let prompt = build_prompt(transcript, style, language, title);

    let base_url = proxy_url.trim_end_matches('/');
    let url =
        if base_url.ends_with("/chat/completions") || base_url.ends_with("/v1/chat/completions") {
            base_url.to_string()
        } else if base_url.ends_with("/v1") {
            format!("{}/chat/completions", base_url)
        } else {
            format!("{}/v1/chat/completions", base_url)
        };

    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.7,
        "max_tokens": 1024,
    });

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(format!(
            "Failed to connect to proxy at {}: {}",
            proxy_url, e
        )))?;

    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            let error_msg = error_json
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or(&response_text);
            return Err(AIError::ApiError(format!("Proxy API error: {}", error_msg)));
        }
        return Err(AIError::ApiError(format!(
            "Status {}: {}",
            status, response_text
        )));
    }

    let json: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| AIError::ParseError(format!("Failed to parse response: {}", e)))?;

    let summary = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError(format!(
            "No content in response. Response: {}",
            &response_text[..response_text.len().min(500)]
        )))?;

    Ok(SummaryResult {
        summary: summary.trim().to_string(),
        provider: "Proxy".to_string(),
        model: model.to_string(),
    })
}

pub async fn generate_with_lmstudio(
    lmstudio_url: &str,
    model: &str,
    transcript: &str,
    style: &SummaryStyle,
    language: &str,
    title: Option<&str>,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let prompt = build_prompt(transcript, style, language, title);

    let base_url = lmstudio_url.trim_end_matches('/');
    let url =
        if base_url.ends_with("/chat/completions") || base_url.ends_with("/v1/chat/completions") {
            base_url.to_string()
        } else if base_url.ends_with("/v1") {
            format!("{}/chat/completions", base_url)
        } else {
            format!("{}/v1/chat/completions", base_url)
        };

    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.7,
        "max_tokens": 1024,
    });

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(format!(
            "Failed to connect to LM Studio at {}: {}",
            lmstudio_url, e
        )))?;

    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            let error_msg = error_json
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or(&response_text);
            return Err(AIError::ApiError(format!(
                "LM Studio API error: {}",
                error_msg
            )));
        }
        return Err(AIError::ApiError(format!(
            "Status {}: {}",
            status, response_text
        )));
    }

    let json: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| AIError::ParseError(format!("Failed to parse response: {}", e)))?;

    let summary = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError(format!(
            "No content in response. Response: {}",
            &response_text[..response_text.len().min(500)]
        )))?;

    Ok(SummaryResult {
        summary: summary.trim().to_string(),
        provider: "LM Studio".to_string(),
        model: model.to_string(),
    })
}

async fn generate_raw_with_gemini(
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );

    let body = serde_json::json!({
        "contents": [{ "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "temperature": 0.3,
            "maxOutputTokens": 2048
        }
    });

    #[cfg(debug_assertions)]
    {
        println!("[GEMINI RAW] URL: {}", url);
        println!("[GEMINI RAW] Prompt: {}", &prompt[..prompt.len().min(500)]);
    }

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-goog-api-key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(e.to_string()))?;

    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();

    #[cfg(debug_assertions)]
    {
        println!("[GEMINI RAW] Response status: {}", status);
        println!(
            "[GEMINI RAW] Response: {}",
            &response_text[..response_text.len().min(1000)]
        );
    }

    if !status.is_success() {
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            let error_msg = error_json
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            return Err(AIError::ApiError(format!(
                "Gemini API error: {}",
                error_msg
            )));
        }
        return Err(AIError::ApiError(format!("Gemini API error: {}", status)));
    }

    let json: serde_json::Value =
        serde_json::from_str(&response_text).map_err(|e| AIError::ParseError(e.to_string()))?;

    let text = json
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError("No text in response".to_string()))?;

    Ok(SummaryResult {
        summary: text.to_string(),
        model: model.to_string(),
        provider: "Gemini".to_string(),
    })
}

async fn generate_raw_with_openai(
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.3,
        "max_tokens": 2048
    });

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(e.to_string()))?;

    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AIError::ApiError(format!(
            "OpenAI API error: {}",
            response_text
        )));
    }

    let json: serde_json::Value =
        serde_json::from_str(&response_text).map_err(|e| AIError::ParseError(e.to_string()))?;

    let text = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError("No text in response".to_string()))?;

    Ok(SummaryResult {
        summary: text.to_string(),
        model: model.to_string(),
        provider: "OpenAI".to_string(),
    })
}

async fn generate_raw_with_ollama(
    base_url: &str,
    model: &str,
    prompt: &str,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let url = format!("{}/api/generate", base_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "options": { "temperature": 0.3 }
    });

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(e.to_string()))?;

    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AIError::ApiError(format!("Ollama error: {}", response_text)));
    }

    let json: serde_json::Value =
        serde_json::from_str(&response_text).map_err(|e| AIError::ParseError(e.to_string()))?;

    let text = json
        .get("response")
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError("No response in Ollama output".to_string()))?;

    Ok(SummaryResult {
        summary: text.to_string(),
        model: model.to_string(),
        provider: "Ollama".to_string(),
    })
}

async fn generate_raw_with_lmstudio(
    lmstudio_url: &str,
    model: &str,
    prompt: &str,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let base_url = lmstudio_url.trim_end_matches('/');
    let url =
        if base_url.ends_with("/chat/completions") || base_url.ends_with("/v1/chat/completions") {
            base_url.to_string()
        } else if base_url.ends_with("/v1") {
            format!("{}/chat/completions", base_url)
        } else {
            format!("{}/v1/chat/completions", base_url)
        };

    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.3,
        "max_tokens": 2048
    });

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(format!(
            "Failed to connect to LM Studio at {}: {}",
            lmstudio_url, e
        )))?;

    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AIError::ApiError(format!(
            "LM Studio API error: {}",
            response_text
        )));
    }

    let json: serde_json::Value =
        serde_json::from_str(&response_text).map_err(|e| AIError::ParseError(e.to_string()))?;

    let text = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError("No text in response".to_string()))?;

    Ok(SummaryResult {
        summary: text.to_string(),
        model: model.to_string(),
        provider: "LM Studio".to_string(),
    })
}

async fn generate_raw_with_deepseek(
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.3,
        "max_tokens": 2048
    });

    let response = client
        .post("https://api.deepseek.com/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(e.to_string()))?;

    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AIError::ApiError(format!(
            "DeepSeek API error: {}",
            response_text
        )));
    }

    let json: serde_json::Value =
        serde_json::from_str(&response_text).map_err(|e| AIError::ParseError(e.to_string()))?;

    let text = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError("No text in response".to_string()))?;

    Ok(SummaryResult {
        summary: text.to_string(),
        model: model.to_string(),
        provider: "DeepSeek".to_string(),
    })
}

async fn generate_raw_with_qwen(
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.3,
        "max_tokens": 2048
    });

    let response = client
        .post("https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(e.to_string()))?;

    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AIError::ApiError(format!(
            "Qwen API error: {}",
            response_text
        )));
    }

    let json: serde_json::Value =
        serde_json::from_str(&response_text).map_err(|e| AIError::ParseError(e.to_string()))?;

    let text = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError("No text in response".to_string()))?;

    Ok(SummaryResult {
        summary: text.to_string(),
        model: model.to_string(),
        provider: "Qwen".to_string(),
    })
}

async fn generate_raw_with_proxy(
    proxy_url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<SummaryResult, AIError> {
    let client = Client::new();
    let url = format!("{}/v1/chat/completions", proxy_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.3,
        "max_tokens": 2048
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AIError::NetworkError(e.to_string()))?;

    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AIError::ApiError(format!(
            "Proxy API error: {}",
            response_text
        )));
    }

    let json: serde_json::Value =
        serde_json::from_str(&response_text).map_err(|e| AIError::ParseError(e.to_string()))?;

    let text = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| AIError::ParseError("No text in response".to_string()))?;

    Ok(SummaryResult {
        summary: text.to_string(),
        model: model.to_string(),
        provider: "Proxy".to_string(),
    })
}

pub(super) async fn generate_raw_for_provider(
    config: &AIConfig,
    prompt: &str,
) -> Result<SummaryResult, AIError> {
    match config.provider {
        AIProvider::Gemini => {
            let api_key = config.api_key.as_ref().ok_or(AIError::NoApiKey)?;
            generate_raw_with_gemini(api_key, &config.model, prompt).await
        }
        AIProvider::OpenAI => {
            let api_key = config.api_key.as_ref().ok_or(AIError::NoApiKey)?;
            generate_raw_with_openai(api_key, &config.model, prompt).await
        }
        AIProvider::DeepSeek => {
            let api_key = config.api_key.as_ref().ok_or(AIError::NoApiKey)?;
            generate_raw_with_deepseek(api_key, &config.model, prompt).await
        }
        AIProvider::Qwen => {
            let api_key = config.api_key.as_ref().ok_or(AIError::NoApiKey)?;
            generate_raw_with_qwen(api_key, &config.model, prompt).await
        }
        AIProvider::Ollama => {
            let ollama_url = config
                .ollama_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            generate_raw_with_ollama(ollama_url, &config.model, prompt).await
        }
        AIProvider::LmStudio => {
            let lmstudio_url = config
                .lmstudio_url
                .as_deref()
                .unwrap_or("http://localhost:1234");
            generate_raw_with_lmstudio(lmstudio_url, &config.model, prompt).await
        }
        AIProvider::Proxy => {
            let api_key = config.api_key.as_ref().ok_or(AIError::NoApiKey)?;
            let proxy_url = config
                .proxy_url
                .as_deref()
                .unwrap_or("https://api.openai.com");
            generate_raw_with_proxy(proxy_url, api_key, &config.model, prompt).await
        }
    }
}
