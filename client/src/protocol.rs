//! Wire protocol types — mirrors proto crate for WASM context

use serde::{Deserialize, Serialize};
use crate::state::HistoryEntry;

// ── Client → Server ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum ClientCmd {
    Auth          { token: String },
    Subscribe     { topic: String },
    Unsubscribe   { topic: String },
    UnsubscribeAll,
    Echo          { text: String },
    Ping,
}

// ── Server → Client ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    AuthRequired  { msg: String },
    Authorized    { puk: String, session_id: String },
    Unauthorized  { msg: String },
    Welcome       { msg: String },
    Subscribed    { topic: String },
    Unsubscribed  { topic: String },
    UnsubscribedAll,
    Shit          (ShitPayload),
    ServiceList   { services: Vec<ServiceInfo> },
    Echo          { payload: String },
    Pong,
    Error         { msg: String },
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct ShitPayload {
    pub seq:         u64,
    pub topic:       String,
    pub payload:     String,
    pub priority:    String,
    pub tags:        Vec<String>,
    pub producer_id: String,
    pub service_id:  String,
    pub ts_ms:       u64,
}

impl ShitPayload {
    pub fn to_history_entry(&self) -> HistoryEntry {
        HistoryEntry {
            seq:         self.seq,
            topic:       self.topic.clone(),
            payload:     self.payload.clone(),
            priority:    self.priority.clone(),
            tags:        self.tags.clone(),
            producer_id: self.producer_id.clone(),
            service_id:  self.service_id.clone(),
            ts_ms:       self.ts_ms,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServiceInfo {
    pub assigned_id:     String,
    pub service_id:      String,
    pub topic:           String,
    pub version:         String,
    pub interval_ms:     u64,
    pub description:     String,
    pub connected_at_ms: u64,
    pub messages_sent:   u64,
}
