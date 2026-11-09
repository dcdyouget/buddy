use crate::{
    models::{Message, MessageRole},
    storage,
};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};

static STORE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Turn {
    pub key: String,
    pub message_id: String,
    pub context_id: String,
    pub start: u64,
    pub end: u64,
    pub created_at: u64,
    pub summary: Option<String>,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Segment {
    pub id: String,
    pub summary: Option<String>,
    pub covered_through: Option<u64>,
    pub revision: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct State {
    pub version: u32,
    pub observed: u64,
    pub active: Option<String>,
    pub turns: Vec<Turn>,
    pub segments: Vec<Segment>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            observed: 0,
            active: None,
            turns: vec![],
            segments: vec![],
        }
    }
}

fn read(dir: &Path) -> Result<State, String> {
    let path = dir.join("context/state.json");
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let state: State =
                serde_json::from_str(&text).map_err(|e| format!("上下文索引损坏: {e}"))?;
            if state.version != 1 {
                return Err("不支持的上下文索引版本".into());
            }
            Ok(state)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
        Err(e) => Err(format!("读取上下文索引失败: {e}")),
    }
}

pub(super) fn update<T>(
    dir: &Path,
    edit: impl FnOnce(&mut State) -> Result<T, String>,
) -> Result<T, String> {
    let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut state = read(dir)?;
    let result = edit(&mut state)?;
    let directory = dir.join("context");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let text = serde_json::to_string(&state).map_err(|e| e.to_string())?;
    storage::atomic_file::write_file_atomic(&directory, "state.json", &text, "上下文索引")?;
    Ok(result)
}

pub(super) fn load(dir: &Path) -> Result<State, String> {
    let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    read(dir)
}

pub(super) fn is_question(message: &Message) -> bool {
    message.role == MessageRole::User && !message.id.starts_with("visual-")
}

impl State {
    pub fn ensure_segment(&mut self, id: &str) {
        if !self.segments.iter().any(|s| s.id == id) {
            self.segments.push(Segment {
                id: id.into(),
                summary: None,
                covered_through: None,
                revision: 0,
            });
        }
    }

    /// Import only a bounded tail for old installations; do not summarize the whole archive.
    /// A leading partial tool round is ignored until its original user message is present.
    pub fn sync(&mut self, dir: &Path) -> Result<(), String> {
        let count = storage::message_count(dir)?;
        if count < self.observed {
            return Err("聊天历史与上下文索引不一致".into());
        }
        if count == self.observed {
            return Ok(());
        }
        let start = self
            .observed
            .max(count.saturating_sub(super::MAX_MESSAGES as u64));
        let messages = storage::load_messages(dir, start, count - start)?;
        let context = self
            .active
            .clone()
            .unwrap_or_else(|| format!("legacy-{start}"));
        self.ensure_segment(&context);
        let starts: Vec<_> = messages
            .iter()
            .enumerate()
            .filter(|(_, m)| is_question(m))
            .map(|(i, _)| i)
            .collect();
        for (index, &first) in starts.iter().enumerate() {
            let offset = start + first as u64;
            if self.turns.iter().any(|t| t.start == offset) {
                continue;
            }
            self.turns.push(Turn {
                key: format!("turn-{offset}"),
                message_id: messages[first].id.clone(),
                context_id: context.clone(),
                start: offset,
                end: start + starts.get(index + 1).copied().unwrap_or(messages.len()) as u64,
                created_at: messages[first].created_at,
                summary: None,
                status: "imported".into(),
            });
        }
        self.active = Some(context);
        self.observed = count;
        Ok(())
    }
}
