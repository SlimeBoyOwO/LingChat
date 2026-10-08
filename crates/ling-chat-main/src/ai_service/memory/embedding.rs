//! 远程 OpenAI 兼容 embeddings 客户端。
//!
//! `POST {base_url}/embeddings`，body `{model, input}`，返回 `data[0].embedding`。
//! 拿到向量后做 L2 归一化，后续余弦相似度退化为点积。

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

use super::EMBED_TIMEOUT;

#[derive(Debug, Deserialize)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

/// 嵌入请求客户端（每次调用都是一次 HTTP 请求，无内部状态）。
pub struct EmbeddingClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl EmbeddingClient {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(EMBED_TIMEOUT)
                .build()
                .unwrap_or_default(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
        }
    }

    /// 当前嵌入模型名。
    pub fn model(&self) -> &str {
        &self.model
    }

    /// 配置是否就绪（缺 base_url 或 model 都视为未配置）。
    pub fn is_configured(&self) -> bool {
        !self.base_url.is_empty() && !self.model.is_empty()
    }

    /// 嵌入一段文本，返回 L2 归一化后的向量。
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        if !self.is_configured() {
            return Err(anyhow!(
                "嵌入模型未配置（需在设置中填写 embedding 接口地址与模型名，或使用内置本地模型）"
            ));
        }
        let url = format!("{}/embeddings", self.base_url);
        let mut request = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "model": self.model, "input": text }));
        if !self.api_key.is_empty() {
            request = request.bearer_auth(&self.api_key);
        }
        let response = request
            .send()
            .await
            .with_context(|| format!("嵌入请求发送失败: {url}"))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("嵌入接口返回 {status}: {}", truncate(&body, 300)));
        }
        let parsed: EmbeddingsResponse = response.json().await.context("嵌入接口响应解析失败")?;
        let vector = parsed
            .data
            .into_iter()
            .next()
            .context("嵌入接口返回空 data")?
            .embedding;
        Ok(l2_normalize(vector))
    }

    /// 嵌入多段文本（一次请求），返回与输入同序的向量列表。
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        if !self.is_configured() {
            return Err(anyhow!(
                "嵌入模型未配置（需在设置中填写 embedding 接口地址与模型名，或使用内置本地模型）"
            ));
        }
        let url = format!("{}/embeddings", self.base_url);
        let mut request = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "model": self.model, "input": texts }));
        if !self.api_key.is_empty() {
            request = request.bearer_auth(&self.api_key);
        }
        let response = request
            .send()
            .await
            .with_context(|| format!("嵌入请求发送失败: {url}"))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("嵌入接口返回 {status}: {}", truncate(&body, 300)));
        }
        let parsed: EmbeddingsResponse = response.json().await.context("嵌入接口响应解析失败")?;
        let mut vectors: Vec<Vec<f32>> = parsed
            .data
            .into_iter()
            .map(|d| l2_normalize(d.embedding))
            .collect();
        if vectors.len() != texts.len() {
            return Err(anyhow!(
                "嵌入接口返回条数 ({}) 与输入条数 ({}) 不一致",
                vectors.len(),
                texts.len()
            ));
        }
        vectors.truncate(texts.len());
        Ok(vectors)
    }
}

fn l2_normalize(mut vector: Vec<f32>) -> Vec<f32> {
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

fn truncate(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}
