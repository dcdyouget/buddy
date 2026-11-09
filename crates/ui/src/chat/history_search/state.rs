use buddy_engine::storage::HistoryMatch;
use std::time::{Duration, SystemTime};

pub(super) const RESULT_LIFETIME: Duration = Duration::from_secs(60 * 60);

/// 搜索词编辑属于 Composer；这里的 query 只表示已提交的查询。
#[derive(Default)]
pub(super) struct SearchState {
    pub query: String,
    pub results: Vec<HistoryMatch>,
    pub loading: bool,
    pub error: Option<String>,
    pub completed_at: Option<SystemTime>,
    generation: u64,
}

impl SearchState {
    pub fn begin(&mut self, query: &str) -> Option<u64> {
        if query.trim().is_empty() {
            return None;
        }
        self.generation += 1;
        self.query = query.trim().to_owned();
        self.results.clear();
        self.error = None;
        self.completed_at = None;
        self.loading = true;
        Some(self.generation)
    }

    pub fn complete(
        &mut self,
        generation: u64,
        result: Result<Vec<HistoryMatch>, String>,
        now: SystemTime,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        self.loading = false;
        match result {
            Ok(results) => {
                self.results = results;
                self.completed_at = Some(now);
            }
            Err(error) => self.error = Some(error),
        }
        true
    }

    pub fn expire(&mut self, now: SystemTime) -> bool {
        if self
            .completed_at
            .is_none_or(|at| now.duration_since(at).unwrap_or_default() < RESULT_LIFETIME)
        {
            return false;
        }
        self.results.clear();
        self.query.clear();
        self.error = None;
        self.completed_at = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiry_is_fixed_from_completion_not_submission_or_navigation() {
        let mut state = SearchState::default();
        let start = SystemTime::now();
        let generation = state.begin("关键词").unwrap();
        assert!(!state.expire(start + RESULT_LIFETIME));
        let completed = start + Duration::from_secs(5);
        state.complete(generation, Ok(vec![]), completed);
        assert!(!state.expire(completed + RESULT_LIFETIME - Duration::from_millis(1)));
        assert!(state.expire(completed + RESULT_LIFETIME));
        assert!(state.query.is_empty());
        assert!(state.error.is_none());
        assert!(!state.expire(completed + RESULT_LIFETIME));

        // 新查询完成才重新计时，旧查询的截止时间不能清掉新结果。
        let next = state.begin("新关键词").unwrap();
        let next_completed = completed + RESULT_LIFETIME;
        state.complete(next, Ok(vec![]), next_completed);
        assert!(!state.expire(completed + RESULT_LIFETIME + Duration::from_secs(1)));
        assert!(state.expire(next_completed + RESULT_LIFETIME));
    }

    #[test]
    fn empty_submission_keeps_results_and_old_requests_cannot_replace_new_ones() {
        let mut state = SearchState::default();
        let old = state.begin("第一次").unwrap();
        let new = state.begin("第二次").unwrap();
        assert!(state.begin(" \n ").is_none());
        assert!(!state.complete(old, Err("旧请求失败".into()), SystemTime::now()));
        assert!(state.loading);
        assert!(state.complete(new, Ok(vec![]), SystemTime::now()));
        assert_eq!(state.query, "第二次");
        assert!(state.error.is_none());
        assert!(state.begin("").is_none());
        assert!(state.completed_at.is_some());
    }
}
