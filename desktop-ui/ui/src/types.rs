use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BookLevel {
    pub price: i64,
    pub qty: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiState {
    pub book: Option<BookSnapshot>,
    pub ws_connected: bool,
    pub latency_ms: f64,
    pub last_order_id: Option<u64>,
    /// (t_seconds, midpoint_usd) — time is seconds since the Tauri bridge started
    pub midpoint_series: Vec<(f64, f64)>,
    /// (t_seconds, fill_price_usd)
    pub fill_markers: Vec<(f64, f64)>,
    pub toasts: Vec<String>,
    pub engine_connections: u32,
    pub best_bid_cached: Option<i64>,
    pub best_ask_cached: Option<i64>,
}

/// Must match C++ `PRICE_SCALE` in `include/types.hpp` (1e6 units per $1).
pub const PRICE_SCALE: f64 = 1_000_000.0;

/// Fixed-point: internal price is dollars * PRICE_SCALE (e.g. $10.50 -> 10_500_000).
#[inline]
pub fn price_to_display(price: i64) -> f64 {
    price as f64 / PRICE_SCALE
}

#[inline]
pub fn midpoint(best_bid: i64, best_ask: Option<i64>) -> Option<f64> {
    let ask = best_ask?;
    if best_bid <= 0 || ask <= 0 || ask == i64::MAX {
        return None;
    }
    Some((price_to_display(best_bid) + price_to_display(ask)) / 2.0)
}
