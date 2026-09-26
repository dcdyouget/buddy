//! S02-09（T05）：真实 provider 契约测试 —— 需要 API Key 与网络，默认忽略。
//!
//! 手动运行：`cargo test -p buddy-engine --test real_provider -- --ignored --nocapture`
//!
//! **只读**默认数据目录（与 v1 相同）中的 `config.json`；直接调用 provider，
//! 不经过 `ChatEngine`，因此不会向用户的聊天记录写入任何消息。输出只含模型名与事件统计，不含 key。

use buddy_engine::models::{AppConfig, raw_model_id};
use buddy_engine::providers::{ProviderType, create_provider};
use buddy_engine::storage;
use buddy_engine::streaming::{StreamEvent, StreamEventEmitter};
use tokio::sync::watch;

fn user_config() -> AppConfig {
    let dir = storage::default_data_dir().expect("默认数据目录");
    storage::get_config(&dir).expect("读取配置")
}

/// 对某类 provider：取第一个「有 key 的 provider 下的模型」发一条极短请求
async fn contract(provider_type: &str) {
    let config = user_config();
    let Some((provider, model)) = config.models.iter().find_map(|m| {
        let p = config.providers.iter().find(|p| p.id == m.provider_id)?;
        (ProviderType::from_str(&p.provider_type) == ProviderType::from_str(provider_type)
            && !p.api_key.is_empty())
        .then_some((p, m))
    }) else {
        println!("[{provider_type}] 配置中没有可用的该类 provider，跳过");
        return;
    };

    let messages = vec![
        serde_json::from_value(serde_json::json!({
            "id": "contract-u1", "role": "user", "content": "请只回复两个字母：ok",
            "model_id": null, "created_at": 0
        }))
        .unwrap(),
    ];
    let (emitter, mut rx) = StreamEventEmitter::channel();
    let (_cancel_tx, cancel_rx) = watch::channel(false);
    let outcome = create_provider(&ProviderType::from_str(&provider.provider_type))
        .stream_chat(
            &provider.base_url,
            &provider.api_key,
            raw_model_id(model),
            "contract",
            &messages,
            &emitter,
            cancel_rx,
            provider.compat.as_ref(),
            &[],
        )
        .await
        .expect("真实请求应成功");
    drop(emitter);

    let mut events = 0;
    let (mut text, mut thinking) = (0, 0);
    while let Some(ev) = rx.recv().await {
        events += 1;
        match ev {
            StreamEvent::TextDelta { .. } => text += 1,
            StreamEvent::ThinkingDelta { .. } => thinking += 1,
            _ => {}
        }
    }
    println!(
        "[{provider_type}] model={} provider={} events={events} text_deltas={text} thinking_deltas={thinking} full_text_chars={} had_stream_error={}",
        model.display_name,
        provider.name,
        outcome.full_text.chars().count(),
        outcome.had_stream_error
    );
    assert!(!outcome.had_stream_error, "{:?}", outcome.terminal_error);
    assert!(text > 0, "应至少收到一个文本增量");
}

#[tokio::test]
#[ignore = "需要真实 API Key 与网络"]
async fn real_openai_compatible_contract() {
    contract("openai_compatible").await;
}

#[tokio::test]
#[ignore = "需要真实 API Key 与网络"]
async fn real_anthropic_contract() {
    contract("anthropic").await;
}
