//! Per-character reveal timelines for live assistant Markdown.
//!
//! The timeline is intentionally separate from the stream pacer. The pacer
//! decides when text becomes visible; this module remembers when each visible
//! source character first appeared so a later batch cannot restart older
//! characters' animations.

use super::zed_markdown::{MarkdownStyle, MarkdownVeil};
use crate::markdown::normalize::EMPHASIS_GUARD;
use crate::theme_system::{Theme, easing::cubic_bezier, tokens::motion};
use gpui::Hsla;
use std::collections::HashMap;
use std::sync::Arc;

/// A source-indexed reveal clock for one Markdown row.
#[derive(Debug, Default)]
pub struct RevealTimeline {
    source: String,
    /// First-visible time by UTF-8 source byte index. Completed entries stay
    /// until the live row ends, so later batches never restart old characters.
    first_seen: HashMap<usize, f64>,
    /// The small active subset copied into the Markdown veil each frame.
    active: HashMap<usize, f64>,
    /// Distinguishes a historical first observation from a brand-new live row.
    initialized: bool,
    was_live: bool,
}

impl RevealTimeline {
    /// Record the current source. `live` controls whether unseen characters
    /// receive a timestamp; initial historical content is therefore silent.
    pub fn observe(&mut self, source: &str, now_ms: f64, live: bool) {
        let was_live = self.was_live;
        self.prune(now_ms);

        // Most stream updates append to the existing normalized source. Avoid
        // rebuilding a map over the entire response on every render frame.
        if self.initialized && source == self.source {
            self.finish_state(was_live, live);
            return;
        }
        if self.initialized && source.starts_with(&self.source) {
            let old_len = self.source.len();
            let suffix = source[old_len..].to_owned();
            if live || was_live {
                for (offset, _) in suffix.char_indices() {
                    let at = old_len + offset;
                    self.first_seen.insert(at, now_ms);
                    self.active.insert(at, now_ms);
                }
            }
            self.source.push_str(&suffix);
            self.finish_state(was_live, live);
            return;
        }

        let next = remap_times(
            &self.source,
            &self.first_seen,
            source,
            now_ms,
            live || was_live,
        );

        self.source.clear();
        self.source.push_str(source);
        self.first_seen = next;
        self.active = self
            .first_seen
            .iter()
            .filter(|(_, first)| now_ms - **first < f64::from(motion::DURATION_STREAMING_REVEAL))
            .map(|(&at, &first)| (at, first))
            .collect();
        self.initialized = true;
        self.finish_state(was_live, live);
    }

    /// Add the timeline veil to normal text. Returns whether another frame is
    /// needed. Reduced motion leaves the style untouched and schedules none.
    pub fn decorate(
        &mut self,
        style: &mut MarkdownStyle,
        now_ms: f64,
        theme: &Theme,
        reduce_motion: bool,
    ) -> bool {
        self.prune(now_ms);
        if reduce_motion || self.active.is_empty() {
            return false;
        }

        let seen = self.active.clone();
        let duration = f64::from(motion::DURATION_STREAMING_REVEAL);
        let ease = cubic_bezier(motion::EASE_STANDARD);
        style.decorations.veil = Some(MarkdownVeil {
            start_color: Hsla::from(theme.colors.buddy_primary),
            start_opacity: motion::STREAMING_REVEAL_START_OPACITY,
            progress: Arc::new(move |at| {
                seen.get(&at).map(|first| {
                    let t = ((now_ms - *first) / duration).clamp(0.0, 1.0) as f32;
                    ease(t)
                })
            }),
        });
        true
    }

    fn prune(&mut self, now_ms: f64) {
        let duration = f64::from(motion::DURATION_STREAMING_REVEAL);
        self.active
            .retain(|_, first| now_ms - *first < duration);
    }

    fn finish_state(&mut self, was_live: bool, live: bool) {
        if live {
            self.was_live = true;
        } else if was_live && self.active.is_empty() {
            self.first_seen.clear();
            self.was_live = false;
        }
    }

    #[cfg(test)]
    fn first_seen_at(&self, source_index: usize) -> Option<f64> {
        self.first_seen.get(&source_index).copied()
    }
}

/// Remap timestamps across a Markdown normalization change. Normalization can
/// insert zero-width emphasis guards and shift every following byte; matching
/// visible characters keeps those old timestamps instead of replaying a block.
fn remap_times(
    old_source: &str,
    old_seen: &HashMap<usize, f64>,
    source: &str,
    now_ms: f64,
    animate_new: bool,
) -> HashMap<usize, f64> {
    let old: Vec<(usize, char)> = old_source.char_indices().collect();
    let mut old_ix = 0;
    let mut next = HashMap::with_capacity(old_seen.len().saturating_add(4));
    let new: Vec<(usize, char)> = source.char_indices().collect();
    let mut new_ix = 0;
    while new_ix < new.len() {
        let (at, ch) = new[new_ix];
        new_ix += 1;
        if ch == EMPHASIS_GUARD {
            continue;
        }
        while old_ix < old.len() && old[old_ix].1 == EMPHASIS_GUARD {
            old_ix += 1;
        }
        if old.get(old_ix).is_some_and(|(_, old_ch)| *old_ch == ch) {
            let (old_at, _) = old[old_ix];
            if let Some(seen) = old_seen.get(&old_at) {
                next.insert(at, *seen);
            }
            old_ix += 1;
        } else {
            let old_match = old.get(old_ix..).and_then(|rest| {
                rest.iter()
                    .take(8)
                    .position(|(_, old_ch)| *old_ch == ch)
            });
            let old_current = old.get(old_ix).map(|(_, old_ch)| *old_ch);
            let new_match = old_current.and_then(|old_ch| {
                new[new_ix..]
                    .iter()
                    .take(8)
                    .position(|(_, new_ch)| *new_ch == old_ch)
            });
            if new_match.is_some() {
                // A new character was inserted before the old current one.
                if animate_new {
                    next.insert(at, now_ms);
                }
            } else if let Some(offset) = old_match {
                // Existing source characters were removed before this one.
                old_ix += offset;
                let (old_at, _) = old[old_ix];
                if let Some(seen) = old_seen.get(&old_at) {
                    next.insert(at, *seen);
                }
                old_ix += 1;
            } else {
                // Replacement: consume the old character so the following
                // unchanged suffix keeps its original timestamps.
                if animate_new {
                    next.insert(at, now_ms);
                }
                old_ix = old_ix.saturating_add(1);
            }
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_source_is_silent() {
        let mut timeline = RevealTimeline::default();
        timeline.observe("历史内容", 100.0, false);
        assert_eq!(timeline.first_seen_at(0), None);
    }

    #[test]
    fn appended_source_preserves_old_time_and_stamps_only_new_text() {
        let mut timeline = RevealTimeline::default();
        timeline.observe("旧", 100.0, true);
        timeline.observe("旧新", 140.0, true);
        assert_eq!(timeline.first_seen_at(0), Some(100.0));
        assert_eq!(timeline.first_seen_at("旧".len()), Some(140.0));
    }

    #[test]
    fn completed_old_text_does_not_restart_when_new_text_arrives() {
        let mut timeline = RevealTimeline::default();
        timeline.observe("旧", 100.0, true);
        timeline.observe("旧", 400.0, true);
        timeline.observe("旧新", 420.0, true);
        assert_eq!(timeline.first_seen_at(0), Some(100.0));
        assert_eq!(timeline.first_seen_at("旧".len()), Some(420.0));
    }

    #[test]
    fn done_frame_stamps_text_appended_before_live_turn_closes() {
        let mut timeline = RevealTimeline::default();
        timeline.observe("前", 100.0, true);
        timeline.observe("前后", 120.0, false);
        assert_eq!(timeline.first_seen_at("前".len()), Some(120.0));
    }

    #[test]
    fn emphasis_guard_insertion_preserves_visible_character_times() {
        let mut timeline = RevealTimeline::default();
        timeline.observe("加粗和", 100.0, true);
        let normalized = format!("{}加粗{}和", EMPHASIS_GUARD, EMPHASIS_GUARD);
        timeline.observe(&normalized, 140.0, true);
        assert_eq!(timeline.first_seen_at(EMPHASIS_GUARD.len_utf8()), Some(100.0));
    }

    #[test]
    fn source_replacement_stamps_changed_index() {
        let mut timeline = RevealTimeline::default();
        timeline.observe("abc", 100.0, true);
        timeline.observe("axc", 140.0, true);
        assert_eq!(timeline.first_seen_at(0), Some(100.0));
        assert_eq!(timeline.first_seen_at(1), Some(140.0));
        assert_eq!(timeline.first_seen_at(2), Some(100.0));
    }

    #[test]
    fn final_batch_survives_until_reveal_window_expires() {
        let mut timeline = RevealTimeline::default();
        timeline.observe("最后", 100.0, true);
        timeline.observe("最后", 110.0, false);
        assert!(timeline.first_seen_at(0).is_some());
        timeline.observe(
            "最后",
            100.0 + f64::from(motion::DURATION_STREAMING_REVEAL),
            false,
        );
        assert_eq!(timeline.first_seen_at(0), None);
    }

    #[test]
    fn settled_timeline_has_no_active_animation() {
        let mut timeline = RevealTimeline::default();
        timeline.observe("字", 100.0, true);
        timeline.observe("字", 100.0 + f64::from(motion::DURATION_STREAMING_REVEAL), true);
        assert!(timeline.active.is_empty());
    }
}
