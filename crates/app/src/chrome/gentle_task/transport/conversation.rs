//! Display projection stays outside both semantic control queues.
use super::Wake;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

pub(crate) const MAX_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_ROWS: usize = 4096;
pub(crate) const FRAME_BYTES: usize = 64 * 1024;
const CHUNK_BYTES: usize = 16 * 1024;
pub(crate) const ROW_BYTES: usize = std::mem::size_of::<Row>();

#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub(crate) id: u64,
    pub(crate) message: u64,
    pub(crate) content_index: usize,
    pub(crate) role: &'static str,
    pub(crate) text: Arc<str>,
}

#[derive(Default)]
pub(crate) struct Cache {
    pub(crate) rows: VecDeque<Row>,
    pub(crate) gap: bool,
    pub(crate) truncated: bool,
    cursor: u64,
    applied_revision: u64,
}

#[derive(Clone)]
pub(crate) struct Display {
    inner: Arc<Mutex<ConversationStore>>,
    wake: Wake,
}

#[derive(Default)]
struct ConversationStore {
    rows: VecDeque<(u64, Row)>,
    bytes: usize,
    next: u64,
    revision: u64,
    message: u64,
    assistant: Option<u64>,
    user_open: Option<u64>,
    blocks: BTreeMap<usize, usize>,
    echo: Option<String>,
    gap: bool,
    truncated: bool,
}

impl Display {
    pub(super) fn new(wake: Wake) -> Self {
        Self {
            inner: Arc::default(),
            wake,
        }
    }

    pub(super) fn user(&self, text: &str) {
        let mut store = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        store.message += 1;
        let message = store.message;
        store.append(message, "user", text);
        store.echo = (text.len() <= super::MAX_TEXT).then(|| text.to_owned());
        store.revision += 1;
        drop(store);
        (self.wake)();
    }

    /// Returns whether another bounded poll is needed; never waits on the pipe reader.
    pub(crate) fn poll(&self, cache: &mut Cache, remaining: &mut usize) -> bool {
        let Ok(store) = self.inner.try_lock() else {
            (self.wake)();
            return true;
        };
        if cache.applied_revision == store.revision {
            return false;
        }
        let ids: BTreeSet<_> = store.rows.iter().map(|(_, r)| r.id).collect();
        cache.rows.retain(|r| ids.contains(&r.id));
        cache.gap = store.gap;
        cache.truncated = store.truncated;
        let mut budget = (*remaining).min(FRAME_BYTES);
        let initial_budget = budget;
        for (_, row) in &store.rows {
            if row.id <= cache.cursor {
                continue;
            }
            if row.text.len() + ROW_BYTES > budget {
                break;
            }
            budget -= row.text.len() + ROW_BYTES;
            cache.cursor = row.id;
            cache.rows.push_back(row.clone());
        }
        cache
            .rows
            .make_contiguous()
            .sort_by_key(|r| (r.message, r.content_index, r.id));
        *remaining -= initial_budget - budget;
        let pending = store.rows.back().is_some_and(|(_, r)| r.id > cache.cursor);
        if !pending {
            cache.applied_revision = store.revision;
        }
        drop(store);
        if pending {
            (self.wake)();
        }
        pending
    }

    pub(super) fn observe(&self, v: &Value) {
        let mut store = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if !store.observe(v) {
            return;
        }
        store.revision += 1;
        drop(store);
        (self.wake)();
    }
}

impl ConversationStore {
    fn append(&mut self, message: u64, role: &'static str, text: &str) {
        self.append_block(message, role, text, 0);
    }

    fn append_block(&mut self, message: u64, role: &'static str, text: &str, content_index: usize) {
        let mut text = text;
        if text.len() > MAX_BYTES {
            let mut cut = text.len() - MAX_BYTES;
            while !text.is_char_boundary(cut) {
                cut += 1;
            }
            text = &text[cut..];
            self.truncated = true;
        }
        while !text.is_empty() {
            let first = text.chars().next().unwrap().len_utf8();
            let merge = self.rows.back().is_some_and(|(m, r)| {
                *m == message
                    && r.role == role
                    && r.content_index == content_index
                    && r.text.len() + first <= CHUNK_BYTES
            });
            let mut chunk = if merge {
                let (_, row) = self.rows.pop_back().unwrap();
                self.bytes -= row.text.len() + ROW_BYTES;
                row.text.to_string()
            } else {
                String::new()
            };
            let mut end = text.len().min(CHUNK_BYTES - chunk.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            chunk.push_str(&text[..end]);
            self.next += 1;
            self.bytes += chunk.len() + ROW_BYTES;
            self.rows.push_back((
                message,
                Row {
                    id: self.next,
                    message,
                    content_index,
                    role,
                    text: Arc::from(chunk),
                },
            ));
            text = &text[end..];
            while self.bytes > MAX_BYTES || self.rows.len() > MAX_ROWS {
                let (_, row) = self.rows.pop_front().unwrap();
                self.bytes -= row.text.len() + ROW_BYTES;
                self.gap = true;
            }
        }
    }

    fn replace(&mut self, message: u64, role: &'static str, text: &str) {
        self.rows.retain(|(m, row)| {
            if *m == message {
                self.bytes -= row.text.len() + ROW_BYTES;
                false
            } else {
                true
            }
        });
        self.append(message, role, text);
    }

    fn content(message: &Value) -> String {
        if let Some(text) = message["content"].as_str() {
            return text.to_owned();
        }
        message["content"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|block| {
                if block["type"] == "text" {
                    block["text"].as_str().unwrap_or("").to_owned()
                } else {
                    format!("[{} content]", block["type"].as_str().unwrap_or("non-text"))
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn observe(&mut self, v: &Value) -> bool {
        match v["type"].as_str().unwrap_or("") {
            "message_start" | "message_end" => {
                let role = v["message"]["role"].as_str().unwrap_or("");
                let end = v["type"] == "message_end";
                if role == "user" {
                    let text = Self::content(&v["message"]);
                    if self.echo.as_deref() == Some(&text) {
                        if end {
                            self.echo = None;
                        }
                        return false;
                    }
                    if end {
                        let id = self.user_open.take().unwrap_or_else(|| {
                            self.message += 1;
                            self.message
                        });
                        self.replace(id, "user", &text);
                    } else {
                        self.message += 1;
                        self.user_open = Some(self.message);
                        self.append(self.message, "user", &text);
                    }
                } else if role == "assistant" {
                    if !end {
                        self.message += 1;
                        self.assistant = Some(self.message);
                        self.blocks.clear();
                        self.append(self.message, "assistant", &Self::content(&v["message"]));
                    } else {
                        let id = self.assistant.take().unwrap_or_else(|| {
                            self.message += 1;
                            self.message
                        });
                        self.replace(id, "assistant", &Self::content(&v["message"]));
                        self.blocks.clear();
                    }
                } else {
                    return false;
                }
            }
            "message_update" => {
                let event = &v["assistantMessageEvent"];
                if event["type"] != "text_delta" {
                    if matches!(
                        event["type"].as_str(),
                        Some("thinking_start" | "toolcall_start")
                    ) {
                        let id = *self.assistant.get_or_insert_with(|| {
                            self.message += 1;
                            self.message
                        });
                        let kind = event["type"].as_str().unwrap().trim_end_matches("_start");
                        let index = event["contentIndex"].as_u64().unwrap_or(0) as usize;
                        self.append_block(id, "assistant", &format!("[{kind} content]"), index);
                        return true;
                    }
                    return false;
                }
                let Some(delta) = event["delta"].as_str() else {
                    return false;
                };
                let id = *self.assistant.get_or_insert_with(|| {
                    self.message += 1;
                    self.message
                });
                // Content indices address blocks, never durable messages. Bound reconstruction too.
                let index = event["contentIndex"].as_u64().unwrap_or(0) as usize;
                if !self.blocks.contains_key(&index) && self.blocks.len() >= MAX_ROWS {
                    self.truncated = true;
                    return true;
                }
                let total: usize = self.blocks.values().sum();
                let mut end = delta.len().min(MAX_BYTES.saturating_sub(total));
                while !delta.is_char_boundary(end) {
                    end -= 1;
                }
                if end < delta.len() {
                    self.truncated = true;
                }
                *self.blocks.entry(index).or_default() += end;
                // Append only new bytes; final content is authoritative and replaces these chunks.
                self.append_block(id, "assistant", &delta[..end], index);
            }
            "tool_execution_start" | "tool_execution_update" | "tool_execution_end" => {
                self.message += 1;
                let status = v["type"].as_str().unwrap();
                let name = v["toolName"].as_str().unwrap_or("tool");
                let id = v["toolCallId"].as_str().unwrap_or("unknown");
                let text = if status == "tool_execution_end" {
                    format!(
                        "{name} [{id}]: {}\n{}",
                        if v["isError"] == true {
                            "failed"
                        } else {
                            "ended"
                        },
                        Self::content(&v["result"])
                    )
                } else {
                    format!("{name} [{id}]: {status}")
                };
                self.append(self.message, "tool", &text);
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn display() -> Display {
        Display::new(Arc::new(|| {}))
    }
    fn drain(display: &Display) -> Cache {
        let mut cache = Cache::default();
        while display.poll(&mut cache, &mut FRAME_BYTES.clone()) {}
        cache
    }
    fn text(cache: &Cache) -> String {
        cache.rows.iter().map(|r| &*r.text).collect()
    }
    #[test]
    fn authoritative_final_replaces_partial_and_user_echo_is_not_duplicated() {
        let display = display();
        display.user("hello");
        display.observe(&json!({"type":"message_end","message":{"role":"user","content":[{"type":"text","text":"hello"}]}}));
        display.observe(&json!({"type":"message_start","message":{"role":"assistant"}}));
        display.observe(&json!({"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"partial"}}));
        let mut cache = drain(&display);
        assert_eq!(text(&cache), "hellopartial");
        display.observe(&json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"final"},{"type":"image"}]}}));
        display.poll(&mut cache, &mut FRAME_BYTES.clone());
        assert_eq!(text(&cache), "hellofinal\n[image content]");
    }
    #[test]
    fn byte_item_and_frame_bounds_apply_to_visible_unicode_projection() {
        let display = display();
        display.user(&"界".repeat(MAX_BYTES));
        let mut cache = Cache::default();
        let mut budget = FRAME_BYTES;
        assert!(display.poll(&mut cache, &mut budget));
        assert!(text(&cache).len() <= FRAME_BYTES);
        assert_eq!(
            budget + text(&cache).len() + cache.rows.len() * ROW_BYTES,
            FRAME_BYTES
        );
        while display.poll(&mut cache, &mut FRAME_BYTES.clone()) {}
        assert!(text(&cache).len() + cache.rows.len() * ROW_BYTES <= MAX_BYTES);
        assert!(cache.truncated);
        for _ in 0..MAX_ROWS + 10 {
            display.user("a");
        }
        while display.poll(&mut cache, &mut FRAME_BYTES.clone()) {}
        assert!(cache.rows.len() <= MAX_ROWS);
        assert!(text(&cache).len() + cache.rows.len() * ROW_BYTES <= MAX_BYTES);
        assert!(cache.gap);
        assert!(text(&cache).ends_with("aaaaaaaaaa"));
    }
    #[test]
    fn clone_retains_history_after_its_owner_drops_and_budget_is_shared() {
        let display = display();
        display.user(&"x".repeat(FRAME_BYTES * 2));
        let retained = display.clone();
        drop(display);
        let mut one = Cache::default();
        let mut two = Cache::default();
        let mut budget = FRAME_BYTES;
        retained.poll(&mut one, &mut budget);
        retained.poll(&mut two, &mut budget);
        assert!(text(&one).len() <= FRAME_BYTES);
        assert_eq!(one.rows.len(), 3);
        assert!(two.rows.is_empty());
    }
    #[test]
    fn interleaved_content_indices_keep_block_order() {
        let display = display();
        for (index, delta) in [(1, "B"), (0, "A"), (1, "C")] {
            display.observe(&json!({"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":index,"delta":delta}}));
        }
        assert_eq!(text(&drain(&display)), "ABC");
    }
    #[test]
    fn idle_poll_spends_nothing_and_empty_final_invalidates_cached_partial() {
        let display = display();
        display.observe(&json!({"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"partial"}}));
        let mut cache = drain(&display);
        let mut budget = FRAME_BYTES;
        assert!(!display.poll(&mut cache, &mut budget));
        assert_eq!(budget, FRAME_BYTES);
        display.observe(&json!({"type":"message_end","message":{"role":"assistant","content":[]}}));
        assert!(!display.poll(&mut cache, &mut budget));
        assert!(cache.rows.is_empty());
    }
}
