//! Global application state

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use serde::Serialize;
use crate::protocol::ServiceInfo;

// ── Connection state machine ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting   { url: String },
    Authenticating,
    Connected    { session_id: String },
    Reconnecting { attempt: u32, delay_secs: u64 },
}

impl ConnectionState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Disconnected          => "отключено",
            Self::Connecting { .. }     => "подключаемся...",
            Self::Authenticating        => "авторизация...",
            Self::Connected { .. }      => "подключено",
            Self::Reconnecting { .. }   => "переподключение...",
        }
    }
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Disconnected          => "🔴",
            Self::Connecting { .. }     => "⏳",
            Self::Authenticating        => "🔐",
            Self::Connected { .. }      => "🟢",
            Self::Reconnecting { .. }   => "🔁",
        }
    }
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Connected { .. }                          => "status-ok",
            Self::Authenticating | Self::Connecting { .. }
            | Self::Reconnecting { .. }                     => "status-connecting",
            Self::Disconnected                              => "status-error",
        }
    }
}

// ── Rate tracker (30-bucket rolling window) ────────────────────────────────────

pub const RATE_BUCKETS: usize = 30;

#[derive(Default)]
pub struct RateTracker {
    pub buckets: VecDeque<u32>,
    pub current: u32,
    pub total:   u64,
}

impl RateTracker {
    pub fn add(&mut self, n: u32) {
        self.current += n;
        self.total   += n as u64;
    }
    pub fn commit_bucket(&mut self) {
        self.buckets.push_back(self.current);
        if self.buckets.len() > RATE_BUCKETS {
            self.buckets.pop_front();
        }
        self.current = 0;
    }
    pub fn max_in_window(&self) -> u32 {
        self.buckets.iter().copied().max().unwrap_or(1).max(1)
    }
    pub fn rate_per_sec(&self) -> f64 {
        if self.buckets.is_empty() { return 0.0; }
        let sum: u32 = self.buckets.iter().sum();
        sum as f64 / self.buckets.len() as f64
    }
}

// ── History entry ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct HistoryEntry {
    pub seq:         u64,
    pub topic:       String,
    pub payload:     String,
    pub priority:    String,
    pub tags:        Vec<String>,
    pub producer_id: String,
    pub service_id:  String,
    pub ts_ms:       u64,
}

// ── AppState ──────────────────────────────────────────────────────────────────

pub struct AppState {
    pub connection_state:   ConnectionState,
    pub ws_url:             String,
    pub token:              String,
    pub pending_auth:       bool,
    pub reconnect_attempt:  u32,
    pub subscriptions:      HashSet<String>,
    pub rates:              HashMap<String, RateTracker>,
    pub message_history:    VecDeque<HistoryEntry>,
    pub services:           Vec<ServiceInfo>,
    pub session_start_ms:   f64,
    pub total_received:     u64,
    pub sound_enabled:      bool,
}

impl AppState {
    fn new() -> Self {
        let now_ms = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);

        let mut rates = HashMap::new();
        for t in &["liquid","solid","gas","critical","__total__"] {
            rates.insert(t.to_string(), RateTracker::default());
        }

        Self {
            connection_state:  ConnectionState::Disconnected,
            ws_url:            String::new(),
            token:             String::new(),
            pending_auth:      false,
            reconnect_attempt: 0,
            subscriptions:     HashSet::new(),
            rates,
            message_history:   VecDeque::new(),
            services:          Vec::new(),
            session_start_ms:  now_ms,
            total_received:    0,
            sound_enabled:     false,   // off by default — requires user opt-in
        }
    }

    pub fn record_message(&mut self, entry: &HistoryEntry) {
        if let Some(t) = self.rates.get_mut(&entry.topic) { t.add(1); }
        if let Some(t) = self.rates.get_mut("__total__")  { t.add(1); }
        self.total_received += 1;
        self.message_history.push_back(entry.clone());
        if self.message_history.len() > 1000 {
            self.message_history.pop_front();
        }
    }

    pub fn tick(&mut self) {
        for t in self.rates.values_mut() {
            t.commit_bucket();
        }
    }

    pub fn session_uptime_secs(&self) -> u64 {
        let now_ms = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        ((now_ms - self.session_start_ms) / 1000.0) as u64
    }
}

// ── Thread-local storage ──────────────────────────────────────────────────────

thread_local! {
    static APP: RefCell<Option<AppState>> = const { RefCell::new(None) };
}

pub fn get<R>(f: impl FnOnce(&mut AppState) -> R) -> R {
    APP.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if borrow.is_none() {
            *borrow = Some(AppState::new());
        }
        f(borrow.as_mut().unwrap())
    })
}
