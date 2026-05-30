# High-Performance Limit Order Book & Matching Engine
 
A full-stack order entry system built in C++ with a Python simulation and analytics layer. The engine implements a real-time limit order book with three allocation modes( FIFO, Pro-Rata, and a novel Hybrid mode driven by an adaptive stability metric) and exposes a WebSocket API for live order submission, execution reporting, and telemetry streaming.
 
---
 
## Performance
 
Benchmarked by submitting 10,000 randomized limit orders over a local WebSocket connection.
 
| Metric | Value |
|--------|-------|
| Throughput | **21,946 orders / second** |
| p50 latency | **0.19 ms** |
| p95 latency | **0.36 ms** |
| p99 latency | **0.54 ms** |
| p99 / p50 ratio | **2.8×** (consistency measure) |
| Max latency (10k orders) | 0.65 ms |
 
The bottleneck is the WebSocket + JSON stack. The raw C++ order book operations (BST insertion, linked-list fill, hash map cancel) run in the single-digit microsecond range.
 
---
 
## Architecture
 
```
┌─────────────────────────────────────┐
│         Python Simulation Layer      │
│  MarketMaker · HFT · Retail · Whale  │
│        stress_test.py                │
└────────────────┬────────────────────┘
                 │  WebSocket (JSON)
                 ▼
┌─────────────────────────────────────┐
│         C++ Matching Engine          │
│   Crow WebSocket Server · port 8080  │
│   OrderBook · OrderPool · Telemetry  │
│        main.cpp                      │
└────────────────┬────────────────────┘
                 │
        ┌────────▼────────┐
        │   order_book    │
        │  ┌───────────┐  │
        │  │  bids BST │  │  std::map<Price, PriceLevel, greater>
        │  │  asks BST │  │  std::map<Price, PriceLevel, less>
        │  │  order_map│  │  std::unordered_map<OrderID, Order*>
        │  └───────────┘  │
        └─────────────────┘
                 │
┌────────────────▼────────────────────┐
│         Python Analytics Layer       │
│  visualize.py · latency_test.py      │
│  experiment_results.csv              │
└─────────────────────────────────────┘
```
 
---
 
## Data Structures
 
| Structure | Purpose | Complexity |
|-----------|---------|------------|
| `std::map` (red-black tree) | Bid and ask price levels, sorted by price | O(log N) insert/remove, O(1) best-price access |
| Doubly-linked list | Orders within each price level, in arrival order | O(1) insert at tail, O(1) remove anywhere |
| `std::unordered_map` | Cancel-by-ID: OrderID → Order pointer | O(1) lookup and erase |
| Object pool | Pre-allocated Order structs (10,000 at startup) | O(1) acquire and release, zero heap allocation on hot path |
 
---
 
## Allocation Modes
 
### FIFO
Oldest resting order at a price level fills completely before the next one receives anything. Rewards queue position and early commitment. Standard on most equity exchanges.
 
### Pro-Rata
Every resting order fills in proportion to its size. Rewards liquidity provision. Used on some options exchanges.
 
### Hybrid (novel)
The FIFO fraction is determined dynamically by the book's current stability using the heat function:
 
```
f(x) = 50 / (e^k - 1) · (e^(kx / 3000) - 1) + 50
```
 
where `x = clamp(S, 0, 3000)` and `S` is the stability metric. Maps `S ∈ [0, 3000] → FIFO% ∈ [50%, 100%]`. In stressed markets the engine leans pro-rata; in calm, deep markets it leans FIFO.
 
**The stability metric:**
 
```
S = L_eff / (H_c + H_p + 1)
```
 
- `L_eff` — effective liquidity: volume at top N price levels, decay-weighted by distance from best price, divided by √(order_count)
- `H_c` — cancel heat: +1 per cancel, ×0.95 per 10ms tick
- `H_p` — price heat: +|Δmidpoint| per tick move, ×0.95 per 10ms tick
**Parameter `k` controls the curve shape:**
 
| k | Behavior |
|---|----------|
| 0 | Linear — FIFO share rises steadily with stability |
| 1 | Moderately convex — best empirical result |
| 4.5 | Strongly convex — near pro-rata until very stable |
| 9+ | Step-like — essentially binary switch |
 
**Two-phase allocation inside the hybrid mode:**
1. `fifo_target = round(fill_qty × fifo_share)` shares go to FIFO (front-to-back queue priority)
2. Remaining `fill_qty - fifo_target` shares go to pro-rata on residual quantities
3. Integer rounding: floor division base fills, remainder distributed by largest fractional remainder with FIFO rank as tiebreaker
---
 
## Order Types
 
| Type | Description |
|------|-------------|
| `LIMIT` | Rests on book at specified price if no immediate match |
| `MARKET` | No price limit, sweeps available liquidity |
| `GTC` | Good Till Cancelled — rests until filled or explicitly cancelled |
| `IOC` | Immediate Or Cancel — fills what it can, cancels remainder |
| `FOK` | Fill Or Kill — fills entirely or not at all |
 
---
 
## Experimental Results
 
Ran a k-sweep experiment across k = 0, 1, 2, 3, 4, 5 comparing Hybrid vs pure FIFO in both whale and no-whale market conditions.
 
**Key findings (no-whale, 60-second runs):**
 
| Mode | avg Stability S | MM PnL | HFT PnL | Retail PnL |
|------|----------------|--------|---------|------------|
| FIFO | 317.5 | +1,918 | -451 | -498 |
| Hybrid k=1 | **337.4 (+6.3%)** | **+2,349** | -416 | -690 |
 
- Hybrid k=1 achieves **6.3% higher average stability** than pure FIFO
- Market maker PnL improves **22%** under hybrid — pro-rata distributes fills more evenly between competing MMs rather than giving everything to queue leader
- Retail PnL worsens slightly — the efficiency tradeoff of distributing fills across more orders
- Under whale conditions, stability difference narrows to <1% — large market sweeps overwhelm any allocation-mode effect
---
 
## File Structure
 
```
├── main.cpp               # Crow WebSocket server, CLI args, telemetry thread
├── order_book.hpp         # OrderBook class declaration, HybridConfig, MatchingDiagnostics
├── order_book.cpp         # Full matching engine implementation
├── order.hpp              # Order struct, OrderPool (object pool)
├── price_level.hpp        # PriceLevel struct (doubly-linked list per price)
├── price_level.cpp        # PriceLevel add/remove/snapshot
├── types.hpp              # Price, Quantity, OrderID typedefs; fixed-point constants
├── test_matching.cpp      # Unit tests: FIFO priority, pro-rata, hybrid, FOK/IOC/market
├── crow_all.h             # Crow single-header WebSocket/HTTP framework
│
├── stress_test.py         # Multi-agent simulation (MM, HFT, Retail, Whale)
├── visualize.py           # Dashboard + agent PnL charts (both phases)
├── latency_test.py        # Latency benchmark: round-trip, fill, throughput sweep
└── run_experiment.py      # K-sweep experiment runner
```
 
---
 
## Build & Run
 
**Requirements:** C++17, a compiler with `std::map`, `std::unordered_map`, and `<thread>` support (GCC 8+ or Clang 7+), Python 3.11+.
 
```bash
# Build the matching engine
g++ -std=c++17 -O2 -pthread \
    main.cpp order_book.cpp price_level.cpp \
    -o matching_engine
 
# Run unit tests
g++ -std=c++17 -O2 -pthread \
    test_matching.cpp order_book.cpp price_level.cpp \
    -o test_matching && ./test_matching
 
# Install Python dependencies
pip install websockets numpy pandas matplotlib
 
# Start the engine (hybrid mode, k=1)
./matching_engine 1.0 hybrid
 
# Run simulation (no whale)
python stress_test.py 1.0 --no-whale
 
# Visualize results
python visualize.py
 
# Latency benchmark
python latency_test.py --orders 10000
```
 
**CLI arguments for the engine:**
```
./matching_engine [k] [mode]
 
  k     Heat function curvature (default: 4.5)
  mode  fifo | prorata | hybrid  (default: hybrid)
 
Examples:
  ./matching_engine 1.0 hybrid    # hybrid, k=1
  ./matching_engine 0   fifo      # pure FIFO baseline
```
 
**CLI arguments for stress_test.py:**
```
python stress_test.py [k] [--no-whale] [--fifo] [--prorata]
```
 
---
 
## K-Sweep Experiment
 
To reproduce the stability comparison across k values:
 
```bash
# Run for each k value (repeat with k = 0, 1, 2, 3, 4, 5)
./matching_engine 1 hybrid
python stress_test.py 1 --no-whale
# Ctrl-C engine, repeat for next k
 
# Run FIFO baseline
./matching_engine 0 fifo
python stress_test.py 0 --fifo --no-whale
 
# Visualize the sweep
python visualize.py --best-k 1
```
 
Each run appends one row to `experiment_results.csv`. The visualizer auto-detects all rows and produces three figures: stability vs k, PnL vs k, and a summary comparison at the optimal k.
 
---
 
## Latency Benchmark
 
```bash
./matching_engine 0 fifo
 
# Full benchmark — 10,000 orders at max speed
python latency_test.py
 
# Controlled rate
python latency_test.py --orders 10000 --rate 2000
 
# Print only, no chart
python latency_test.py --no-plot
```
 
Produces a three-panel chart: latency histogram with p50/p95/p99 lines, throughput curve (latency vs order rate), and CDF for both round-trip and fill latency.
 
---
 
## Unit Tests
 
Seven test cases covering the full allocation surface:
 
| Test | What it verifies |
|------|-----------------|
| FIFO same-price priority | Oldest order fills first; partial fill leaves correct remainder |
| Pro-rata deterministic remainders | Proportional allocation with correct integer rounding |
| Hybrid minimum FIFO floor | 50% floor is respected regardless of stability |
| Hybrid high stability | Near-FIFO behavior when stability is maximized |
| Price priority across levels | Best price exhausted before moving to next level |
| Time-in-force semantics | FOK rejects correctly; IOC never rests; MARKET ignores price |
| Diagnostics and midpoint guard | Heat metrics update correctly; guard suppresses heat on thin levels |
 
---
 
## PnL Methodology
 
Mark-to-mid PnL, computed per fill:
 
```
BUY fill:   PnL += qty × (mid_at_fill − fill_price)
SELL fill:  PnL += qty × (fill_price − mid_at_fill)
```
 
`mid_at_fill` is captured inside the C++ engine at match time — before the fill changes the book — and broadcast in the execution report. Both sides of every trade receive the identical mid value, making PnL zero-sum per matched pair. Measures adverse selection (negative for aggressive orders crossing the spread) and spread capture (positive for passive orders resting at better-than-mid prices).
 
---
 
## Simulation Agents
 
| Agent | Sigma | Strategy | Expected PnL |
|-------|-------|----------|--------------|
| MarketMaker (×2) | 1 | Passive ±2 tick quotes, 500-lot, re-quote every 500ms | Positive (earns spread) |
| RetailTrader (×5) | 10 | Random market orders 1–20 lots, 1–3s intervals | Negative (pays spread) |
| HFTSniper (×3) | 3 | Passive ±1 tick quotes, 1-lot, 20×/second | Slightly negative (adverse selection) |
| WhaleTrader (×1) | 5 | 3,000-lot market sweep every 10–20s | Large negative (market impact) |
