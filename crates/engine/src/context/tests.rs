use super::*;
use crate::models::MessageRole;

fn message(id: usize, role: MessageRole) -> Message {
    serde_json::from_value(
        json!({"id":format!("m{id}"),"role":role,"content":format!("内容{id}"),"created_at":id}),
    )
    .unwrap()
}

#[test]
fn elapsed_time_only_changes_search_batch_not_topic() {
    assert_eq!(search_limits(100, 400), &[10, 20, 30]);
    assert_eq!(search_limits(100, 401), &[5, 10, 20, 30]);
    assert_eq!(search_limits(100, 99), &[10, 20, 30]);
}

#[test]
fn summary_material_excludes_reasoning_but_preserves_user_literal_tags() {
    let mut user = message(0, MessageRole::User);
    user.content = "解释 <think> 标签".into();
    let mut answer = message(1, MessageRole::Assistant);
    answer.content = "<think>internal reasoning</think>标签说明".into();
    let data = material(&[user, answer], 600);
    assert_eq!(data[0]["content"], "解释 <think> 标签");
    assert_eq!(data[1]["content"], "标签说明");
}

#[test]
fn judge_cannot_invent_context_ids_or_expand_past_limit() {
    let ids = HashSet::from(["existing".into()]);
    assert!(
        Decision::parse(
            json!({"related":true,"context_id":"invented","need_more":false}),
            &ids,
            true
        )
        .is_err()
    );
    assert!(
        Decision::parse(
            json!({"related":null,"context_id":null,"need_more":true}),
            &ids,
            false
        )
        .is_err()
    );
    assert!(
        Decision::parse(
            json!({"related":false,"context_id":null,"need_more":false}),
            &ids,
            true
        )
        .is_ok()
    );
    assert!(
        Decision::parse(
            json!({"related":true,"context_id":"existing","need_more":false}),
            &ids,
            true
        )
        .is_ok()
    );
}

#[test]
fn migration_imports_complete_rounds_in_bounded_tail_and_keeps_raw_history() {
    let dir = tempfile::tempdir().unwrap();
    let messages: Vec<_> = (0..61)
        .map(|i| {
            message(
                i,
                if i % 2 == 0 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
            )
        })
        .collect();
    storage::append_messages(dir.path(), &messages).unwrap();
    let state = store::update(dir.path(), |s| {
        s.sync(dir.path())?;
        Ok(s.clone())
    })
    .unwrap();
    assert_eq!(state.observed, 61);
    assert_eq!(state.turns.first().unwrap().start, 32); // offset31 is a partial round
    assert_eq!(state.turns.last().unwrap().end, 61);
    assert_eq!(storage::message_count(dir.path()).unwrap(), 61);
    store::update(dir.path(), |s| s.sync(dir.path())).unwrap();
    assert_eq!(
        store::load(dir.path()).unwrap().turns.len(),
        state.turns.len()
    );
}

#[test]
fn corrupt_sidecar_is_not_silently_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("context")).unwrap();
    let path = dir.path().join("context/state.json");
    std::fs::write(&path, "broken").unwrap();
    assert!(store::update(dir.path(), |_| Ok(())).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "broken");
}
