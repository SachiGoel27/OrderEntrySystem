mod components;
mod ipc;
mod types;

use components::{
    chart::ExecutionChart, health::HealthMonitor, order_entry::OrderEntry, price_ladder::PriceLadder,
    toasts::ToastStack,
};
use ipc::invoke_json;
use leptos::*;
use std::sync::atomic::{AtomicBool, Ordering};
use types::{BookSnapshot, UiState};
use wasm_bindgen::prelude::*;

static POLL_STARTED: AtomicBool = AtomicBool::new(false);

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App/> });
}

#[component]
fn App() -> impl IntoView {
    let (book, set_book) = create_signal(Option::<BookSnapshot>::None);
    let (series, set_series) = create_signal(Vec::<(f64, f64)>::new());
    let (fills, set_fills) = create_signal(Vec::<(f64, f64)>::new());
    let (ws_ok, set_ws) = create_signal(false);
    let (latency, set_latency) = create_signal(0.0_f64);
    let (last_order, set_last_order) = create_signal(Option::<u64>::None);
    let (connections, set_connections) = create_signal(0_u32);
    let (bbc, set_bbc) = create_signal(Option::<i64>::None);
    let (bac, set_bac) = create_signal(Option::<i64>::None);
    let (toasts, set_toasts) = create_signal(Vec::<String>::new());

    if !POLL_STARTED.swap(true, Ordering::SeqCst) {
        spawn_local(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(16).await;
                match invoke_json::<UiState>("get_ui_state", &serde_json::json!({})).await {
                    Ok(s) => {
                        set_book.set(s.book);
                        set_series.set(s.midpoint_series);
                        set_fills.set(s.fill_markers);
                        set_ws.set(s.ws_connected);
                        set_latency.set(s.latency_ms);
                        set_last_order.set(s.last_order_id);
                        set_connections.set(s.engine_connections);
                        set_bbc.set(s.best_bid_cached);
                        set_bac.set(s.best_ask_cached);
                        for t in s.toasts {
                            set_toasts.update(|v| v.push(t));
                        }
                    }
                    Err(e) => {
                        set_ws.set(false);
                        set_toasts.update(|v| v.push(format!("state sync: {e}")));
                    }
                }
            }
        });
    }

    let ping = move |_| {
        spawn_local(async move {
            let r: Result<bool, String> = invoke_json("ping_engine", &serde_json::json!({})).await;
            if let Err(e) = r {
                set_toasts.update(|v| v.push(format!("ping: {e}")));
            }
        });
    };

    view! {
        <div class="min-h-screen bg-oesbg text-neutral-200 p-3 flex flex-col gap-3">
            <header class="flex items-center justify-between border-b border-neutral-800 pb-2">
                <div>
                    <div class="text-sm tracking-widest text-neutral-400">OES DESKTOP</div>
                    <div class="text-[10px] text-neutral-600">Tauri + Leptos | engine ws://127.0.0.1:8080/ws</div>
                </div>
                <div class="flex items-center gap-2 text-xs">
                    <span class="text-neutral-500">last id</span>
                    <span class="text-emerald-300 font-mono">{move || last_order.get().map(|x| x.to_string()).unwrap_or_else(|| "—".into())}</span>
                    <button class="px-2 py-1 rounded border border-cyan-800 text-cyan-200 text-xs" on:click=ping>"ping"</button>
                </div>
            </header>
            <div class="grid grid-cols-1 xl:grid-cols-3 gap-3 flex-1">
                <PriceLadder book=book ws_connected=ws_ok />
                <div class="flex flex-col gap-3">
                    <ExecutionChart series=series fills=fills />
                    <HealthMonitor
                        ws=ws_ok
                        latency=latency
                        connections=connections
                        best_bid_cached=bbc
                        best_ask_cached=bac
                    />
                </div>
                <div class="flex flex-col gap-3">
                    <OrderEntry on_toast=set_toasts />
                </div>
            </div>
            <ToastStack items=toasts set_items=set_toasts />
        </div>
    }
}
