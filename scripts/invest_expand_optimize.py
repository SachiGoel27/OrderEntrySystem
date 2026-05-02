"""
Invest & Expand — optimal Research/Scale given Speed, plus opponent simulations.

PnL = research(r) * scale(s) * hit_rate_mult - BUDGET
research(x) = RESEARCH_MAX * log(1+x) / log(1+100)
scale(x) = SCALE_MAX * x / 100
Constraint: r + s + v = 100 (percentages), x in [0, 100] for pillar inputs.
"""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import numpy as np
import pandas as pd
from scipy.stats import skewnorm

BUDGET = 50_000.0
RESEARCH_MAX = 200_000.0
SCALE_MAX = 7.0
LOG101 = np.log(101.0)
N_OPPONENTS = 1000
N_DISTRIBUTIONS = 100
N_LOGNORMAL_SCENARIOS = 1000
RNG_SEED = 42


def research(pct: np.ndarray) -> np.ndarray:
    pct = np.asarray(pct, dtype=np.float64)
    return RESEARCH_MAX * np.log1p(pct) / LOG101


def scale(pct: np.ndarray) -> np.ndarray:
    pct = np.asarray(pct, dtype=np.float64)
    return SCALE_MAX * pct / 100.0


def newton_r_given_v(
    v: float,
    *,
    iters: int = 4,
    x0: float | None = None,
) -> float:
    """
    Solve (100 - v - r) = (1 + r) * ln(1 + r) for r in [0, 100 - v].
    Equivalent root: f(r) = v - 100 + r + (1+r)*ln(1+r) = 0.
    f'(r) = 2 + ln(1+r).
    """
    cap = max(0.0, 100.0 - v)
    if cap <= 0.0:
        return 0.0

    r = 23.0 - 0.2 * v if x0 is None else float(x0)
    r = np.clip(r, 0.0, cap)

    for _ in range(iters):
        rp1 = 1.0 + r
        f = v - 100.0 + r + rp1 * np.log(rp1)
        fp = 2.0 + np.log(rp1)
        if fp == 0.0:
            break
        r = r - f / fp
        r = np.clip(r, 0.0, cap)
    return float(r)


def competition_ranks(values: np.ndarray) -> np.ndarray:
    """
    Competition ranking (1-based): ties share rank; next rank skips.
    Example: [70,70,70,50] -> [1,1,1,4]
    """
    values = np.asarray(values, dtype=np.float64).ravel()
    n = values.size
    order = np.argsort(-values, kind="mergesort")
    ranks = np.empty(n, dtype=np.float64)
    ranks[order[0]] = 1.0
    for i in range(1, n):
        idx = order[i]
        prev = order[i - 1]
        if values[idx] == values[prev]:
            ranks[idx] = ranks[prev]
        else:
            ranks[idx] = float(i + 1)
    return ranks


def hit_rate_from_rank(rank: float, worst_rank: float) -> float:
    """Linear from 0.9 at best (rank 1) to 0.1 at worst_rank."""
    if worst_rank <= 1.0:
        return 0.9
    t = (rank - 1.0) / (worst_rank - 1.0)
    return 0.9 - 0.8 * t


def build_allocation_table(
    v_step: float = 0.1,
    *,
    newton_iters: int = 4,
) -> pd.DataFrame:
    """Optimal r,s for each speed v; PnL columns for hit-rate multiples 0.1..0.9."""
    vs = np.arange(0.0, 100.0 + 1e-9, v_step)
    rows = []
    mults = np.round(np.arange(0.1, 1.0, 0.1), 1)

    for v in vs:
        v = float(np.round(v, 10))
        r = newton_r_given_v(v, iters=newton_iters)
        s = 100.0 - r - v
        s = max(0.0, s)
        rv = research(np.array([r]))[0]
        sv = scale(np.array([s]))[0]
        gross_base = rv * sv
        row = {
            "speed_pct": v,
            "research_pct": r,
            "scale_pct": s,
            "research_value": rv,
            "scale_value": sv,
            "gross_research_times_scale": gross_base,
        }
        for m in mults:
            row[f"pnl_mult_{m:.1f}"] = gross_base * m - BUDGET
        rows.append(row)

    return pd.DataFrame(rows)


def hit_rate_us_first(all_speeds: np.ndarray) -> float:
    """
    Rank-based hit rate; linear 0.9 (best) .. 0.1 (worst).
    Caller must put our speed in all_speeds[0]; opponents follow.
    """
    all_speeds = np.asarray(all_speeds, dtype=np.float64).ravel()
    ranks = competition_ranks(all_speeds)
    worst = float(all_speeds.size)
    return hit_rate_from_rank(float(ranks[0]), worst)


def normal_scenarios() -> list[dict[str, float | str]]:
    """
    100 (mu, sigma) pairs on a 10x10 grid; mu +/- 3*sigma in [0, 100].
    Opponents drawn N(mu, sigma), clipped to [0, 100].
    """
    mus = np.linspace(22.0, 78.0, 10)
    sigs = np.linspace(3.0, 7.0, 10)
    rows: list[dict[str, float | str]] = []
    for mu in mus:
        for sig in sigs:
            if mu - 3.0 * sig < -1e-9 or mu + 3.0 * sig > 100.0 + 1e-9:
                continue
            m, s = float(mu), float(sig)
            rows.append(
                {
                    "distribution": f"Normal({m:.2f},{s:.2f})",
                    "opp_mean": m,
                    "opp_std": s,
                }
            )
    if len(rows) != N_DISTRIBUTIONS:
        raise RuntimeError(f"normal: expected {N_DISTRIBUTIONS} scenarios, got {len(rows)}")
    return rows


def sample_normal_opponents(sc: dict[str, float | str], rng: np.random.Generator) -> np.ndarray:
    opp = rng.normal(float(sc["opp_mean"]), float(sc["opp_std"]), size=N_OPPONENTS)
    return np.clip(opp, 0.0, 100.0)


def lognormal_scenarios() -> list[dict[str, float | str]]:
    """
    Opponent draws X ~ LogNormal with ln X ~ N(mu_ln, sigma_ln^2), then clip to [0, 100].

    Instead of gridding mu_ln directly (which mostly produced very low *level* means on
    [0, 100]), we grid an approximate **pre-clip mean** ``target_mean`` on the same scale
    as the other families (~15–85) and set

        mu_ln = ln(target_mean) - sigma_ln^2 / 2

    so E[X] = target_mean for the underlying lognormal. Pairs are kept only if roughly
    99.9% of mass sits below 100: mu_ln + 3*sigma_ln <= ln(100) - eps.

    We take N_LOGNORMAL_SCENARIOS points **evenly spaced through the sorted valid list**
    (by target_mean, then sigma) so scenario means vary smoothly from low to high.
    """
    log_cap = np.log(100.0) - 0.02
    triples: list[tuple[float, float, float]] = []
    # Wide mean range, aligned with Normal/SkewNormal-style centers; sigma includes small
    # values so high means remain feasible under the upper-tail cap.
    for m_tgt in np.linspace(14.0, 86.0, 72):
        for sig in np.linspace(0.05, 0.50, 72):
            m_tgt = float(m_tgt)
            sig = float(sig)
            if m_tgt <= 0.0:
                continue
            mu_ln = float(np.log(m_tgt) - 0.5 * sig * sig)
            if mu_ln + 3.0 * sig > log_cap:
                continue
            triples.append((m_tgt, mu_ln, sig))

    triples.sort(key=lambda t: (t[0], t[2]))
    n_tri = len(triples)
    if n_tri < N_LOGNORMAL_SCENARIOS:
        raise RuntimeError(
            f"lognormal: need {N_LOGNORMAL_SCENARIOS} scenarios, got {n_tri}; widen grid"
        )
    sel = (
        np.arange(N_LOGNORMAL_SCENARIOS, dtype=np.float64)
        * (n_tri - 1)
        / max(N_LOGNORMAL_SCENARIOS - 1, 1)
    )
    sel = np.round(sel).astype(np.int64)
    chosen = [triples[int(i)] for i in sel]

    return [
        {
            "distribution": f"LogNormal(E≈{m:.1f},σ_ln={sig:.2f})",
            "param_target_mean": m,
            "param_mu_ln": mu_ln,
            "param_sigma_ln": sig,
        }
        for m, mu_ln, sig in chosen
    ]


def sample_lognormal_opponents(sc: dict[str, float | str], rng: np.random.Generator) -> np.ndarray:
    opp = rng.lognormal(float(sc["param_mu_ln"]), float(sc["param_sigma_ln"]), size=N_OPPONENTS)
    return np.clip(opp, 0.0, 100.0)


def geometric_scenarios() -> list[dict[str, float | str]]:
    """
    (G - 1) * scale, G ~ Geometric(p) on {1,2,...} (NumPy parameterization), clipped to [0, 100].
    100 (p, scale) pairs; tails clipped so almost all mass in [0, 100] for typical draws.
    """
    ps = np.linspace(0.035, 0.20, 10)
    scales = np.linspace(2.5, 16.0, 10)
    rows: list[dict[str, float | str]] = []
    for p in ps:
        for sc in scales:
            pf, sf = float(p), float(sc)
            rows.append(
                {
                    "distribution": f"Geom(p={pf:.3f},scale={sf:.2f})",
                    "param_geom_p": pf,
                    "param_geom_scale": sf,
                }
            )
    if len(rows) != N_DISTRIBUTIONS:
        raise RuntimeError(f"geometric: expected {N_DISTRIBUTIONS} scenarios, got {len(rows)}")
    return rows


def sample_geometric_opponents(sc: dict[str, float | str], rng: np.random.Generator) -> np.ndarray:
    g = rng.geometric(float(sc["param_geom_p"]), size=N_OPPONENTS)
    raw = (g - 1).astype(np.float64) * float(sc["param_geom_scale"])
    return np.clip(raw, 0.0, 100.0)


def left_skew_normal_scenarios() -> list[dict[str, float | str]]:
    """
    Skew-normal with a < 0 (left / negative skew). loc, scale chosen so bulk lies in [0, 100];
    samples clipped to [0, 100].
    """
    alphas = np.linspace(-12.0, -2.0, 10)
    mus = np.linspace(32.0, 68.0, 10)
    sig = 7.0
    rows: list[dict[str, float | str]] = []
    for a in alphas:
        for mu in mus:
            af, mf = float(a), float(mu)
            rows.append(
                {
                    "distribution": f"SkewNormal(a={af:.1f},loc={mf:.1f},scale={sig:.1f})",
                    "param_skew_a": af,
                    "param_skew_loc": mf,
                    "param_skew_scale": float(sig),
                }
            )
    if len(rows) != N_DISTRIBUTIONS:
        raise RuntimeError(f"skew-normal: expected {N_DISTRIBUTIONS} scenarios, got {len(rows)}")
    return rows


def sample_skewnormal_opponents(sc: dict[str, float | str], rng: np.random.Generator) -> np.ndarray:
    opp = skewnorm.rvs(
        float(sc["param_skew_a"]),
        loc=float(sc["param_skew_loc"]),
        scale=float(sc["param_skew_scale"]),
        size=N_OPPONENTS,
        random_state=rng,
    )
    return np.clip(np.asarray(opp, dtype=np.float64), 0.0, 100.0)


def run_scenarios(
    table: pd.DataFrame,
    rng: np.random.Generator,
    scenarios: list[dict[str, float | str]],
    sample_opponents: Callable[[dict[str, float | str], np.random.Generator], np.ndarray],
) -> tuple[pd.DataFrame, pd.DataFrame]:
    """
    For each scenario, draw opponents, search best our speed on the allocation table.
    Returns (per_scenario_df, aggregate_df).
    """
    v_col = table["speed_pct"].to_numpy()
    r_col = table["research_pct"].to_numpy()
    s_col = table["scale_pct"].to_numpy()
    gross = table["gross_research_times_scale"].to_numpy()

    summaries: list[dict[str, float | str]] = []

    for sc in scenarios:
        opp = sample_opponents(sc, rng)
        emp_mean = float(np.mean(opp))
        emp_std = float(np.std(opp))

        best_v = 0.0
        best_pnl = -np.inf
        best_mult = 0.0
        best_i = 0

        for i, v in enumerate(v_col):
            ours = np.concatenate([[v], opp])
            mult = hit_rate_us_first(ours)
            pnl = gross[i] * mult - BUDGET
            if pnl > best_pnl:
                best_pnl = pnl
                best_v = float(v)
                best_mult = float(mult)
                best_i = i

        row: dict[str, float | str] = {**sc}
        row["opp_mean_empirical"] = emp_mean
        row["opp_std_empirical"] = emp_std
        row["best_speed_pct"] = best_v
        row["best_research_pct"] = float(r_col[best_i])
        row["best_scale_pct"] = float(s_col[best_i])
        row["implied_hit_rate"] = best_mult
        row["best_pnl"] = float(best_pnl)
        summaries.append(row)

    sim_df = pd.DataFrame(summaries)
    agg_row: dict[str, float | str] = {"distribution": "AGGREGATE (mean over sims)"}
    for c in sim_df.columns:
        if c == "distribution":
            continue
        agg_row[c] = float(sim_df[c].mean())
    agg = pd.DataFrame([agg_row])
    agg = agg[sim_df.columns]
    return sim_df, agg


def run_simulations(
    table: pd.DataFrame,
    rng: np.random.Generator,
) -> tuple[pd.DataFrame, pd.DataFrame]:
    """Backward-compatible: Gaussian opponents (same as before, plus empirical opp stats)."""
    return run_scenarios(table, rng, normal_scenarios(), sample_normal_opponents)


def main() -> None:
    rng = np.random.default_rng(RNG_SEED)

    table = build_allocation_table(v_step=0.1, newton_iters=4)
    out_dir = Path(__file__).resolve().parent
    table_path = out_dir / "invest_expand_allocation_table.csv"
    table.to_csv(table_path, index=False)

    sim_df, agg = run_simulations(table, rng)
    sim_full = pd.concat([sim_df, agg], ignore_index=True)
    sim_path = out_dir / "invest_expand_simulation_results.csv"
    sim_full.to_csv(sim_path, index=False)

    # Separate streams so each output file is reproducible regardless of run order.
    ln_df, ln_agg = run_scenarios(
        table,
        np.random.default_rng(RNG_SEED + 1),
        lognormal_scenarios(),
        sample_lognormal_opponents,
    )
    (pd.concat([ln_df, ln_agg], ignore_index=True)).to_csv(
        out_dir / "invest_expand_simulation_lognormal.csv", index=False
    )

    geom_df, geom_agg = run_scenarios(
        table,
        np.random.default_rng(RNG_SEED + 2),
        geometric_scenarios(),
        sample_geometric_opponents,
    )
    (pd.concat([geom_df, geom_agg], ignore_index=True)).to_csv(
        out_dir / "invest_expand_simulation_geometric.csv", index=False
    )

    skew_df, skew_agg = run_scenarios(
        table,
        np.random.default_rng(RNG_SEED + 3),
        left_skew_normal_scenarios(),
        sample_skewnormal_opponents,
    )
    (pd.concat([skew_df, skew_agg], ignore_index=True)).to_csv(
        out_dir / "invest_expand_simulation_left_skew_normal.csv", index=False
    )

    print(f"Wrote {table_path} ({len(table)} rows)")
    print(f"Wrote {sim_path} ({len(sim_full)} rows)")
    print(f"Wrote {out_dir / 'invest_expand_simulation_lognormal.csv'} ({len(ln_df) + 1} rows)")
    print(f"Wrote {out_dir / 'invest_expand_simulation_geometric.csv'} ({len(geom_df) + 1} rows)")
    print(f"Wrote {out_dir / 'invest_expand_simulation_left_skew_normal.csv'} ({len(skew_df) + 1} rows)")
    print("\nFirst 5 allocation rows:")
    print(table.head().to_string())
    print("\nLast 3 simulation rows (incl. aggregate):")
    print(sim_full.tail(3).to_string())


if __name__ == "__main__":
    main()
