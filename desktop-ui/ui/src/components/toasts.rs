use leptos::*;

#[component]
pub fn ToastStack(items: ReadSignal<Vec<String>>, set_items: WriteSignal<Vec<String>>) -> impl IntoView {
    view! {
        <div class="fixed bottom-4 right-4 z-50 flex flex-col gap-2 max-w-sm">
            <For
                each=move || {
                    items.get().into_iter().enumerate().collect::<Vec<_>>()
                }
                key=|(i, _)| *i
                children=move |(i, msg)| {
                    view! {
                        <div class="bg-neutral-900 border border-red-900/60 text-red-100 text-xs px-3 py-2 rounded shadow-lg flex justify-between gap-3">
                            <span>{msg}</span>
                            <button class="text-neutral-500 hover:text-white" on:click=move |_| {
                                set_items.update(|v| { if i < v.len() { v.remove(i); } });
                            }>"×"</button>
                        </div>
                    }
                }
            />
        </div>
    }
}
