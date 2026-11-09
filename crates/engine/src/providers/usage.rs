//! One machine-readable usage record per HTTP attempt, including cancelled streams.
//! Missing vendor counters remain null; estimated tokens never masquerade as billed usage.
use serde_json::{Value, json};
use std::time::Instant;

#[derive(Default, Debug)]
struct Tokens {
    input: Option<u64>,
    output: Option<u64>,
    read: Option<u64>,
    write: Option<u64>,
    anthropic_uncached: Option<u64>,
}

impl Tokens {
    fn openai(&mut self, usage: &Value) {
        if let Some(value) = usage.get("prompt_tokens").and_then(Value::as_u64) {
            self.input = Some(value);
        }
        if let Some(value) = usage.get("completion_tokens").and_then(Value::as_u64) {
            self.output = Some(value);
        }
        self.read = usage
            .get("prompt_cache_hit_tokens")
            .and_then(Value::as_u64)
            .or_else(|| usage["prompt_tokens_details"]["cached_tokens"].as_u64())
            .or(self.read);
        self.write = usage["prompt_tokens_details"]["cache_write_tokens"]
            .as_u64()
            .or(self.write);
    }

    fn anthropic(&mut self, usage: &Value) {
        self.anthropic_uncached = usage["input_tokens"].as_u64().or(self.anthropic_uncached);
        self.output = usage["output_tokens"].as_u64().or(self.output);
        self.read = usage["cache_read_input_tokens"].as_u64().or(self.read);
        self.write = usage["cache_creation_input_tokens"].as_u64().or(self.write);
        // Anthropic input_tokens excludes both cache reads and writes. Without those
        // counters a proxy's total is unknown, rather than an invented zero-cache total.
        self.input = self
            .anthropic_uncached
            .zip(self.read)
            .zip(self.write)
            .map(|((input, read), write)| input.saturating_add(read).saturating_add(write));
    }

    fn rate(&self) -> Option<f64> {
        self.read
            .zip(self.input)
            .and_then(|(read, input)| (input > 0).then(|| read as f64 * 100.0 / input as f64))
    }
}

pub(super) struct UsageLog<'a> {
    request_id: &'a str,
    question_id: &'a str,
    stage: &'static str,
    model: &'a str,
    tokens: Tokens,
    started: Instant,
    completed: bool,
}

impl<'a> UsageLog<'a> {
    pub fn new(request_id: &'a str, model: &'a str, options: super::ChatOptions<'a>) -> Self {
        Self {
            request_id,
            question_id: if options.question_id.is_empty() {
                request_id
            } else {
                options.question_id
            },
            stage: options.stage,
            model,
            tokens: Tokens::default(),
            started: Instant::now(),
            completed: false,
        }
    }
    pub fn openai(&mut self, usage: &Value) {
        self.tokens.openai(usage);
    }
    pub fn anthropic(&mut self, usage: &Value) {
        self.tokens.anthropic(usage);
    }
    pub fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for UsageLog<'_> {
    fn drop(&mut self) {
        log::info!(
            "[usage] {}",
            json!({
                "question_id": self.question_id, "request_id": self.request_id,
                "stage": self.stage, "model": self.model,
                "input_tokens": self.tokens.input, "output_tokens": self.tokens.output,
                "total_tokens": self.tokens.input.zip(self.tokens.output).map(|(a,b)| a.saturating_add(b)),
                "cache_read_tokens": self.tokens.read, "cache_write_tokens": self.tokens.write,
                "uncached_input_tokens": self.tokens.anthropic_uncached,
                "cache_hit_percent": self.tokens.rate(),
                "completed": self.completed, "elapsed_ms": self.started.elapsed().as_millis(),
            })
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn openai_distinguishes_missing_usage_and_explicit_zero() {
        let mut tokens = Tokens::default();
        tokens.openai(&json!({"prompt_tokens":100,"completion_tokens":8}));
        assert_eq!(tokens.read, None);
        assert_eq!(tokens.rate(), None);
        tokens.openai(&json!({"prompt_tokens_details":{"cached_tokens":0}}));
        assert_eq!(tokens.rate(), Some(0.0));
    }
    #[test]
    fn anthropic_cumulative_deltas_do_not_double_count() {
        let mut tokens = Tokens::default();
        tokens.anthropic(&json!({"input_tokens":100,"cache_read_input_tokens":800,"cache_creation_input_tokens":100,"output_tokens":1}));
        tokens.anthropic(&json!({"output_tokens":20}));
        tokens.anthropic(&json!({"output_tokens":30}));
        assert_eq!(tokens.input, Some(1000));
        assert_eq!(tokens.output, Some(30));
        assert_eq!(tokens.rate(), Some(80.0));
    }
}
