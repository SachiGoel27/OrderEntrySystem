# OES Desktop (Tauri + Leptos)

Desktop execution UI for the C++ matching engine in the parent repo. The Tauri backend maintains a **Tokio** WebSocket client to `ws://127.0.0.1:8080/ws`, parses JSON pushed by the engine, **throttles book/midpoint commits to ~60Hz**, and exposes state via **`get_ui_state`**. The Leptos CSR frontend polls that command on a **~16ms** interval and updates **fine-grained signals** (ladder, chart, health, order entry). The midpoint chart uses **elapsed seconds since the bridge started** on the X axis and **midpoint = (best bid + best ask) / 2** (display dollars, fixed-point ÷ 100) on the Y axis; fills are overlaid at their wall time.

## Prerequisites

- Rust stable + `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- [Trunk](https://trunkrs.dev/) (`cargo install trunk`)
- C++ engine built and listening on **port 8080** (`make` in repo root → `bin/oes_engine`)

## Dev workflow (two terminals)

1. **Engine:** from repo root, run `./bin/oes_engine` (or Windows equivalent).
2. **UI assets:** `cd desktop-ui/ui` then `trunk serve` (serves on **http://127.0.0.1:1420** — avoids clashing with the engine on 8080).
3. **Shell:** `cd desktop-ui/src-tauri` then `cargo tauri dev`.

`tauri.conf.json` points `devUrl` at `http://127.0.0.1:1420` and `frontendDist` at `../ui/dist` for release builds (`trunk build --release` in `ui/` first).

## Engine protocol (JSON)

The engine emits structured lines (see `src/main.cpp` in the parent repo):

- `{"type":"book", ...}` — top-of-book + top 10 bid/ask levels, `connections`, cached best bid/ask.
- `{"type":"fill", ...}` — trade prints for chart markers.
- `{"type":"ack", ...}` / `{"type":"error", ...}` / `{"type":"cancel_result", ...}` — order lifecycle.
- `{"type":"client_pong", ...}` — latency measurement for `client_ping`.

## Tailwind

The UI uses the **Tailwind Play CDN** in `ui/index.html` for a low-light Bloomberg-style palette (see `tailwind.config` inline script).

## Notes

- If Tauri **capability / permission** errors appear on `invoke`, widen `src-tauri/capabilities/default.json` per your installed Tauri 2.x docs.
- Price display uses **÷ 1_000_000** to match C++ `PRICE_SCALE` in `include/types.hpp` (wire format is still integer `price` fields in JSON).
