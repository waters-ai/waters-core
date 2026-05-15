use anyhow::Result;
use serde_json::Value;
use tracing::info;

pub enum LlmProvider {
    DeepSeek { api_key: String, model: String },
    Ollama { url: String, model: String },
}

pub struct LlmClient {
    provider: LlmProvider,
    http_client: reqwest::Client,
}

impl LlmClient {
    pub fn new_deepseek(api_key: &str, model: &str) -> Self {
        LlmClient {
            provider: LlmProvider::DeepSeek {
                api_key: api_key.to_string(),
                model: model.to_string(),
            },
            http_client: reqwest::Client::new(),
        }
    }

    pub fn new_ollama(url: &str, model: &str) -> Self {
        LlmClient {
            provider: LlmProvider::Ollama {
                url: url.to_string(),
                model: model.to_string(),
            },
            http_client: reqwest::Client::new(),
        }
    }

    pub async fn chat(&self, system: &str, prompt: &str) -> Result<String> {
        match &self.provider {
            LlmProvider::DeepSeek { api_key, model } => {
                let body = serde_json::json!({
                    "model": model,
                    "messages": [
                        {"role": "system", "content": system},
                        {"role": "user", "content": prompt}
                    ],
                    "stream": false
                });

                let resp = self.http_client
                    .post("https://api.deepseek.com/beta/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&body)
                    .send()
                    .await?;

                let result: Value = resp.json().await?;
                let content = result["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                info!("DeepSeek response: {} chars", content.len());
                Ok(content)
            }
            LlmProvider::Ollama { url, model } => {
                let body = serde_json::json!({
                    "model": model,
                    "system": system,
                    "prompt": prompt,
                    "stream": false
                });

                let resp = self.http_client
                    .post(format!("{}/api/generate", url))
                    .json(&body)
                    .send()
                    .await?;

                let result: Value = resp.json().await?;
                let content = result["response"].as_str().unwrap_or("").to_string();
                info!("Ollama response: {} chars", content.len());
                Ok(content)
            }
        }
    }
}
