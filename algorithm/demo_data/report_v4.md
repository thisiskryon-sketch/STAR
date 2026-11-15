# S.T.A.R. campaign report: EXP_ROVER_LOWSUN_MOBILITY_01

> **SYNTHETIC:** Run flagged as synthetic demo: results test the logic, not scientific accuracy.
> **SYNTHETIC:** 61 of 61 Earth cells are synthetic.
> **SYNTHETIC:** Mechanism evidence / correction models include synthetic placeholders.
> **SYNTHETIC:** Laboratory capability catalog includes synthetic placeholders.
> **SYNTHETIC:** Strict policy: synthetic content never yields Confirmed coverage; it stays Provisional.

Target: Moon / Shackleton_Connecting_Ridge (LUNAR_SP_SHACKLETON_01). Objective: Rover traction and navigation on rough slopes under low sun (synthetic demo).
Run: `star-EXP_ROVER_LOWSUN_MOBILITY_01-0bfc09219b7760ca`, 2026-11-15T16:22:56Z, data fnv1a64:0bfc09219b7760ca.

## Decision

**Lead field site undecided between Kamchatka_Volcanic and Iceland_Highlands: a decisive uncertainty is unknown. Rehearsal campaign: FIELD:Kamchatka_Volcanic:KAM_POS_2 + FIELD:Devon_Island_Arctic:DEVON_3 + LAB:TVAC_DUSTY_OPTICS. Before flight 0% of reducible requirements are confirmed, 100% testable. Mission-critical not confirmed: TR_TRACTION, TR_LOWSUN_NAV, TR_GRAVITY_SINKAGE. First action: TR_LOWSUN_NAV — Replace synthetic placeholder with independently verifiable data/evidence (evidence for 'sustained_low_sun_illumination' at DEVON_3)**

## 1. What Earth can reproduce

| Requirement | Earth cells within range | Observed Earth range |
|---|---|---|
| REQ_SLOPE_1X | 20 | 2.500 … 34.000 |
| REQ_ROUGH_1X | 11 | 0.050 … 1.450 |
| REQ_SHADOW_1DEG | 14 | 0.050 … 0.920 |

**Joint envelope:** all requirements met together at 10 independent sites (10 cells).

| Mechanism | Field | Lab | Note |
|---|---|---|---|
| granular_regolith_shear | yes | yes |  |
| high_vacuum_radiative_transfer | no | yes |  |
| sustained_low_sun_illumination | yes | yes | field only at |lat| ≥ 66° |
| sustained_lunar_gravity | no | no | irreducible on Earth |

**No single place or facility on Earth reproduces all required mechanisms** — the experiment has to be split into a campaign.

## 2. Candidate sites by discovery mode

**MissionMatch** (Ranked): Cells within epsilon (QC-valid, hard constraints passed) on Pareto fronts of (-d_mission, data quality); cells beyond epsilon follow as near-misses ordered by d_mission (front 0).
- KAM_POS_2 (Kamchatka_Volcanic) d = 0.016, front 1
- ICE_POS_3 (Iceland_Highlands) d = 0.034, front 2
- KAM_POS_3 (Kamchatka_Volcanic) d = 0.047, front 2
- ICE_POS_2 (Iceland_Highlands) d = 0.056, front 3
- KAM_POS_1 (Kamchatka_Volcanic) d = 0.086, front 3

**RareCombination** (Ranked): d_mission <= epsilon AND rarity status SupportedRareCombination (outliers excluded).
- PUNA_3 (Puna_Plateau) d = 0.753, front 1, metric 0.262
- PUNA_2 (Puna_Plateau) d = 0.754, front 1, metric 0.265
- PUNA_1 (Puna_Plateau) d = 0.754, front 1, metric 0.267
- DEVON_3 (Devon_Island_Arctic) d = 0.821, front 2, metric 0.245
- DEVON_2 (Devon_Island_Arctic) d = 0.851, front 3, metric 0.236

**UnderexploredRegion** (Ranked): d_mission <= epsilon AND outside every catalogued analog cluster; Pareto of (-d_mission, km from catalog).
- PUNA_3 (Puna_Plateau) d = 0.753, front 1, metric 10794.811
- PUNA_2 (Puna_Plateau) d = 0.754, front 2, metric 10784.077
- PUNA_1 (Puna_Plateau) d = 0.754, front 3, metric 10773.362

## 3. Site dossiers

| Site | d ± σ | In range | Rarity | AET | Transferable | Not transferable |
|---|---|---|---|---|---|---|
| KAM_POS_2 (Kamchatka_Volcanic) | 0.016 ± 0.092 | 3/3 | CommonTerrestrialTerrain | PartialTransfer | TR_TRACTION | TR_LOWSUN_NAV, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |
| ICE_POS_3 (Iceland_Highlands) | 0.034 ± n/a | 3/3 | CommonTerrestrialTerrain | Unknown |  | TR_LOWSUN_NAV, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |
| KAM_POS_3 (Kamchatka_Volcanic) | 0.047 ± 0.091 | 3/3 | CommonTerrestrialTerrain | PartialTransfer | TR_TRACTION | TR_LOWSUN_NAV, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |
| ICE_POS_2 (Iceland_Highlands) | 0.056 ± n/a | 3/3 | CommonTerrestrialTerrain | Unknown |  | TR_LOWSUN_NAV, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |
| KAM_POS_1 (Kamchatka_Volcanic) | 0.086 ± 0.100 | 3/3 | CommonTerrestrialTerrain | PartialTransfer | TR_TRACTION | TR_LOWSUN_NAV, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |
| ICE_POS_1 (Iceland_Highlands) | 0.093 ± n/a | 3/3 | CommonTerrestrialTerrain | Unknown |  | TR_LOWSUN_NAV, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |
| KAM_POS_4 (Kamchatka_Volcanic) | 0.110 ± 0.091 | 3/3 | CommonTerrestrialTerrain | PartialTransfer | TR_TRACTION | TR_LOWSUN_NAV, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |
| PUNA_3 (Puna_Plateau) | 0.753 ± 0.092 | 3/3 | SupportedRareCombination | Unknown |  | TR_LOWSUN_NAV, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |
| DEVON_3 (Devon_Island_Arctic) | 0.821 ± 0.100 | 1/3 | SupportedRareCombination | PartialTransfer | TR_LOWSUN_NAV | TR_TRACTION, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |
| PUNA_2 (Puna_Plateau) | 0.754 ± 0.092 | 3/3 | SupportedRareCombination | Unknown |  | TR_LOWSUN_NAV, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |
| PUNA_1 (Puna_Plateau) | 0.754 ± 0.092 | 3/3 | SupportedRareCombination | Unknown |  | TR_LOWSUN_NAV, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |
| DEVON_2 (Devon_Island_Arctic) | 0.851 ± 0.100 | 1/3 | SupportedRareCombination | PartialTransfer | TR_LOWSUN_NAV | TR_TRACTION, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |
| DEVON_1 (Devon_Island_Arctic) | 0.881 ± 0.100 | 1/3 | SupportedRareCombination | PartialTransfer | TR_LOWSUN_NAV | TR_TRACTION, TR_VACUUM_OPTICS, TR_GRAVITY_SINKAGE |

## 4. Rehearsal campaign (E.C.O.)

Status: AllReducibleTestableSomeProvisional. Cost model: UnknownCostsMinimisingAssetCount. Budget: NotConstrained. Duration: NotConstrained.

1. **FIELD:Kamchatka_Volcanic:KAM_POS_2** — confirms: none; testable only: REQ_ROUGH_1X, REQ_SHADOW_1DEG, REQ_SLOPE_1X, TR_TRACTION; upgrades: none
2. **FIELD:Devon_Island_Arctic:DEVON_3** — confirms: none; testable only: TR_LOWSUN_NAV; upgrades: none
3. **LAB:TVAC_DUSTY_OPTICS** — confirms: none; testable only: TR_VACUUM_OPTICS; upgrades: none

Confirmed before flight: **0%** of reducible requirements; testable: **100%**.
Single most similar site (FIELD:Iceland_Highlands:ICE_POS_3): confirmed 0%, testable 67%.
Exact check over 7 assets: greedy is optimal (exact optimum: FIELD:Kamchatka_Volcanic:KAM_POS_2 + FIELD:Devon_Island_Arctic:DEVON_3 + LAB:TVAC_DUSTY_OPTICS).
- Without FIELD:Kamchatka_Volcanic:KAM_POS_2: FIELD:Puna_Plateau:PUNA_3 + FIELD:Devon_Island_Arctic:DEVON_3 + LAB:TVAC_DUSTY_OPTICS — confirmed 0%, testable 100%
- Without FIELD:Devon_Island_Arctic:DEVON_3: FIELD:Kamchatka_Volcanic:KAM_POS_2 + LAB:LOWSUN_LIGHT_RIG + LAB:TVAC_DUSTY_OPTICS — confirmed 0%, testable 100%
- Without LAB:TVAC_DUSTY_OPTICS: FIELD:Kamchatka_Volcanic:KAM_POS_2 + FIELD:Devon_Island_Arctic:DEVON_3 — confirmed 0%, testable 83%

## 5. Residual risk register

| Requirement | Criticality | Pre-flight status | Rehearsed by | Missing for confirmation |
|---|---|---|---|---|
| REQ_SLOPE_1X — RmsSlope1x within [7, 16] deg | High | ProvisionalBeforeFlight | FIELD:Kamchatka_Volcanic:KAM_POS_2 | Replace synthetic placeholder with independently verifiable data/evidence (Earth features of KAM_POS_2) |
| REQ_ROUGH_1X — Roughness1x within [0.15, 0.6] m | Normal | ProvisionalBeforeFlight | FIELD:Kamchatka_Volcanic:KAM_POS_2 | Replace synthetic placeholder with independently verifiable data/evidence (Earth features of KAM_POS_2) |
| REQ_SHADOW_1DEG — ShadowObstruction1Deg within [0.4, 0.85] ratio | High | ProvisionalBeforeFlight | FIELD:Kamchatka_Volcanic:KAM_POS_2, FIELD:Devon_Island_Arctic:DEVON_3 | Replace synthetic placeholder with independently verifiable data/evidence (Earth features of DEVON_3); Replace synthetic placeholder with independently verifiable data/evidence (Earth features of KAM_POS_2) |
| TR_TRACTION — Wheel slip threshold calibration | MissionCritical | ProvisionalBeforeFlight | FIELD:Kamchatka_Volcanic:KAM_POS_2 | Replace synthetic placeholder with independently verifiable data/evidence (evidence for 'granular_regolith_shear' at KAM_POS_2) |
| TR_LOWSUN_NAV — Navigation camera / LiDAR performance in long shadows | MissionCritical | ProvisionalBeforeFlight | FIELD:Devon_Island_Arctic:DEVON_3 | Replace synthetic placeholder with independently verifiable data/evidence (evidence for 'sustained_low_sun_illumination' at DEVON_3) |
| TR_VACUUM_OPTICS — Optics and dust behaviour in vacuum | High | ProvisionalBeforeFlight | LAB:TVAC_DUSTY_OPTICS | Replace synthetic placeholder with independently verifiable data/evidence (capability/transfer evidence of TVAC_DUSTY_OPTICS) |
| TR_GRAVITY_SINKAGE — Sinkage and slip under sustained 1/6 g | MissionCritical | IrreducibleUntilFlight | none | - |

**Next actions (campaign U.D.D.):**
- [MissionCritical] TR_LOWSUN_NAV: Replace synthetic placeholder with independently verifiable data/evidence (evidence for 'sustained_low_sun_illumination' at DEVON_3)
- [MissionCritical] TR_TRACTION: Replace synthetic placeholder with independently verifiable data/evidence (evidence for 'granular_regolith_shear' at KAM_POS_2)
- [MissionCritical] TR_GRAVITY_SINKAGE: Cannot be rehearsed on Earth: carry as flight risk (design margin or in-situ verification).
- [High] REQ_SHADOW_1DEG: Replace synthetic placeholder with independently verifiable data/evidence (Earth features of DEVON_3)
- [High] REQ_SHADOW_1DEG: Replace synthetic placeholder with independently verifiable data/evidence (Earth features of KAM_POS_2)
- [High] REQ_SLOPE_1X: Replace synthetic placeholder with independently verifiable data/evidence (Earth features of KAM_POS_2)
- [High] TR_VACUUM_OPTICS: Replace synthetic placeholder with independently verifiable data/evidence (capability/transfer evidence of TVAC_DUSTY_OPTICS)
- [Normal] REQ_ROUGH_1X: Replace synthetic placeholder with independently verifiable data/evidence (Earth features of KAM_POS_2)

## 6. Lead-site decision (site U.D.D. + weight sensitivity)

#1 and #2 differ by Δd = 0.018; the uncertainty of REQ_ROUGH_1X is unknown, so whether this gap is real cannot be tested. Measure its uncertainty at both sites first.

Weight sensitivity (200 trials, each requirement weight × 2^U(-1,1), independent per trial): lead-site stability 0.96; leaders {"Iceland_Highlands": 0.035, "Kamchatka_Volcanic": 0.965}.

## 7. Rejected cells and what would change the decision

- HC_FAIL_STEEP_CLIFF (mission): RmsSlope1x = 34.000 deg must be [2, 24] (HC_SAFE_SLOPE); only a different site or a mission-approved relaxation changes this.
- QC_FAIL_LOW_COVERAGE (QC): Valid-pixel coverage must rise from 0.55 to >= 0.85 (re-acquire or gap-fill the DEM).
- QC_FAIL_RESOLUTION (QC): Resolution 30.0 m vs target 2.0 m exceeds ratio 4.0; resample or use a DEM closer to 2.0 m.

## Validation (logic checks)

H1 known-analog retrieval, k = 4: P@k 0.88, R@k 1.00, R-precision 1.00, NDCG@k 1.00; negative controls in global top-k: 0.
H2 noise robustness, 200 trials: mean Spearman 0.946, min 0.910, top-1 stability 0.21 (3 cells used assumed σ).

_Positive labels are known-analog roles, not proof of analogy for this experiment; on synthetic data this checks code logic only, not scientific accuracy._
