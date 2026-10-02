//! Collect public reasoning text from both streaming deltas and completed snapshots.
//! Encrypted reasoning is conversation state, never display text.

use std::collections::BTreeMap;

use serde_json::Value;

// Output item, summary/content kind, and part index identify a stable stream segment.
type PartKey = (u64, bool, u64);

#[derive(Default)]
pub(super) struct ReasoningBuffer {
    parts: BTreeMap<PartKey, String>,
    text: String,
    last_part: Option<PartKey>,
}

impl ReasoningBuffer {
    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn consume(&mut self, event: &Value) -> Vec<String> {
        let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
        let output = event
            .get("output_index")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let summary = kind.starts_with("response.reasoning_summary_");
        let part = event
            .get(if summary {
                "summary_index"
            } else {
                "content_index"
            })
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let mut chunks = Vec::new();
        match kind {
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                self.append(
                    (output, summary, part),
                    event.get("delta"),
                    false,
                    &mut chunks,
                );
            },
            "response.reasoning_summary_text.done" | "response.reasoning_text.done" => {
                self.append(
                    (output, summary, part),
                    event.get("text"),
                    true,
                    &mut chunks,
                );
            },
            "response.reasoning_summary_part.added" | "response.reasoning_summary_part.done" => {
                self.append(
                    (output, true, part),
                    event.pointer("/part/text"),
                    true,
                    &mut chunks,
                );
            },
            "response.output_item.done" => {
                if let Some(item) = event.get("item") {
                    self.item(output, item, &mut chunks);
                }
            },
            "response.completed" | "response.incomplete" => {
                if let Some(items) = event.pointer("/response/output").and_then(Value::as_array) {
                    for (index, item) in items.iter().enumerate() {
                        self.item(index as u64, item, &mut chunks);
                    }
                }
            },
            _ => {},
        }
        chunks
    }

    fn item(&mut self, output: u64, item: &Value, chunks: &mut Vec<String>) {
        if item.get("type").and_then(Value::as_str) != Some("reasoning") {
            return;
        }
        for (field, summary, part_type) in [
            ("summary", true, "summary_text"),
            ("content", false, "reasoning_text"),
        ] {
            if let Some(parts) = item.get(field).and_then(Value::as_array) {
                for (index, part) in parts.iter().enumerate() {
                    if part.get("type").and_then(Value::as_str) == Some(part_type) {
                        self.append(
                            (output, summary, index as u64),
                            part.get("text"),
                            true,
                            chunks,
                        );
                    }
                }
            }
        }
    }

    fn append(
        &mut self,
        key: PartKey,
        value: Option<&Value>,
        snapshot: bool,
        chunks: &mut Vec<String>,
    ) {
        let Some(incoming) = value
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
        else {
            return;
        };
        let previous = self.parts.entry(key).or_default();
        let addition = if snapshot {
            // Completed events repeat the streamed prefix. Emit only a missing suffix.
            // A conflicting snapshot cannot rewrite text already delivered to consumers.
            let Some(suffix) = incoming.strip_prefix(previous.as_str()) else {
                return;
            };
            suffix
        } else {
            incoming
        };
        if addition.is_empty() {
            return;
        }
        previous.push_str(addition);
        let mut chunk = String::new();
        if self.last_part.is_some_and(|last| last != key) && !self.text.ends_with('\n') {
            chunk.push_str("\n\n");
        }
        chunk.push_str(addition);
        self.text.push_str(&chunk);
        self.last_part = Some(key);
        chunks.push(chunk);
    }
}
