use super::prompts;
use crate::{
    models::{AppConfig, Message, ModelInfo, ProviderConfig, raw_model_id},
    providers::{self, ChatOptions, ProviderType},
    streaming::StreamEventEmitter,
};
use serde_json::{Value, json};
use tokio::sync::watch;

#[derive(Clone)]
pub(super) struct Client {
    pub model: ModelInfo,
    provider: ProviderConfig,
}

impl Client {
    pub fn new(config: &AppConfig, model_id: &str) -> Result<Self, String> {
        let model = config
            .models
            .iter()
            .find(|m| m.id == model_id)
            .ok_or("未找到指定的模型")?
            .clone();
        let provider = config
            .providers
            .iter()
            .find(|p| p.id == model.provider_id)
            .ok_or("未找到对应的 Provider")?
            .clone();
        Ok(Self { model, provider })
    }

    pub async fn request(
        &self,
        prompt: &str,
        payload: Value,
        stage: &'static str,
        question_id: &str,
        attempt: usize,
        cancel: watch::Receiver<bool>,
    ) -> Result<Value, String> {
        let content = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
        let budget = ((self.model.context_window as f64 * 0.5) as u32).min(12_000);
        if crate::chat::estimate_tokens(prompt)
            .saturating_add(crate::chat::estimate_tokens(&content))
            .saturating_add(32)
            > budget
        {
            return Err("后台请求超过独立上下文预算".into());
        }
        let message: Message = serde_json::from_value(
            json!({"id":"background", "role":"user", "content":content, "created_at":0}),
        )
        .map_err(|e| e.to_string())?;
        let request_id = format!("{question_id}#{stage}-{attempt}");
        let options = ChatOptions {
            system_prompt: prompt,
            runtime_context: false,
            context: None,
            max_output_tokens: Some(if stage == "relevance" { 512 } else { 1024 }),
            stage,
            question_id,
        };
        let (emitter, receiver) = StreamEventEmitter::channel();
        drop(receiver); // Never buffer background deltas or expose them to the UI.
        let provider =
            providers::create_provider(&ProviderType::from_str(&self.provider.provider_type));
        let messages = [message];
        let request = provider.stream_chat_with_options(
            &self.provider.base_url,
            &self.provider.api_key,
            raw_model_id(&self.model),
            &request_id,
            &messages,
            &emitter,
            cancel,
            self.provider.compat.as_ref(),
            &[],
            options,
        );
        let timeout = if stage == "relevance" { 15 } else { 60 };
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(timeout), request)
            .await
            .map_err(|_| format!("{stage} 请求超时"))?
            .map_err(|e| e.to_string())?;
        if outcome.had_stream_error {
            return Err("后台模型请求未完成".into());
        }
        parse_json(&outcome.full_text)
    }

    pub async fn summarize(
        &self,
        payload: Value,
        stage: &'static str,
        question_id: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<String, String> {
        let mut payload = payload;
        for attempt in 1..=2 {
            let output = match self
                .request(
                    prompts::SUMMARY,
                    payload.clone(),
                    stage,
                    question_id,
                    attempt,
                    cancel.clone(),
                )
                .await
            {
                Ok(output) => output,
                Err(error) if error == "后台模型返回的JSON无效" => Value::Null,
                Err(error) => return Err(error),
            };
            if let Some(summary) = output.get("summary").and_then(Value::as_str) {
                let summary = summary.trim();
                if !summary.is_empty() && summary.chars().count() <= 200 {
                    return Ok(summary.to_owned());
                }
            }
            // One stateless repair attempt, preserving the original factual material.
            payload["length_requirement"] = json!(
                "上次输出无效。summary必须为非空字符串，最多200个字符，请缩短。只输出规定JSON。"
            );
        }
        Err("摘要格式无效或超过200字".into())
    }
}

fn parse_json(text: &str) -> Result<Value, String> {
    // Some OpenAI-compatible reasoning models put a think block in content,
    // even for JSON answers. Only unwrap a leading block, never JSON values.
    let mut text = text.trim();
    while let Some(thinking) = text.strip_prefix("<think>") {
        text = thinking
            .split_once("</think>")
            .ok_or("后台模型返回的JSON无效")?
            .1
            .trim();
    }
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .unwrap_or(text)
        .trim();
    serde_json::from_str(text).map_err(|_| "后台模型返回的JSON无效".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reasoning_wrappers_are_removed_but_json_values_are_untouched() {
        assert_eq!(
            parse_json("<think>reasoning {invalid}</think>\n```json\n{\"summary\":\"ok\"}\n```")
                .unwrap(),
            json!({"summary":"ok"})
        );
        assert_eq!(
            parse_json(r#"{"summary":"解释 <think> 标签"}"#).unwrap(),
            json!({"summary":"解释 <think> 标签"})
        );
        assert!(parse_json("<think>unfinished").is_err());
        assert!(parse_json("{\"summary\":\"unfinished").is_err());
    }
}
