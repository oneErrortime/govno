//! 💩 ГОВНО-ПРОТОКОЛ — shared types for all crates
//!
//! Topology:
//!   Workers ──ws /producer──► Orchestrator ──broadcast──► Consumers
//!
//! Every wire message is JSON with a `"type"` tag.

use serde::{Deserialize, Serialize};

// ── Топики ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ShitTopic {
    Liquid,
    Solid,
    Gas,
    Critical,
}

impl ShitTopic {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Liquid   => "жидкое",
            Self::Solid    => "твёрдое",
            Self::Gas      => "газообразное",
            Self::Critical => "критическое",
        }
    }
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Liquid   => "💧",
            Self::Solid    => "🧱",
            Self::Gas      => "💨",
            Self::Critical => "🚨",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Priority { Normal, Critical }

// ── Producer → Orchestrator ────────────────────────────────────────────────────
// First message MUST be Hello; then Emit* then optional Bye

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProducerMsg {
    /// Sent once after WS connect — registers the service
    Hello {
        service_id:  String,
        topic:       ShitTopic,
        version:     String,
        interval_ms: u64,
        description: String,
    },
    /// A unit of shit to broadcast
    Emit {
        payload:  String,
        priority: Priority,
        tags:     Vec<String>,
    },
    /// Graceful shutdown
    Bye,
}

// ── Orchestrator → Producer ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestratorMsg {
    Welcome { assigned_id: String },
    Reject  { reason: String },
    Ack     { seq: u64 },
}

// ── Orchestrator → Consumer ────────────────────────────────────────────────────
// The fully-annotated broadcast message

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShitMessage {
    pub seq:         u64,
    pub topic:       ShitTopic,
    pub payload:     String,
    pub priority:    Priority,
    pub tags:        Vec<String>,
    pub producer_id: String,
    pub service_id:  String,
    pub ts_ms:       u64,
}

// ── Consumer client commands ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum ClientCmd {
    Auth          { token: String },
    Subscribe     { topic: ShitTopic },
    Unsubscribe   { topic: ShitTopic },
    UnsubscribeAll,
    Echo          { text: String },
    Ping,
}

// ── Server → Consumer ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    AuthRequired  { msg: String },
    Authorized    { puk: String, session_id: String },
    Unauthorized  { msg: String },
    Welcome       { msg: String },
    Subscribed    { topic: String },
    Unsubscribed  { topic: String },
    UnsubscribedAll,
    Shit          (ShitMessage),
    ServiceList   { services: Vec<ServiceInfo> },
    Echo          { payload: String },
    Pong,
    Error         { msg: String },
}

// ── Service registry ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub assigned_id:    String,
    pub service_id:     String,
    pub topic:          ShitTopic,
    pub version:        String,
    pub interval_ms:    u64,
    pub description:    String,
    pub connected_at_ms: u64,
    pub messages_sent:  u64,
}
