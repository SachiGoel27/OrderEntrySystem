#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::State;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::connect_async;

const WS_URL: &str = "ws://127.0.0.1:8080/ws";
/// Must match `PRICE_SCALE` in `include/types.hpp` (fixed-point units per $1).
const PRICE_SCALE: f64 = 1_000_000.0;

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct BookLevel {
    pub price: i64,
    pub qty: i32,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct BookSnapshot {
    #[serde(rename = "type", default)]
    pub msg_type: Option<String>,
    #[serde(default)]
    pub best_bid: i64,
    pub best_ask: Option<i64>,
    #[serde(default)]
    pub best_bid_cached: Option<i64>,
    pub best_ask_cached: Option<i64>,
    #[serde(default)]
    pub bids: Vec<BookLevel>,
    #[serde(default)]
    pub asks: Vec<BookLevel>,
    pub connections: Option<u32>,
}

#[derive(Default, Clone, Serialize)]
pub struct UiState {
    pub book: Option<BookSnapshot>,
    pub ws_connected: bool,
    pub latency_ms: f64,
    pub last_order_id: Option<u64>,
    /// (t_sec, midpoint_price) — `t_sec` is seconds since the desktop bridge started
    pub midpoint_series: Vec<(f64, f64)>,
    /// (t_sec, last trade price) at fill time
    pub fill_markers: Vec<(f64, f64)>,
    pub toasts: Vec<String>,
    pub engine_connections: u32,
    pub best_bid_cached: Option<i64>,
    pub best_ask_cached: Option<i64>,
}

struct Inner {
    /// Wall clock for chart X axis (seconds since bridge created)
    t0: Instant,
    pending_book: Option<BookSnapshot>,
    committed_book: Option<BookSnapshot>,
    last_commit: Option<Instant>,
    midpoint_series: VecDeque<(f64, f64)>,
    fill_markers: VecDeque<(f64, f64)>,
    last_order_id: Option<u64>,
    ws_connected: bool,
    latency_ms: f64,
    ping_sent_at: Option<Instant>,
    pending_toasts: Vec<String>,
}

impl Inner {
    fn new() -> Self {
        Self {
            t0: Instant::now(),
            pending_book: None,
            committed_book: None,
            last_commit: None,
            midpoint_series: VecDeque::new(),
            fill_markers: VecDeque::new(),
            last_order_id: None,
            ws_connected: false,
            latency_ms: 0.0,
            ping_sent_at: None,
            pending_toasts: Vec::new(),
        }
    }

    fn elapsed_chart_secs(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    fn trim_time_series(&mut self) {
        const WINDOW_SECS: f64 = 240.0;
        let t_hi = self
            .midpoint_series
            .back()
            .map(|(t, _)| *t)
            .unwrap_or(0.0)
            .max(
                self.fill_markers
                    .back()
                    .map(|(t, _)| *t)
                    .unwrap_or(0.0),
            );
        let cutoff = t_hi - WINDOW_SECS;
        while let Some(&(t, _)) = self.midpoint_series.front() {
            if t < cutoff {
                self.midpoint_series.pop_front();
            } else {
                break;
            }
        }
        while let Some(&(t, _)) = self.fill_markers.front() {
            if t < cutoff {
                self.fill_markers.pop_front();
            } else {
                break;
            }
        }
    }

    fn price_to_f(p: i64) -> f64 {
        p as f64 / PRICE_SCALE
    }

    fn midpoint_from_book(b: &BookSnapshot) -> Option<f64> {
        let ask = b.best_ask?;
        if b.best_bid <= 0 || ask <= 0 || ask == i64::MAX {
            return None;
        }
        Some((Self::price_to_f(b.best_bid) + Self::price_to_f(ask)) / 2.0)
    }

    fn commit_if_ready(&mut self, now: Instant) {
        const FRAME: Duration = Duration::from_millis(16);
        let due = self.pending_book.is_some()
            && self
                .last_commit
                .map(|t| now.duration_since(t) >= FRAME)
                .unwrap_or(true);
        if !due {
            return;
        }
        if let Some(b) = self.pending_book.take() {
            self.committed_book = Some(b.clone());
            self.last_commit = Some(now);
            if let Some(mid) = Self::midpoint_from_book(&b) {
                let t = self.elapsed_chart_secs();
                self.midpoint_series.push_back((t, mid));
                self.trim_time_series();
            }
        }
    }

    fn ingest_json_line(&mut self, text: &str) {
        let v: serde_json::Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => {
                self.pending_toasts
                    .push("Non-JSON message from engine".into());
                return;
            }
        };
        let typ = v["type"].as_str().unwrap_or("");
        match typ {
            "book" => match serde_json::from_value::<BookSnapshot>(v) {
                Ok(b) => {
                    self.pending_book = Some(b);
                }
                Err(e) => self.pending_toasts.push(format!("book parse: {e}")),
            },
            "fill" => {
                let price = v["price"].as_i64().unwrap_or(0);
                let px = Self::price_to_f(price);
                let t = self.elapsed_chart_secs();
                self.fill_markers.push_back((t, px));
                self.trim_time_series();
            }
            "ack" => {
                if let Some(id) = v["order_id"].as_u64() {
                    self.last_order_id = Some(id);
                }
            }
            "cancel_result" => {
                let ok = v["success"].as_bool().unwrap_or(false);
                if !ok {
                    self.pending_toasts.push("Cancel rejected by engine".into());
                }
            }
            "error" => {
                let msg = v["message"].as_str().unwrap_or("engine error");
                self.pending_toasts.push(msg.to_string());
            }
            "client_pong" => {
                if let Some(t0) = self.ping_sent_at.take() {
                    self.latency_ms = t0.elapsed().as_secs_f64() * 1000.0;
                }
            }
            _ => {}
        }
    }

    fn snapshot_ui(&mut self, now: Instant) -> UiState {
        self.commit_if_ready(now);
        let book = self.committed_book.clone();
        let engine_connections = book.as_ref().and_then(|b| b.connections).unwrap_or(0);
        let best_bid_cached = book.as_ref().and_then(|b| b.best_bid_cached).or_else(|| book.as_ref().map(|b| b.best_bid));
        let best_ask_cached = book
            .as_ref()
            .and_then(|b| b.best_ask_cached)
            .or_else(|| book.as_ref().and_then(|b| b.best_ask));
        let toasts = std::mem::take(&mut self.pending_toasts);
        UiState {
            book,
            ws_connected: self.ws_connected,
            latency_ms: self.latency_ms,
            last_order_id: self.last_order_id,
            midpoint_series: self.midpoint_series.iter().copied().collect(),
            fill_markers: self.fill_markers.iter().copied().collect(),
            toasts,
            engine_connections,
            best_bid_cached,
            best_ask_cached,
        }
    }
}

struct Bridge {
    out_tx: Mutex<Option<mpsc::UnboundedSender<String>>>,
    inner: Mutex<Inner>,
}

impl Bridge {
    fn new() -> Self {
        Self {
            out_tx: Mutex::new(None),
            inner: Mutex::new(Inner::new()),
        }
    }

    fn set_out_tx(&self, tx: Option<mpsc::UnboundedSender<String>>) {
        *self.out_tx.lock().expect("bridge lock") = tx;
    }

    fn send_line(&self, line: String) -> Result<(), String> {
        let g = self.out_tx.lock().map_err(|_| "bridge lock poisoned")?;
        let tx = g.as_ref().ok_or_else(|| "engine socket not connected".to_string())?;
        tx.send(line).map_err(|_| "engine socket closed".to_string())
    }
}

async fn engine_loop(bridge: Arc<Bridge>) {
    loop {
        match connect_async(WS_URL).await {
            Ok((ws, _)) => {
                {
                    let mut inner = bridge.inner.lock().expect("inner");
                    inner.ws_connected = true;
                }
                let (mut write, mut read) = ws.split();
                let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                bridge.set_out_tx(Some(tx));

                loop {
                    tokio::select! {
                        biased;
                        out = rx.recv() => {
                            match out {
                                Some(msg) => {
                                    if write.send(Message::Text(msg)).await.is_err() {
                                        break;
                                    }
                                }
                                None => break,
                            }
                        }
                        inc = read.next() => {
                            match inc {
                                Some(Ok(Message::Text(t))) => {
                                    let mut inner = bridge.inner.lock().expect("inner");
                                    inner.ingest_json_line(&t);
                                }
                                Some(Ok(Message::Ping(p))) => {
                                    let _ = write.send(Message::Pong(p)).await;
                                }
                                Some(Ok(Message::Close(_))) | None => break,
                                Some(Err(_)) => break,
                                _ => {}
                            }
                        }
                    }
                }
                bridge.set_out_tx(None);
                let mut inner = bridge.inner.lock().expect("inner");
                inner.ws_connected = false;
                inner
                    .pending_toasts
                    .push("WebSocket disconnected — reconnecting…".into());
            }
            Err(_) => {
                let mut inner = bridge.inner.lock().expect("inner");
                inner.ws_connected = false;
                inner.pending_toasts.push(format!(
                    "Cannot connect to engine at {WS_URL} (is oes_engine running?)"
                ));
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[tauri::command]
fn get_ui_state(bridge: State<'_, Arc<Bridge>>) -> UiState {
    let now = Instant::now();
    let mut inner = bridge.inner.lock().expect("inner");
    inner.snapshot_ui(now)
}

#[tauri::command]
fn submit_order(
    bridge: State<'_, Arc<Bridge>>,
    side: String,
    price: String,
    qty: String,
    order_type: String,
    tif: String,
) -> Result<(), String> {
    let price_i: i64 = price.trim().parse().map_err(|_| "invalid price")?;
    let qty_i: i32 = qty.trim().parse().map_err(|_| "invalid qty")?;
    let body = serde_json::json!({
        "side": side,
        "price": price_i,
        "qty": qty_i,
        "type": order_type,
        "tif": tif,
    });
    bridge.send_line(body.to_string())
}

#[tauri::command]
fn quick_cancel_last(bridge: State<'_, Arc<Bridge>>) -> Result<(), String> {
    let id = {
        let inner = bridge.inner.lock().expect("inner");
        inner
            .last_order_id
            .ok_or_else(|| "no last order id yet".to_string())?
    };
    let body = serde_json::json!({ "action": "cancel", "id": id as i64 });
    bridge.send_line(body.to_string())
}

#[tauri::command]
fn ping_engine(bridge: State<'_, Arc<Bridge>>) -> Result<bool, String> {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    {
        let mut inner = bridge.inner.lock().expect("inner");
        inner.ping_sent_at = Some(Instant::now());
    }
    let body = serde_json::json!({ "type": "client_ping", "id": id });
    bridge.send_line(body.to_string())?;
    Ok(true)
}

fn main() {
    let bridge = Arc::new(Bridge::new());
    let b2 = bridge.clone();
    tauri::async_runtime::spawn(async move {
        engine_loop(b2).await;
    });

    tauri::Builder::default()
        .manage(bridge)
        .invoke_handler(tauri::generate_handler![
            get_ui_state,
            submit_order,
            quick_cancel_last,
            ping_engine
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
