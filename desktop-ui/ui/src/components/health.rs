use leptos::*;

#[component]
pub fn HealthMonitor(
    ws: ReadSignal<bool>,
    latency: ReadSignal<f64>,
    connections: ReadSignal<u32>,
    best_bid_cached: ReadSignal<Option<i64>>,
    best_ask_cached: ReadSignal<Option<i64>>,
) -> impl IntoView {
    view! {
        <div class="rounded border border-neutral-800 bg-black/40 p-3 space-y-2 text-xs">
            <div class="text-[10px] uppercase tracking-widest text-neutral-500">system health</div>
            <div class="flex justify-between">
                <span class="text-neutral-500">ws bridge</span>
                <span class=move || if ws.get() { "text-emerald-400" } else { "text-red-400" }>
                    {move || if ws.get() { "connected" } else { "disconnected" }}
                </span>
            </div>
            <div class="flex justify-between">
                <span class="text-neutral-500">ping rtt (ms)</span>
                <span>{move || format!("{:.2}", latency.get())}</span>
            </div>
            <div class="flex justify-between">
                <span class="text-neutral-500">engine connections</span>
                <span>{move || connections.get().to_string()}</span>
            </div>
            <div class="flex justify-between">
                <span class="text-neutral-500">top-of-book cache (bid / ask)</span>
                <span class="text-neutral-300">
                    {move || {
                        let bb = best_bid_cached.get().map(|v| v.to_string()).unwrap_or_else(|| "—".into());
                        let ba = best_ask_cached.get().map(|v| v.to_string()).unwrap_or_else(|| "—".into());
                        format!("{bb} / {ba}")
                    }}
                </span>
            </div>
            <div class="text-[10px] text-neutral-600 leading-snug">
                cache fields mirror OrderBook best bid/ask; UI poll throttled to ~60Hz.
            </div>
        </div>
    }
}
