//! Real API smoke test. Reads the selected model config, uses a temporary chat,
//! and prints only responses/context/usage (never credentials or provider bodies).
//! cargo run -p buddy-engine --example context_usage
use buddy_engine::{
    chat::ChatEngine,
    storage,
    streaming::{ContentBlock, StreamEvent, StreamEventEmitter},
};
use serde_json::{Value, json};
use std::sync::Mutex;

struct UsageLogger(Mutex<Vec<Value>>);
static LOGGER: UsageLogger = UsageLogger(Mutex::new(Vec::new()));
impl log::Log for UsageLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }
    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let text = record.args().to_string();
        if let Some(usage) = text.strip_prefix("[usage] ") {
            if let Ok(value) = serde_json::from_str(usage) {
                self.0.lock().unwrap().push(value);
            }
            println!("{text}");
        } else if text.starts_with("[context]") {
            println!("{text}");
        }
    }
    fn flush(&self) {}
}

#[tokio::main]
async fn main() -> Result<(), String> {
    log::set_logger(&LOGGER).map_err(|e| e.to_string())?;
    log::set_max_level(log::LevelFilter::Info);
    let mut config = storage::get_config(&storage::default_data_dir()?)?;
    let model_id = config.selected_model_id.clone();
    if model_id.is_empty() {
        return Err("未选择模型".into());
    }
    config.allowed_paths.clear();
    let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
    storage::save_config(directory.path(), &config)?;
    let engine = ChatEngine::new(directory.path().to_path_buf());
    println!("model={model_id}; temporary isolated chat");
    for (id, text) in [
        ("probe-seed", "请用一句话解释 Rust 的所有权，不调用工具。"),
        (
            "probe-question",
            "它如何避免重复释放？请用两句话回答，不调用工具。",
        ),
    ] {
        println!("question_id={id} question={text}");
        let message = serde_json::from_value(json!({"id":id,"role":"user","content":text,"created_at":chrono::Utc::now().timestamp().max(0)})).map_err(|e| e.to_string())?;
        let (emitter, mut receiver) = StreamEventEmitter::channel();
        engine
            .send_message_from_history(emitter, message, model_id.clone())
            .await?;
        let mut completed = false;
        while let Some(event) = receiver.recv().await {
            match event {
                StreamEvent::Done { full_text, .. } => {
                    let answer = ContentBlock::parse_from_text(&full_text)
                        .into_iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { content } => Some(content),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    println!("answer={}", answer.trim());
                    completed = true;
                }
                StreamEvent::Error { message, .. } => return Err(message),
                _ => {}
            }
        }
        if !completed {
            return Err("主请求未完成".into());
        }
        engine.wait_for_context_tasks().await;
    }
    let state: Value = serde_json::from_slice(
        &std::fs::read(directory.path().join("context/state.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    for turn in state["turns"].as_array().ok_or("缺少问答索引")? {
        let summary = turn["summary"].as_str().ok_or("摘要未生成")?;
        println!(
            "summary question_id={} chars={} text={summary}",
            turn["message_id"],
            summary.chars().count()
        );
    }
    let records = LOGGER.0.lock().unwrap();
    let usage: Vec<_> = records
        .iter()
        .filter(|r| r["question_id"] == "probe-question")
        .collect();
    for key in [
        "input_tokens",
        "output_tokens",
        "cache_read_tokens",
        "cache_write_tokens",
    ] {
        let total = usage
            .iter()
            .try_fold(0u64, |total, r| r[key].as_u64().map(|v| total + v));
        println!(
            "question_total {key}={}",
            total.map_or("unknown".into(), |v| v.to_string())
        );
    }
    Ok(())
}
