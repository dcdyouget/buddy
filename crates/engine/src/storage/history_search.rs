//! 按提问检索本机历史；逐个分块读取，回答摘录跨工具轮和分块拼接。

use super::{APPEND_LOCK, manifest_for_read_unlocked, read_chunk};
use crate::{
    models::{Message, MessageRole},
    streaming::ContentBlock,
};
use std::path::Path;

/// 一次提问及其原回答摘录。消息 ID 是定位依据，offset 只用于读取历史范围。
#[derive(Clone, Debug)]
pub struct HistoryMatch {
    message_id: String,
    offset: u64,
    question: String,
    created_at: u64,
    answer: String,
}

impl HistoryMatch {
    /// 提问的稳定消息 ID。
    pub fn message_id(&self) -> &str {
        &self.message_id
    }
    /// 提问在持久化历史中的序号。
    pub fn offset(&self) -> u64 {
        self.offset
    }
    /// 原始提问全文。
    pub fn question(&self) -> &str {
        &self.question
    }
    /// 提问时间（Unix 秒）。
    pub fn created_at(&self) -> u64 {
        self.created_at
    }
    /// 原回答的正文摘录，不包含思考和工具输出。
    pub fn answer(&self) -> &str {
        &self.answer
    }
}

const EXCERPT_CHARS: usize = 600;

struct Search {
    phrase: String,
    terms: Vec<String>,
    results: Vec<HistoryMatch>,
    current: Option<HistoryMatch>,
    excerpt_chars: usize,
}

impl Search {
    fn new(query: &str) -> Self {
        let phrase = query
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let terms = phrase.split_whitespace().map(str::to_owned).collect();
        Self {
            phrase,
            terms,
            results: Vec::new(),
            current: None,
            excerpt_chars: 0,
        }
    }

    fn push(&mut self, offset: u64, message: &Message) {
        match message.role {
            MessageRole::User => {
                if let Some(previous) = self.current.take() {
                    self.results.push(previous);
                }
                self.excerpt_chars = 0;
                let question = message.content.to_lowercase();
                if self.terms.iter().all(|term| question.contains(term)) {
                    self.current = Some(HistoryMatch {
                        message_id: message.id.clone(),
                        offset,
                        question: message.content.clone(),
                        created_at: message.created_at,
                        answer: String::new(),
                    });
                }
            }
            MessageRole::Assistant => {
                if self.current.is_none() || self.excerpt_chars >= EXCERPT_CHARS {
                    return;
                }
                if let Some(blocks) = &message.blocks {
                    for block in blocks {
                        if let ContentBlock::Text { content } = block {
                            self.append(content);
                        }
                    }
                } else {
                    for block in ContentBlock::parse_from_text(&message.content) {
                        if let ContentBlock::Text { content } = block {
                            self.append(&content);
                        }
                    }
                }
            }
            MessageRole::Tool => {}
        }
    }

    fn append(&mut self, content: &str) {
        let Some(current) = &mut self.current else {
            return;
        };
        let content = content.trim();
        if content.is_empty() || self.excerpt_chars >= EXCERPT_CHARS {
            return;
        }
        if !current.answer.is_empty() {
            current.answer.push('\n');
        }
        let excerpt: String = content
            .chars()
            .take(EXCERPT_CHARS - self.excerpt_chars)
            .collect();
        self.excerpt_chars += excerpt.chars().count();
        current.answer.push_str(&excerpt);
    }

    fn finish(mut self) -> Vec<HistoryMatch> {
        if let Some(last) = self.current.take() {
            self.results.push(last);
        }
        let phrase = &self.phrase;
        self.results.sort_by_cached_key(|result| {
            std::cmp::Reverse((
                result.question.to_lowercase().contains(phrase),
                result.created_at,
                result.offset,
            ))
        });
        self.results
    }
}

/// 搜索全部已保存的用户消息；空白分隔的关键词取交集，忽略大小写。
///
/// 分块文件使用原子替换。只在取得清单快照时持有写锁，扫描按快照计数截断，
/// 避免长时间阻塞正在生成的回答落盘。损坏的分块会返回错误，不冒充完整结果。
pub fn search_history(data_dir: &Path, query: &str) -> Result<Vec<HistoryMatch>, String> {
    let mut search = Search::new(query);
    if search.terms.is_empty() {
        return Ok(Vec::new());
    }
    let manifest = {
        let _guard = APPEND_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        manifest_for_read_unlocked(data_dir)?
    };
    let mut offset = 0;
    for meta in manifest.chunks {
        let chunk = read_chunk(&data_dir.to_path_buf(), &meta.file)?;
        if chunk.messages.len() < meta.count as usize {
            return Err("历史记录已变化，请重新查询".into());
        }
        for message in chunk.messages.iter().take(meta.count as usize) {
            search.push(offset, message);
            offset += 1;
        }
    }
    Ok(search.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: usize, role: MessageRole, content: &str) -> Message {
        serde_json::from_value(serde_json::json!({
            "id":id.to_string(), "role":role, "content":content, "created_at":id,
        }))
        .unwrap()
    }

    #[test]
    fn searches_unloaded_chunks_and_collects_only_answer_text() {
        let dir = tempfile::tempdir().unwrap();
        for id in 0..99 {
            super::super::append_message(dir.path(), &message(id, MessageRole::User, "普通消息"))
                .unwrap();
        }
        super::super::append_message(
            dir.path(),
            &message(99, MessageRole::User, "Rust 快捷键冲突"),
        )
        .unwrap();
        let mut answer = message(100, MessageRole::Assistant, "不应回退到这里");
        answer.blocks = Some(vec![
            ContentBlock::Thinking {
                content: "隐藏思考".into(),
                is_open: false,
            },
            ContentBlock::Text {
                content: "第一段回答".into(),
            },
        ]);
        for item in [
            answer,
            message(101, MessageRole::Tool, "隐藏工具输出"),
            message(
                102,
                MessageRole::Assistant,
                "<think>旧版隐藏思考</think>第二段回答",
            ),
            message(103, MessageRole::User, "新的问题"),
            message(104, MessageRole::Assistant, "不属于这个问题"),
        ] {
            super::super::append_message(dir.path(), &item).unwrap();
        }
        let results = search_history(dir.path(), "快捷键 RUST").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message_id(), "99");
        assert_eq!(results[0].offset(), 99);
        assert_eq!(results[0].answer(), "第一段回答\n第二段回答");
        assert!(search_history(dir.path(), "隐藏工具").unwrap().is_empty());
        assert!(search_history(dir.path(), " \n ").unwrap().is_empty());
    }

    #[test]
    fn phrase_precedes_recency_and_followups_without_answers_are_included() {
        let mut search = Search::new("rust 快捷键");
        search.push(0, &message(1, MessageRole::User, "Rust 快捷键怎么改"));
        search.push(1, &message(3, MessageRole::User, "快捷键如何用 Rust 实现"));
        search.push(2, &message(2, MessageRole::User, "补充一下 Rust 快捷键"));
        let results = search.finish();
        assert_eq!(
            results
                .iter()
                .map(HistoryMatch::message_id)
                .collect::<Vec<_>>(),
            ["2", "1", "3"]
        );
        assert!(results.iter().all(|result| result.answer().is_empty()));
    }
}
