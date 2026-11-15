# S.T.A.R. v0.6.1 — Earth-based Mission Rehearsal Planning

**Principle.** STAR does not look for the Moon's twin on Earth. It answers three questions:
- which parts of a lunar experiment can be rehearsed on Earth;
- which rehearsal results can be trusted;
- which risk remains unverified until flight.

STAR does not have to name a winner every time. It has to know when the data are not sufficient for a defensible choice.

## Rules that set STAR apart

1. **Testability ≠ Transferability.** Being able to run a test (field site or test stand) does not prove that its result transfers to the Moon. Every requirement coverage has one of two statuses:
   - **Confirmed** — the test can be run, the uncertainty is within tolerance, and transfer is supported by evidence applicable to this site;
   - **Provisional** — the test can be run, but nothing confirms it yet.
2. **The unknown never becomes a positive result.** Each of the following gives Provisional, not Confirmed:
   - unknown σ;
   - evidence that does not state its domain of applicability;
   - a site outside the evidence domain;
   - a hard-constraint pass within k·σ of the bound.
3. **Synthetic content never yields Confirmed** (`Strict` policy, the default). Synthetic Earth data, evidence, correction models and lab catalog entries keep coverage Provisional with a "replace synthetic data" item. `--trust-synthetic` (`TreatAsConfirmedForLogicDemo`) is for demonstrating the logic only; every report then says "LOGIC DEMO MODE".
4. **Critical requirements are never traded away.** The portfolio objective is lexicographic: critical confirmed > critical provisional > confirmed > provisional. No number of lower-class requirements outweighs one critical requirement.

## Pipeline

Pipeline: Mission Spec → mission distance (z within tolerances) → Earth Envelope (marginal / joint / mechanism) → C.N.D. modes → site dossiers with AET → E.C.O. → residual risk register → decision.

- **E.C.O.** A constrained greedy heuristic.
  - With ≤ 16 assets an exact enumeration also runs, and the report states whether the greedy choice is optimal.
  - For each asset the report shows what is lost without it, including critical confirmations.
  - Unknown cost or duration is never reported as meeting a constraint: status `Unverifiable`.
- **Time windows.** For a time-dependent mechanism (`requires_field_time_window`, e.g. low sun), a field test counts as follows:
  - window unknown → Provisional;
  - window does not overlap the campaign period (`period_start_utc` / `period_end_utc`) → the site does not cover this test;
  - window overlaps → the window is attached to the asset.
- **Tie-break on equal status:** the asset with fewer open items before confirmation wins.
- **Residual risk register.** For each engineering decision:
  - status: ConfirmedBeforeFlight / ProvisionalBeforeFlight / NotRehearsedInPlan / NoTerrestrialAsset / IrreducibleUntilFlight;
  - who rehearses it;
  - what is missing before confirmation.
- **U.D.D.**
  - **Campaign:** actions that turn Provisional into Confirmed, ordered by criticality.
  - **Site:** #1 is compared with the best cell of a *different* site. The recommended measurement is the one that *resolves* the choice (Δd ≥ 2σ after measuring), not simply the noisiest feature. σ_Δ is always flagged as computed under the assumption of independent errors (no covariance is computed). If both sites come from the same DEM source, the report adds that errors are likely correlated and the risk is overstated.
- **Weight sensitivity.** Weights are perturbed by ×2^U(−1,1) over 200 runs. If the leader holds in fewer than 80% of runs, the verdict is `WeightSensitive`.
- **Lead-site verdict:** `Resolved` / `TiedWithinUncertainty` / `UndecidedUncertaintyUnknown` / `WeightSensitive` / `NoCandidate`.
- **Multiple regions (`--all-targets`).** The same experiment, with the same tolerance widths, runs for every lunar region in `targets.json`. Regions are ordered by remaining pre-flight risk *for this experiment*, with no single blended score. This is not a landing-safety ranking.

## Run

```
cargo test --workspace                                           # 53 tests
cargo run -p star-cli -- --demo --all-targets                    # strict: Confirmed = 0% on synthetic data
cargo run -p star-cli -- --demo --all-targets --trust-synthetic  # logic demonstration only
cargo run -p star-cli -- --input-dir DIR --output OUT.json [--target-id ID] [--validate] [--all-targets]
cargo run -p star-cli -- --import MANIFEST.json --out-dir DIR [--force]          # teammate star-temp-0 exports -> STAR inputs
cargo run -p star-cli -- --lowsun-windows --region R --mechanism M --lat LAT --year Y [--max-elev 3]
```

Real-data run: see `DATA_PLAN.md` and `real_data_template/`.

## New input fields (all have defaults; old JSON still loads)

- `criticality` on requirements: `MissionCritical | High | Normal | Exploratory`.
- `valid_feature_bounds` and `independent_validation` on mechanism evidence.
- `transfer_evidence` on laboratories.
- `requires_field_time_window` on mechanisms; `site_windows.json`; `period_start_utc` / `period_end_utc` in `campaign_constraints.json`.
- `synthetic_evidence_policy` in `params.json`.
- `hard_constraint_sigma_margin`, `sensitivity_trials`, `exact_check_max_assets` in `params.json`.

## Synthetic content

All cells, evidence, laboratories and values for the Malapert / de Gerlache / Haworth regions are synthetic, and the report says so. Region names are real; values are placeholders until replaced with LOLA-derived features.
