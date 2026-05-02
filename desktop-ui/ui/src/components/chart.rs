use leptos::*;

/// Plot area (SVG user units)
const PX0: f64 = 64.0;
const PX1: f64 = 536.0;
const PY0: f64 = 32.0;
const PY1: f64 = 228.0;

fn map_x(t: f64, t_lo: f64, t_hi: f64) -> f64 {
    if (t_hi - t_lo).abs() < 1e-9 {
        return (PX0 + PX1) / 2.0;
    }
    PX0 + (t - t_lo) / (t_hi - t_lo) * (PX1 - PX0)
}

fn map_y(p: f64, p_lo: f64, p_hi: f64) -> f64 {
    if (p_hi - p_lo).abs() < 1e-9 {
        return (PY0 + PY1) / 2.0;
    }
    PY1 - (p - p_lo) / (p_hi - p_lo) * (PY1 - PY0)
}

#[component]
pub fn ExecutionChart(
    series: ReadSignal<Vec<(f64, f64)>>,
    fills: ReadSignal<Vec<(f64, f64)>>,
) -> impl IntoView {
    /// When `Some((y_lo, y_hi))`, Y-axis uses this window; `None` = auto-fit to data.
    let y_manual = create_rw_signal(None::<(f64, f64)>);

    let re_center = move |_| {
        let pts = series.get();
        if let Some((_, last_p)) = pts.last() {
            let mid = *last_p;
            let half = (mid.abs() * 0.0005).max(0.05);
            y_manual.set(Some((mid - half, mid + half)));
        }
    };

    let auto_y = move |_| {
        y_manual.set(None);
    };

    view! {
        <div class="rounded border border-neutral-800 bg-black/40 p-3 flex flex-col gap-2 min-h-[300px]">
            <div class="flex flex-wrap items-center justify-between gap-2">
                <div class="text-xs uppercase tracking-widest text-neutral-500">
                    midpoint vs time (best bid / best ask on the book)
                </div>
                <div class="flex gap-2">
                    <button
                        type="button"
                        class="px-2 py-0.5 rounded border border-cyan-900/60 text-cyan-200 text-[10px] hover:bg-cyan-950/50"
                        on:click=re_center
                    >
                        "Re-center Y"
                    </button>
                    <button
                        type="button"
                        class="px-2 py-0.5 rounded border border-neutral-700 text-neutral-400 text-[10px] hover:bg-neutral-900"
                        on:click=auto_y
                    >
                        "Auto Y"
                    </button>
                </div>
            </div>
            <div class="text-[10px] text-neutral-600 leading-snug">
                X: seconds since the desktop app connected. Y: midpoint = (best bid + best ask) / 2 in display dollars (engine fixed-point / 1_000_000).
            </div>
            <svg class="w-full h-[240px]" viewBox="0 0 560 260" preserveAspectRatio="xMidYMid meet">
                {move || {
                    let pts = series.get();
                    let fill_pts = fills.get();

                    if pts.is_empty() {
                        return view! {
                            <text x="20" y="120" fill="#666" font-size="13">waiting for book / midpoint…</text>
                        }
                        .into_view();
                    }

                    let mut t_lo = pts.iter().map(|(t, _)| *t).fold(f64::INFINITY, f64::min);
                    let mut t_hi = pts.iter().map(|(t, _)| *t).fold(f64::NEG_INFINITY, f64::max);
                    for (t, _) in &fill_pts {
                        t_lo = t_lo.min(*t);
                        t_hi = t_hi.max(*t);
                    }
                    if pts.len() == 1 {
                        t_lo -= 1.0;
                        t_hi += 1.0;
                    } else if (t_hi - t_lo) < 0.5 {
                        let c = (t_lo + t_hi) / 2.0;
                        t_lo = c - 0.5;
                        t_hi = c + 0.5;
                    }

                    let mut p_lo = pts.iter().map(|(_, p)| *p).fold(f64::INFINITY, f64::min);
                    let mut p_hi = pts.iter().map(|(_, p)| *p).fold(f64::NEG_INFINITY, f64::max);
                    for (_, p) in &fill_pts {
                        p_lo = p_lo.min(*p);
                        p_hi = p_hi.max(*p);
                    }
                    let pad_p = ((p_hi - p_lo).abs() * 0.08).max(1e-6);
                    p_lo -= pad_p;
                    p_hi += pad_p;

                    if let Some((lo, hi)) = y_manual.get() {
                        if hi > lo {
                            p_lo = lo;
                            p_hi = hi;
                        }
                    }

                    let mut d = String::new();
                    for (i, (t, p)) in pts.iter().enumerate() {
                        let x = map_x(*t, t_lo, t_hi);
                        let y = map_y(*p, p_lo, p_hi);
                        if i == 0 {
                            d.push_str(&format!("M {x:.2},{y:.2} "));
                        } else {
                            d.push_str(&format!("L {x:.2},{y:.2} "));
                        }
                    }

                    let x_axis_y = PY1 + 4.0;
                    let markers = fill_pts
                        .into_iter()
                        .map(|(t, p)| {
                            let x = map_x(t, t_lo, t_hi);
                            let y = map_y(p, p_lo, p_hi);
                            view! {
                                <circle
                                    cx=format!("{:.2}", x)
                                    cy=format!("{:.2}", y)
                                    r="3.0"
                                    fill="#fbbf24"
                                    opacity="0.9"
                                    stroke="#78350f"
                                    stroke-width="0.6"
                                />
                            }
                        })
                        .collect_view();

                    let nx = 5usize;
                    let x_ticks = (0..nx)
                        .map(|i| {
                            let u = i as f64 / (nx - 1).max(1) as f64;
                            let tv = t_lo + u * (t_hi - t_lo);
                            let x = map_x(tv, t_lo, t_hi);
                            view! {
                                <g>
                                    <line
                                        x1=format!("{:.1}", x)
                                        y1=format!("{:.1}", PY1)
                                        x2=format!("{:.1}", x)
                                        y2=format!("{:.1}", x_axis_y + 2.0)
                                        stroke="#444"
                                        stroke-width="1"
                                    />
                                    <text
                                        x=format!("{:.1}", x - 12.0)
                                        y=format!("{:.1}", x_axis_y + 14.0)
                                        fill="#888"
                                        font-size="9"
                                    >
                                        {format!("{:.1}s", tv)}
                                    </text>
                                </g>
                            }
                        })
                        .collect_view();

                    let ny = 5usize;
                    let y_ticks = (0..ny)
                        .map(|i| {
                            let u = i as f64 / (ny - 1).max(1) as f64;
                            let pv = p_lo + u * (p_hi - p_lo);
                            let y = map_y(pv, p_lo, p_hi);
                            view! {
                                <g>
                                    <line
                                        x1=format!("{:.1}", PX0 - 4.0)
                                        y1=format!("{:.1}", y)
                                        x2=format!("{:.1}", PX1)
                                        y2=format!("{:.1}", y)
                                        stroke="#2a2a2a"
                                        stroke-width="1"
                                        stroke-dasharray="3,3"
                                    />
                                    <text x="6" y=format!("{:.1}", y + 3.0) fill="#888" font-size="9">
                                        {format!("{:.6}", pv)}
                                    </text>
                                </g>
                            }
                        })
                        .collect_view();

                    view! {
                        <rect x="0" y="0" width="560" height="260" fill="#050505" />
                        {y_ticks}
                        <line
                            x1=format!("{:.1}", PX0)
                            y1=format!("{:.1}", PY0)
                            x2=format!("{:.1}", PX0)
                            y2=format!("{:.1}", PY1)
                            stroke="#555"
                            stroke-width="1.2"
                        />
                        <line
                            x1=format!("{:.1}", PX0)
                            y1=format!("{:.1}", PY1)
                            x2=format!("{:.1}", PX1)
                            y2=format!("{:.1}", PY1)
                            stroke="#555"
                            stroke-width="1.2"
                        />
                        {x_ticks}
                        <path
                            d=d
                            fill="none"
                            stroke="#22d3ee"
                            stroke-width="1.6"
                            vector-effect="non-scaling-stroke"
                        />
                        {markers}
                        <text x="64" y="252" fill="#777" font-size="10">"time (s)"</text>
                        <text x="8" y="130" fill="#777" font-size="9">"mid ($)"</text>
                    }
                    .into_view()
                }}
            </svg>
            <div class="text-[10px] text-neutral-600">gold: last traded price at fill time</div>
        </div>
    }
}
