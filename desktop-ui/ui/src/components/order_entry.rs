use crate::ipc::invoke_json;
use leptos::*;
use serde::Serialize;
use wasm_bindgen::JsCast;

#[derive(Serialize)]
struct SubmitArgs {
    side: String,
    price: String,
    qty: String,
    order_type: String,
    tif: String,
}

#[component]
pub fn OrderEntry(on_toast: WriteSignal<Vec<String>>) -> impl IntoView {
    let price = create_rw_signal(String::from("100500000"));
    let qty = create_rw_signal(String::from("10"));
    let order_type = create_rw_signal(String::from("LIMIT"));
    let tif = create_rw_signal(String::from("GTC"));
    let busy = create_rw_signal(false);

    let fire = move |side: &'static str| {
        let p = price.get();
        let q = qty.get();
        let ot = order_type.get();
        let tf = tif.get();
        busy.set(true);
        spawn_local(async move {
            let res: Result<(), String> = invoke_json(
                "submit_order",
                &SubmitArgs {
                    side: side.to_string(),
                    price: p,
                    qty: q,
                    order_type: ot,
                    tif: tf,
                },
            )
            .await;
            busy.set(false);
            if let Err(e) = res {
                on_toast.update(|t| t.push(format!("Order rejected: {e}")));
            }
        });
    };

    let quick_cancel = move |_| {
        busy.set(true);
        spawn_local(async move {
            let r: Result<(), String> = invoke_json("quick_cancel_last", &serde_json::json!({})).await;
            busy.set(false);
            if let Err(e) = r {
                on_toast.update(|t| t.push(format!("Cancel failed: {e}")));
            }
        });
    };

    view! {
        <div class="rounded border border-neutral-800 bg-black/40 p-3 space-y-3">
            <div class="text-xs uppercase tracking-widest text-neutral-500">order entry</div>
            <div class="grid grid-cols-2 gap-2">
                <label class="text-[10px] text-neutral-500">price (fixed-point)</label>
                <label class="text-[10px] text-neutral-500">qty</label>
                <input type="text" class="bg-neutral-950 border border-neutral-800 rounded px-2 py-1 text-sm"
                    prop:value=move || price.get()
                    on:input=move |ev| {
                        if let Some(t) = ev.target() {
                            if let Ok(el) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                price.set(el.value());
                            }
                        }
                    }
                />
                <input type="text" class="bg-neutral-950 border border-neutral-800 rounded px-2 py-1 text-sm"
                    prop:value=move || qty.get()
                    on:input=move |ev| {
                        if let Some(t) = ev.target() {
                            if let Ok(el) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                qty.set(el.value());
                            }
                        }
                    }
                />
            </div>
            <div class="flex flex-wrap gap-2">
                <button class="px-3 py-1 rounded bg-emerald-900/40 text-bid border border-emerald-800 disabled:opacity-40"
                    prop:disabled=move || busy.get()
                    on:click=move |_| fire("BUY")>"buy"</button>
                <button class="px-3 py-1 rounded bg-red-900/40 text-ask border border-red-800 disabled:opacity-40"
                    prop:disabled=move || busy.get()
                    on:click=move |_| fire("SELL")>"sell"</button>
            </div>
            <div class="grid grid-cols-2 gap-2 text-xs">
                <span class="text-neutral-500">type</span>
                <span class="text-neutral-500">tif</span>
                <select class="bg-neutral-950 border border-neutral-800 rounded px-1 py-1"
                    prop:value=move || order_type.get()
                    on:change=move |ev| {
                        if let Some(t) = ev.target() {
                            if let Ok(el) = t.dyn_into::<web_sys::HtmlSelectElement>() {
                                order_type.set(el.value());
                            }
                        }
                    }>
                    <option value="LIMIT">"LIMIT"</option>
                    <option value="MARKET">"MARKET"</option>
                </select>
                <select class="bg-neutral-950 border border-neutral-800 rounded px-1 py-1"
                    prop:value=move || tif.get()
                    on:change=move |ev| {
                        if let Some(t) = ev.target() {
                            if let Ok(el) = t.dyn_into::<web_sys::HtmlSelectElement>() {
                                tif.set(el.value());
                            }
                        }
                    }>
                    <option value="GTC">"GTC"</option>
                    <option value="IOC">"IOC"</option>
                    <option value="FOK">"FOK"</option>
                </select>
            </div>
            <button class="w-full py-1 rounded border border-amber-700/50 text-amber-200 text-xs disabled:opacity-40"
                prop:disabled=move || busy.get()
                on:click=quick_cancel>"quick cancel (last order id)"</button>
        </div>
    }
}
