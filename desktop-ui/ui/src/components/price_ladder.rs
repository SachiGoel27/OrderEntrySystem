use crate::types::{price_to_display, BookLevel, BookSnapshot};
use leptos::*;

fn max_qty(levels: &[BookLevel]) -> i32 {
    levels.iter().map(|l| l.qty).max().unwrap_or(1).max(1)
}

#[component]
pub fn PriceLadder(book: ReadSignal<Option<BookSnapshot>>, ws_connected: ReadSignal<bool>) -> impl IntoView {
    view! {
        <div
            class=move || {
                let base = "rounded border p-3 min-h-[520px] flex flex-col transition-colors duration-300 ";
                if ws_connected.get() {
                    format!("{base} border-neutral-800")
                } else {
                    format!("{base} border-amber-900/50 ring-1 ring-amber-900/30")
                }
            }
            style:background-color=move || {
                if ws_connected.get() {
                    "rgba(0,0,0,0.45)".to_string()
                } else {
                    "rgba(48,48,48,0.92)".to_string()
                }
            }
        >
            <div class="flex items-center justify-between mb-2">
                <div class="text-xs uppercase tracking-widest text-neutral-500">L2 ladder (top 10)</div>
                <div class="flex items-center gap-2 text-[10px] text-neutral-400">
                    <span
                        class=move || {
                            if ws_connected.get() {
                                "inline-block h-2 w-2 rounded-full bg-emerald-500 shadow shadow-emerald-500/40"
                            } else {
                                "inline-block h-2 w-2 rounded-full bg-neutral-500"
                            }
                        }
                    />
                    <span>{move || if ws_connected.get() { "engine link".to_string() } else { "offline".to_string() }}</span>
                </div>
            </div>
            <div class="flex-1 flex flex-col text-sm">
                {move || {
                    let b = match book.get() {
                        Some(v) => v,
                        None => return view! { <div class="text-neutral-600">no book data</div> }.into_view(),
                    };

                    let max_v = max_qty(&b.bids).max(max_qty(&b.asks));
                    let asks_dom: Vec<BookLevel> = b.asks.iter().take(10).rev().cloned().collect();
                    let bids_show: Vec<BookLevel> = b.bids.iter().take(10).cloned().collect();
                    let mid = crate::types::midpoint(b.best_bid, b.best_ask);
                    let spread_txt = match (b.best_bid, b.best_ask) {
                        (bb, Some(ba)) if bb > 0 && ba < i64::MAX => {
                            let spr = price_to_display(ba - bb);
                            let m = mid.map(|m| format!("{m:.6}")).unwrap_or_default();
                            format!("{spr:.6}  |  mid {m}")
                        }
                        _ => "—".to_string(),
                    };

                    view! {
                        <div class="flex-1 flex flex-col">
                            <div class="text-neutral-500 text-xs mb-1">asks</div>
                            <div class="flex flex-col gap-0.5 flex-1 justify-end">
                                {asks_dom.into_iter().map(|lvl| {
                                    let intensity = (lvl.qty as f64 / max_v as f64).clamp(0.0, 1.0);
                                    let bg = format!("rgba(255,59,92,{:.2})", 0.08 + intensity * 0.35);
                                    view! {
                                        <div class="flex justify-between px-2 py-1 rounded border border-red-900/30"
                                             style:background-color=bg>
                                            <span class="text-ask">{format!("{:.6}", price_to_display(lvl.price))}</span>
                                            <span class="text-neutral-300">{lvl.qty}</span>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                            <div class="my-2 py-2 text-center border-y border-neutral-800 bg-neutral-950/80">
                                <div class="text-[10px] text-neutral-500">spread / mid</div>
                                <div class="text-emerald-200 font-semibold">{spread_txt}</div>
                            </div>
                            <div class="text-neutral-500 text-xs mb-1">bids</div>
                            <div class="flex flex-col gap-0.5 flex-1">
                                {bids_show.into_iter().map(|lvl| {
                                    let intensity = (lvl.qty as f64 / max_v as f64).clamp(0.0, 1.0);
                                    let bg = format!("rgba(0,255,163,{:.2})", 0.08 + intensity * 0.35);
                                    view! {
                                        <div class="flex justify-between px-2 py-1 rounded border border-emerald-900/30"
                                             style:background-color=bg>
                                            <span class="text-bid">{format!("{:.6}", price_to_display(lvl.price))}</span>
                                            <span class="text-neutral-300">{lvl.qty}</span>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        </div>
                    }.into_view()
                }}
            </div>
        </div>
    }
}
