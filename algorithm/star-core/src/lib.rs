// star-core/src/lib.rs
//! S.T.A.R. core: mission-driven analog discovery and experiment-campaign planning.
//!
//! Pipeline: QC -> Earth Envelope (marginal / joint / mechanism) -> hard constraints
//! -> mission distance -> C.N.D. modes (Mission Match, Rare Combination,
//! Underexplored Region) -> site dossiers with AET -> E.C.O. portfolio -> U.D.D.
pub mod assessment;
pub mod dna;
pub mod envelope;
pub mod novelty;
pub mod pareto;
pub mod schema;
pub mod shadows;

use assessment::{
    build_udd_recommendation, evaluate_aet_transferability, optimize_campaign_portfolio,
    AetAssessment, CampaignAssetCapability, CampaignAssetKind, CampaignConstraints,
    CampaignPortfolio, CampaignRequirement, CoverageClaim, CoverageStatus, EvidenceLevel,
    FlipAssessment, LabFacility, SiteMechanismEvidence, SiteWindow, TransferabilityStatus,
    UncertaintyRecommendation, VenueCheck,
};
use dna::{build_analog_dna, AnalogDna};
use envelope::{compute_feature_scales, evaluate_earth_envelope, mission_distance, EnvelopeAssessment, MissionDistance};
use novelty::{compute_novelty_metrics, NoveltyMetrics, RaritySupportStatus};
use pareto::{compute_pareto_fronts, ParetoInput};
use schema::{
    check_target_consistency, validate_cells, CandidateRejection, Cell, DataOrigin, FeatureScales,
    Criticality, FeatureFamily, KnownAnalogSite, MissionExperimentSpec, ProvenanceMetadata,
    RejectionReason, RunManifest, RunParameters, SchemaError, SyntheticEvidencePolicy, TargetProfile,
    TestVenue,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DiscoveryMode {
    /// Best fulfilment of the mission requirements (objectives: -d_mission, data quality).
    MissionMatch,
    /// Mission-feasible combinations that are rare on Earth and supported by independent sites.
    RareCombination,
    /// Mission-feasible sites far from the known-analog catalog.
    UnderexploredRegion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModeStatus {
    Ranked,
    NoEligibleCandidates,
    NoCatalogProvided,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedEntry {
    pub cell_id: String,
    pub region: String,
    pub d_mission: f64,
    /// 0 for near-misses listed after the epsilon-eligible fronts.
    pub pareto_front_rank: usize,
    pub within_epsilon: bool,
    pub mode_metric: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeRanking {
    pub mode: DiscoveryMode,
    pub status: ModeStatus,
    pub eligibility_rule: String,
    pub objective_names: Vec<String>,
    pub entries: Vec<RankedEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQuality {
    pub valid_pixel_ratio: f64,
    pub vertical_uncertainty_m: f64,
    pub resolution_m: f64,
    pub active_feature_uncertainty_known: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateDossier {
    pub cell_id: String,
    pub region: String,
    pub lat: f64,
    pub lon: f64,
    pub data_origin: DataOrigin,
    pub provenance: Option<ProvenanceMetadata>,
    pub data_quality: DataQuality,
    pub within_soft_epsilon: bool,
    /// (mode, 1-based position in that mode's ranking)
    pub appears_in_modes: Vec<(DiscoveryMode, usize)>,
    pub observable_match: AnalogDna,
    pub novelty: NoveltyMetrics,
    pub aet: AetAssessment,
    /// Hard constraints passed by less than k·σ (pass is not confirmed by the data).
    pub constraint_margin_warnings: Vec<String>,
    pub ranking_explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryReport {
    pub manifest: RunManifest,
    pub parameters: RunParameters,
    pub mission_spec: MissionExperimentSpec,
    pub target_id: String,
    pub consistency_warnings: Vec<String>,
    pub envelope: EnvelopeAssessment,
    pub scales: FeatureScales,
    pub rejections: Vec<CandidateRejection>,
    pub epsilon_cutoff_count: usize,
    pub epsilon_sensitivity_counts: Vec<(f64, usize)>,
    pub mode_rankings: Vec<ModeRanking>,
    pub dossiers: Vec<CandidateDossier>,
    pub campaign_assets: Vec<CampaignAssetCapability>,
    pub campaign_portfolio: CampaignPortfolio,
    /// Pre-flight status of every requirement: what the rehearsal confirms, what stays open.
    pub residual_risk_register: Vec<ResidualRiskEntry>,
    /// Campaign-level U.D.D.: actions that turn provisional / missing items into confirmed ones.
    pub campaign_next_actions: Vec<NextAction>,
    pub udd_recommendation: UncertaintyRecommendation,
    pub weight_sensitivity: WeightSensitivity,
    pub decision: DecisionSummary,
    pub synthetic_content_warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RehearsalStatus {
    ConfirmedBeforeFlight,
    ProvisionalBeforeFlight,
    /// An asset could test it, but the plan under the constraints does not include one.
    NotRehearsedInPlan,
    NoTerrestrialAsset,
    IrreducibleUntilFlight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidualRiskEntry {
    pub requirement_id: String,
    pub is_transfer_requirement: bool,
    pub engineering_decision: String,
    pub criticality: Criticality,
    pub mechanism_id: Option<String>,
    pub status: RehearsalStatus,
    pub rehearsed_by: Vec<String>,
    pub missing_for_confirmation: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextAction {
    pub requirement_id: String,
    pub criticality: Criticality,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightSensitivity {
    pub trials: usize,
    pub perturbation: String,
    /// Share of trials in which the lead site (region) stays the lead.
    pub lead_site_stability: Option<f64>,
    /// Region -> share of trials it leads.
    pub leaders: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LeadSiteVerdict {
    NoCandidate,
    /// The data cannot separate the leading sites; treat them as equivalent options.
    TiedWithinUncertainty { sites: Vec<String> },
    /// Uncertainty of a decisive requirement is unknown: no defensible choice yet.
    UndecidedUncertaintyUnknown { sites: Vec<String> },
    /// The lead changes with plausible requirement weights.
    WeightSensitive { leaders: Vec<String> },
    Resolved { site: String, cell_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionSummary {
    pub lead_site: LeadSiteVerdict,
    pub campaign_assets: Vec<String>,
    pub confirmed_coverage: f64,
    pub testable_coverage: f64,
    pub mission_critical_unconfirmed: Vec<String>,
    pub irreducible_until_flight: Vec<String>,
    pub first_action: Option<NextAction>,
    pub statement: String,
}

pub struct DiscoveryInputs<'a> {
    pub manifest: RunManifest,
    pub params: &'a RunParameters,
    pub spec: &'a MissionExperimentSpec,
    pub target: &'a TargetProfile,
    pub cells: &'a [Cell],
    pub catalog: &'a [KnownAnalogSite],
    pub precomputed_scales: Option<&'a FeatureScales>,
    pub evidence: &'a [SiteMechanismEvidence],
    pub labs: &'a [LabFacility],
    /// Known time windows of time-dependent mechanisms at field sites.
    pub site_windows: &'a [SiteWindow],
    pub campaign_constraints: &'a CampaignConstraints,
}

/// QC stage shared by the pipeline and the validation harness.
pub fn qc_filter<'a>(
    cells: &'a [Cell],
    spec: &MissionExperimentSpec,
    target: &TargetProfile,
    params: &RunParameters,
) -> (Vec<&'a Cell>, Vec<CandidateRejection>) {
    let max_ratio = spec.effective_max_resolution_ratio(params.max_resolution_ratio);
    let mut ok = Vec::new();
    let mut rejected = Vec::new();
    for cell in cells {
        let mut reasons = Vec::new();
        if cell.valid_pixel_ratio < params.min_valid_pixel_ratio {
            reasons.push(RejectionReason::LowValidPixelCoverage {
                observed: cell.valid_pixel_ratio,
                required: params.min_valid_pixel_ratio,
            });
        }
        if cell.vertical_uncertainty_m > params.max_vertical_uncertainty_m {
            reasons.push(RejectionReason::ExcessiveVerticalUncertainty {
                observed: cell.vertical_uncertainty_m,
                max_allowed: params.max_vertical_uncertainty_m,
            });
        }
        let ratio = (cell.resolution_m / target.resolution_m).max(target.resolution_m / cell.resolution_m);
        if ratio > max_ratio {
            reasons.push(RejectionReason::IncompatibleSpatialResolution {
                cell_res_m: cell.resolution_m,
                target_res_m: target.resolution_m,
                max_ratio,
            });
        }
        if reasons.is_empty() {
            ok.push(cell);
        } else {
            rejected.push(CandidateRejection::new(&cell.id, true, reasons));
        }
    }
    (ok, rejected)
}

pub fn hard_constraint_violations(spec: &MissionExperimentSpec, cell: &Cell) -> Vec<RejectionReason> {
    spec.hard_constraints
        .iter()
        .filter_map(|hc| {
            let v = cell.features.get(hc.feature);
            let ok = hc.min_allowed.map_or(true, |m| v >= m) && hc.max_allowed.map_or(true, |m| v <= m);
            (!ok).then(|| RejectionReason::HardConstraintViolation {
                constraint_id: hc.constraint_id.clone(),
                feature: hc.feature,
                observed: v,
                min_allowed: hc.min_allowed,
                max_allowed: hc.max_allowed,
            })
        })
        .collect()
}

fn rank_mode(
    mode: DiscoveryMode,
    rule: &str,
    names: &[&str],
    rows: Vec<(&Cell, f64, Vec<Option<f64>>, Option<f64>)>,
) -> ModeRanking {
    if rows.is_empty() {
        return ModeRanking {
            mode,
            status: ModeStatus::NoEligibleCandidates,
            eligibility_rule: rule.into(),
            objective_names: names.iter().map(|s| s.to_string()).collect(),
            entries: Vec::new(),
        };
    }
    let inputs: Vec<ParetoInput> = rows
        .iter()
        .map(|(c, d, obj, _)| ParetoInput { cell_id: c.id.clone(), objectives: obj.clone(), tie_key: *d })
        .collect();
    let lookup: BTreeMap<&str, &(&Cell, f64, Vec<Option<f64>>, Option<f64>)> =
        rows.iter().map(|r| (r.0.id.as_str(), r)).collect();
    let entries = compute_pareto_fronts(&inputs, names)
        .into_iter()
        .map(|a| {
            let r = lookup[a.cell_id.as_str()];
            RankedEntry {
                cell_id: a.cell_id.clone(),
                region: r.0.region.clone(),
                d_mission: r.1,
                pareto_front_rank: a.pareto_front_rank,
                within_epsilon: true,
                mode_metric: r.3,
            }
        })
        .collect();
    ModeRanking {
        mode,
        status: ModeStatus::Ranked,
        eligibility_rule: rule.into(),
        objective_names: names.iter().map(|s| s.to_string()).collect(),
        entries,
    }
}

pub fn discover_candidates_v4(inp: DiscoveryInputs) -> Result<DiscoveryReport, SchemaError> {
    let (params, spec, target) = (inp.params, inp.spec, inp.target);
    params.validate()?;
    spec.validate()?;
    validate_cells(inp.cells)?;
    let consistency_warnings = check_target_consistency(spec, target)?;
    let active = spec.active_features();

    // 1. QC and Earth Envelope on QC-valid cells (independent of mission constraints).
    let (qc_cells, mut rejections) = qc_filter(inp.cells, spec, target, params);
    let envelope = evaluate_earth_envelope(spec, &qc_cells, params);
    let scales = match inp.precomputed_scales {
        Some(s) => s.clone(),
        None => compute_feature_scales(&qc_cells, &inp.manifest.calibration_version).unwrap_or(FeatureScales {
            calibration_version: inp.manifest.calibration_version.clone(),
            sample_count: 0,
            p01: BTreeMap::new(),
            p99: BTreeMap::new(),
            degenerate_features: Vec::new(),
        }),
    };

    // 2. Mission hard constraints.
    let mut feasible: Vec<&Cell> = Vec::new();
    for &c in &qc_cells {
        let v = hard_constraint_violations(spec, c);
        if v.is_empty() {
            feasible.push(c);
        } else {
            rejections.push(CandidateRejection::new(&c.id, false, v));
        }
    }

    // 3. Mission distance + epsilon handling.
    let mut distances: Vec<(&Cell, MissionDistance)> = feasible
        .iter()
        .map(|&c| (c, mission_distance(spec, &c.features, &c.feature_uncertainty)))
        .collect();
    distances.sort_by(|a, b| a.1.d.total_cmp(&b.1.d).then_with(|| a.0.id.cmp(&b.0.id)));
    let eps = params.epsilon_mission_distance;
    let epsilon_sensitivity_counts = [0.5, 1.0, 1.5, 2.0, 3.0]
        .iter()
        .map(|&e| (e, distances.iter().filter(|(_, m)| m.d <= e).count()))
        .collect();
    let epsilon_cutoff_count = distances.iter().filter(|(_, m)| m.d > eps).count();
    if params.epsilon_is_hard_constraint {
        for (c, m) in distances.iter().filter(|(_, m)| m.d > eps) {
            rejections.push(CandidateRejection::new(
                &c.id,
                false,
                vec![RejectionReason::MissionDistanceExceedsHardEpsilon { d_mission: m.d, epsilon: eps }],
            ));
        }
        distances.retain(|(_, m)| m.d <= eps);
    }
    rejections.sort_by(|a, b| a.cell_id.cmp(&b.cell_id));

    // 4. Novelty for eligible cells (pool = all QC-valid Earth cells).
    let novelty: BTreeMap<String, NoveltyMetrics> = distances
        .iter()
        .map(|(c, _)| (c.id.clone(), compute_novelty_metrics(c, &qc_cells, inp.catalog, &active, &scales, params)))
        .collect();

    // 5. C.N.D. modes, each with its own eligibility and objectives.
    // Mission Match: Pareto (fit vs data quality) only among cells within epsilon, so that a
    // high-quality but unsuitable cell can never reach front 1; near-misses follow, by d.
    let mm_rows = distances
        .iter()
        .filter(|(_, m)| m.d <= eps)
        .map(|(c, m)| (*c, m.d, vec![Some(-m.d), Some(novelty[&c.id].heuristic_quality_score)], None))
        .collect();
    let mut mission_match = rank_mode(
        DiscoveryMode::MissionMatch,
        "Cells within epsilon (QC-valid, hard constraints passed) on Pareto fronts of (-d_mission, data quality); \
         cells beyond epsilon follow as near-misses ordered by d_mission (front 0).",
        &["-d_mission", "data_quality"],
        mm_rows,
    );
    for (c, m) in distances.iter().filter(|(_, m)| m.d > eps) {
        mission_match.entries.push(RankedEntry {
            cell_id: c.id.clone(),
            region: c.region.clone(),
            d_mission: m.d,
            pareto_front_rank: 0,
            within_epsilon: false,
            mode_metric: None,
        });
        mission_match.status = ModeStatus::Ranked;
    }
    let rare_rows = distances
        .iter()
        .filter(|(c, m)| m.d <= eps && novelty[&c.id].rarity_status == RaritySupportStatus::SupportedRareCombination)
        .map(|(c, m)| {
            let r = novelty[&c.id].earth_rarity_knn_distance;
            (*c, m.d, vec![Some(-m.d), r], r)
        })
        .collect();
    let rare = rank_mode(
        DiscoveryMode::RareCombination,
        "d_mission <= epsilon AND rarity status SupportedRareCombination (outliers excluded).",
        &["-d_mission", "earth_rarity_knn"],
        rare_rows,
    );
    let under = if inp.catalog.is_empty() {
        ModeRanking {
            mode: DiscoveryMode::UnderexploredRegion,
            status: ModeStatus::NoCatalogProvided,
            eligibility_rule: "Requires a known-analog catalog; absent catalog is not zero novelty.".into(),
            objective_names: vec!["-d_mission".into(), "geographic_distance_km".into()],
            entries: Vec::new(),
        }
    } else {
        let rows = distances
            .iter()
            .filter(|(c, m)| m.d <= eps && novelty[&c.id].geographic_distance_km.map_or(false, |g| g > 0.0))
            .map(|(c, m)| {
                let g = novelty[&c.id].geographic_distance_km;
                (*c, m.d, vec![Some(-m.d), g], g)
            })
            .collect();
        rank_mode(
            DiscoveryMode::UnderexploredRegion,
            "d_mission <= epsilon AND outside every catalogued analog cluster; Pareto of (-d_mission, km from catalog).",
            &["-d_mission", "geographic_distance_km"],
            rows,
        )
    };
    let mode_rankings = vec![mission_match, rare, under];

    // 6. Dossiers for the Top-N of every mode.
    let by_id: BTreeMap<&str, &(&Cell, MissionDistance)> = distances.iter().map(|x| (x.0.id.as_str(), x)).collect();
    let evidence: BTreeMap<&str, &SiteMechanismEvidence> = inp.evidence.iter().map(|e| (e.cell_id.as_str(), e)).collect();
    let mut order: Vec<String> = Vec::new();
    let mut appears: BTreeMap<String, Vec<(DiscoveryMode, usize)>> = BTreeMap::new();
    for mr in &mode_rankings {
        for (i, e) in mr.entries.iter().take(params.top_n_dossiers).enumerate() {
            if !appears.contains_key(&e.cell_id) {
                order.push(e.cell_id.clone());
            }
            appears.entry(e.cell_id.clone()).or_default().push((mr.mode, i + 1));
        }
    }
    // Every region inside the soft epsilon gets its best cell as a site representative,
    // so that E.C.O. sees all candidate sites, not only the Top-N cells.
    let mut represented: BTreeSet<String> = order.iter().map(|id| by_id[id.as_str()].0.region.clone()).collect();
    for (i, e) in mode_rankings[0].entries.iter().enumerate() {
        if e.d_mission <= eps && represented.insert(e.region.clone()) && !appears.contains_key(&e.cell_id) {
            order.push(e.cell_id.clone());
            appears.entry(e.cell_id.clone()).or_default().push((DiscoveryMode::MissionMatch, i + 1));
        }
    }
    let mm_front: BTreeMap<&str, usize> = mode_rankings[0]
        .entries
        .iter()
        .map(|e| (e.cell_id.as_str(), e.pareto_front_rank))
        .collect();
    let dossiers: Vec<CandidateDossier> = order
        .iter()
        .map(|id| {
            let (c, md) = by_id[id.as_str()];
            let dna = build_analog_dna(&c.id, md);
            let aet = evaluate_aet_transferability(spec, c, &dna, evidence.get(c.id.as_str()).copied());
            let n_req = dna.breakdown.len();
            let n_in = dna.breakdown.iter().filter(|r| r.within_range).count();
            let explanation = format!(
                "d_mission = {:.3}{} ({} of {} requirements within acceptable range{}); Mission Match front {}{}.",
                md.d,
                md.sigma_d.map(|s| format!(" ± {:.3}", s)).unwrap_or_else(|| " (σ unknown)".into()),
                n_in,
                n_req,
                if dna.limiting_requirements.is_empty() {
                    String::new()
                } else {
                    format!("; limiting: {}", dna.limiting_requirements.join(", "))
                },
                mm_front.get(id.as_str()).copied().unwrap_or(0),
                if md.d > eps { "; beyond soft epsilon (near-miss)" } else { "" },
            );
            CandidateDossier {
                cell_id: c.id.clone(),
                region: c.region.clone(),
                lat: c.lat,
                lon: c.lon,
                data_origin: c.data_origin,
                provenance: c.provenance.clone(),
                data_quality: DataQuality {
                    valid_pixel_ratio: c.valid_pixel_ratio,
                    vertical_uncertainty_m: c.vertical_uncertainty_m,
                    resolution_m: c.resolution_m,
                    active_feature_uncertainty_known: active.iter().all(|f| c.feature_uncertainty.contains_key(f)),
                },
                within_soft_epsilon: md.d <= eps,
                appears_in_modes: appears[id].clone(),
                observable_match: dna,
                novelty: novelty[id].clone(),
                aet,
                constraint_margin_warnings: margin_warnings(spec, c, params.hard_constraint_sigma_margin),
                ranking_explanation: explanation,
            }
        })
        .collect();

    // 7. Rehearsal assets. Testability (can the test be run?) and transferability (can its
    //    result be trusted on the Moon?) are separate: only the latter gives Confirmed coverage.
    let mut assets: Vec<CampaignAssetCapability> = Vec::new();
    let mut seen_regions = BTreeSet::new();
    let mm_pos: BTreeMap<&str, usize> =
        mode_rankings[0].entries.iter().enumerate().map(|(i, e)| (e.cell_id.as_str(), i)).collect();
    let mut site_order: Vec<&CandidateDossier> = dossiers.iter().collect();
    site_order.sort_by_key(|d| mm_pos.get(d.cell_id.as_str()).copied().unwrap_or(usize::MAX));
    for d in site_order {
        if !d.within_soft_epsilon || !seen_regions.insert(d.region.clone()) {
            continue;
        }
        assets.push(field_asset(spec, d, params.synthetic_evidence_policy, inp.site_windows, inp.campaign_constraints));
    }
    for lab in inp.labs {
        assets.push(lab_asset(spec, lab, params.synthetic_evidence_policy));
    }
    let campaign_reqs: Vec<CampaignRequirement> = spec
        .required_parameters
        .iter()
        .map(|r| CampaignRequirement {
            requirement_id: r.requirement_id.clone(),
            weight: r.weight,
            criticality: r.criticality,
            irreducible: false,
        })
        .chain(spec.transfer_requirements.iter().map(|t| CampaignRequirement {
            requirement_id: t.requirement_id.clone(),
            weight: t.weight,
            criticality: t.criticality,
            irreducible: spec.mechanism(&t.required_mechanism_id).map_or(false, |m| m.is_irreducible_on_earth()),
        }))
        .collect();
    let campaign_portfolio =
        optimize_campaign_portfolio(&campaign_reqs, &assets, inp.campaign_constraints, params.exact_check_max_assets);
    let residual_risk_register = build_risk_register(spec, &campaign_reqs, &assets, &campaign_portfolio);
    let campaign_next_actions = next_actions(&residual_risk_register, &campaign_reqs);

    // 8. Site-level decision: U.D.D. between the lead site and the best cell of a DIFFERENT
    //    site (two cells of one site are not a site choice), plus weight sensitivity.
    let mm_within: Vec<&RankedEntry> = mode_rankings[0].entries.iter().filter(|e| e.within_epsilon).collect();
    let lead = mm_within.first().copied();
    let runner = lead.and_then(|l| mm_within.iter().copied().find(|e| e.region != l.region));
    let dna_of = |e: &RankedEntry| build_analog_dna(&e.cell_id, &by_id[e.cell_id.as_str()].1);
    let lead_dna = lead.map(dna_of);
    let runner_dna = runner.map(dna_of);
    let shared_source = match (lead, runner) {
        (Some(a), Some(b)) => {
            let src = |e: &RankedEntry| by_id[e.cell_id.as_str()].0.provenance.as_ref().map(|p| p.source_id.clone());
            src(a).is_some() && src(a) == src(b)
        }
        _ => false,
    };
    let udd_recommendation = build_udd_recommendation(lead_dna.as_ref(), runner_dna.as_ref(), shared_source);
    let weight_sensitivity = weight_sensitivity(spec, &distances, eps, params.sensitivity_trials);
    let decision = decide(lead, runner, &udd_recommendation, &weight_sensitivity, &campaign_portfolio, &campaign_next_actions);

    // 9. Synthetic-content disclosure (technical spec v4, §13).
    let mut synthetic_content_warnings = Vec::new();
    if inp.manifest.is_synthetic_demo {
        synthetic_content_warnings.push("Run flagged as synthetic demo: results test the logic, not scientific accuracy.".into());
    }
    let n_syn = inp.cells.iter().filter(|c| c.data_origin == DataOrigin::Synthetic).count();
    if n_syn > 0 {
        synthetic_content_warnings.push(format!("{} of {} Earth cells are synthetic.", n_syn, inp.cells.len()));
    }
    let n_unspec = inp.cells.iter().filter(|c| c.data_origin == DataOrigin::Unspecified).count();
    if n_unspec > 0 {
        synthetic_content_warnings.push(format!("{} Earth cells have unspecified data origin.", n_unspec));
    }
    if inp.evidence.iter().any(|e| {
        e.mechanisms.iter().any(|m| m.synthetic_placeholder)
            || e.validated_correction_models.iter().any(|m| m.synthetic_placeholder)
    }) {
        synthetic_content_warnings.push("Mechanism evidence / correction models include synthetic placeholders.".into());
    }
    if inp.labs.iter().any(|l| l.synthetic_placeholder) {
        synthetic_content_warnings.push("Laboratory capability catalog includes synthetic placeholders.".into());
    }
    match params.synthetic_evidence_policy {
        SyntheticEvidencePolicy::Strict if !synthetic_content_warnings.is_empty() => synthetic_content_warnings
            .push("Strict policy: synthetic content never yields Confirmed coverage; it stays Provisional.".into()),
        SyntheticEvidencePolicy::TreatAsConfirmedForLogicDemo => synthetic_content_warnings.push(
            "LOGIC DEMO MODE: synthetic content is counted as if it were real evidence. Confirmed figures are NOT scientific results.".into(),
        ),
        _ => {}
    }

    Ok(DiscoveryReport {
        manifest: inp.manifest,
        parameters: params.clone(),
        mission_spec: spec.clone(),
        target_id: target.id.clone(),
        consistency_warnings,
        envelope,
        scales,
        rejections,
        epsilon_cutoff_count,
        epsilon_sensitivity_counts,
        mode_rankings,
        dossiers,
        campaign_assets: assets,
        campaign_portfolio,
        residual_risk_register,
        campaign_next_actions,
        udd_recommendation,
        weight_sensitivity,
        decision,
        synthetic_content_warnings,
    })
}

// ---------------------------------------------------------------------------
// Decision helpers
// ---------------------------------------------------------------------------

fn margin_warnings(spec: &MissionExperimentSpec, c: &Cell, k: f64) -> Vec<String> {
    let mut w = Vec::new();
    for hc in &spec.hard_constraints {
        let v = c.features.get(hc.feature);
        let Some(s) = c.feature_uncertainty.get(&hc.feature) else { continue };
        for (bound, side) in [(hc.min_allowed, "min"), (hc.max_allowed, "max")] {
            if let Some(b) = bound {
                if (v - b).abs() < k * s {
                    w.push(format!(
                        "{}: {:?} = {:.3} is within {:.1}σ ({:.3}) of the {} bound {}; the pass is not confirmed by the data.",
                        hc.constraint_id, hc.feature, v, k, s, side, b
                    ));
                }
            }
        }
    }
    w
}

fn fmt_bound(b: Option<f64>) -> String {
    b.map(|v| format!("{}", (v * 1000.0).round() / 1000.0)).unwrap_or_else(|| "-".into())
}

const REPLACE_SYNTHETIC: &str = "Replace synthetic placeholder with independently verifiable data/evidence";

fn in_period(w: &assessment::TemporalAnalogWindow, c: &CampaignConstraints) -> bool {
    w.illumination_geometry_valid
        && c.period_end_utc.as_ref().map_or(true, |e| w.start_utc.as_str() < e.as_str())
        && c.period_start_utc.as_ref().map_or(true, |st| w.end_utc.as_str() > st.as_str())
}

fn field_asset(
    spec: &MissionExperimentSpec,
    d: &CandidateDossier,
    policy: SyntheticEvidencePolicy,
    windows: &[SiteWindow],
    constraints: &CampaignConstraints,
) -> CampaignAssetCapability {
    let strict = policy == SyntheticEvidencePolicy::Strict;
    let synthetic_data = d.data_origin == DataOrigin::Synthetic;
    let marginal = !d.constraint_margin_warnings.is_empty();
    let mut time_window = None;
    let mut coverage = Vec::new();
    for r in d.observable_match.breakdown.iter().filter(|r| r.within_range) {
        let max_u = spec.required_parameters.iter().find(|p| p.requirement_id == r.requirement_id).and_then(|p| p.max_uncertainty);
        let mut missing = Vec::new();
        match (r.sigma_value, max_u) {
            (None, _) => missing.push(format!("Measure the uncertainty of {:?} at {}", r.feature, d.cell_id)),
            (Some(s), Some(m)) if s > m => missing.push(format!("Reduce σ of {:?} at {} from {:.3} to <= {:.3}", r.feature, d.cell_id, s, m)),
            _ => {}
        }
        if marginal {
            missing.push(format!("Re-measure hard-constrained features at {} (marginal pass)", d.cell_id));
        }
        if strict && synthetic_data {
            missing.push(format!("{REPLACE_SYNTHETIC} (Earth features of {})", d.cell_id));
        }
        coverage.push(CoverageClaim {
            requirement_id: r.requirement_id.clone(),
            status: if missing.is_empty() { CoverageStatus::Confirmed } else { CoverageStatus::Provisional },
            basis: format!("{:?} = {:.3} {} within [{}, {}]", r.feature, r.observed_value, r.unit, fmt_bound(r.acceptable_min), fmt_bound(r.acceptable_max)),
            missing_for_confirmation: missing,
        });
    }
    for f in &d.aet.findings {
        let placeholder = if f.relies_on_synthetic_placeholder { " [SYNTHETIC placeholder evidence]" } else { "" };
        let mut claim = match f.status {
            TransferabilityStatus::Transferable | TransferabilityStatus::TransferableWithCorrection => Some((CoverageStatus::Confirmed, Vec::new())),
            TransferabilityStatus::PartialTransfer => Some((CoverageStatus::Provisional, vec![f.limitation_note.clone()])),
            TransferabilityStatus::Unknown if f.venue_check == VenueCheck::ReproducibleAtFieldSite && f.observable_match_satisfied => {
                Some((CoverageStatus::Provisional, vec![f.limitation_note.clone()]))
            }
            _ => None,
        };
        // Time-dependent mechanism: the field test needs a known valid window inside the campaign period.
        let needs_window = spec.mechanism(&f.mechanism_id).map_or(false, |m| m.requires_field_time_window);
        if needs_window && claim.is_some() {
            let site_windows: Vec<&SiteWindow> =
                windows.iter().filter(|w| w.region == d.region && w.mechanism_id == f.mechanism_id).collect();
            if site_windows.is_empty() {
                if let Some((_, m)) = claim.as_mut() {
                    m.push(format!("Time window in which '{}' occurs at {} is unknown", f.mechanism_id, d.region));
                }
            } else if let Some(w) = site_windows.iter().find(|w| in_period(&w.window, constraints)) {
                time_window = Some(w.window.clone());
            } else {
                claim = None; // the mechanism never occurs at this site during the campaign period
            }
        }
        if let Some((mut status, mut missing)) = claim {
            if marginal {
                missing.push(format!("Re-measure hard-constrained features at {}", d.cell_id));
            }
            if strict && (f.relies_on_synthetic_placeholder || synthetic_data) {
                missing.push(format!("{REPLACE_SYNTHETIC} (evidence for '{}' at {})", f.mechanism_id, d.cell_id));
            }
            if !missing.is_empty() {
                status = CoverageStatus::Provisional;
            }
            coverage.push(CoverageClaim {
                requirement_id: f.requirement_id.clone(),
                status,
                basis: format!("Field test possible; AET {:?}{}", f.status, placeholder),
                missing_for_confirmation: missing,
            });
        }
    }
    CampaignAssetCapability {
        asset_id: format!("FIELD:{}:{}", d.region, d.cell_id),
        kind: CampaignAssetKind::FieldAnalogSite { cell_id: d.cell_id.clone(), region: d.region.clone() },
        time_window,
        coverage,
        estimated_cost_units: None,
        duration_days: None,
        synthetic_placeholder: synthetic_data,
    }
}

fn lab_asset(spec: &MissionExperimentSpec, lab: &LabFacility, policy: SyntheticEvidencePolicy) -> CampaignAssetCapability {
    let strict = policy == SyntheticEvidencePolicy::Strict;
    let mut coverage = Vec::new();
    for r in &spec.required_parameters {
        if lab.reproduces_features.contains(&r.feature) {
            let missing: Vec<String> = if strict && lab.synthetic_placeholder {
                vec![format!("{REPLACE_SYNTHETIC} (capability of {})", lab.facility_id)]
            } else {
                vec![]
            };
            coverage.push(CoverageClaim {
                requirement_id: r.requirement_id.clone(),
                status: if missing.is_empty() { CoverageStatus::Confirmed } else { CoverageStatus::Provisional },
                basis: format!("Controlled condition: facility sets {:?}", r.feature),
                missing_for_confirmation: missing,
            });
        }
    }
    for tr in &spec.transfer_requirements {
        let lab_venue = spec.mechanism(&tr.required_mechanism_id).map_or(false, |m| m.venues.contains(&TestVenue::Laboratory));
        if !(lab_venue && lab.reproduces_mechanisms.contains(&tr.required_mechanism_id)) {
            continue;
        }
        let ev = lab.transfer_evidence.iter().find(|e| e.mechanism_id == tr.required_mechanism_id);
        let mut missing = Vec::new();
        match ev {
            None => missing.push(format!("Evidence that '{}' results from {} transfer to the target", tr.required_mechanism_id, lab.facility_id)),
            Some(e) if e.level < EvidenceLevel::Literature => missing.push(format!("Stronger than expert-judgement evidence for {}", lab.facility_id)),
            _ => {}
        }
        if tr.requires_validated_correction_model {
            missing.push(format!("Validated correction model for '{}' in laboratory conditions", tr.required_mechanism_id));
        }
        if strict && (lab.synthetic_placeholder || ev.map_or(false, |e| e.synthetic_placeholder)) {
            missing.push(format!("{REPLACE_SYNTHETIC} (capability/transfer evidence of {})", lab.facility_id));
        }
        coverage.push(CoverageClaim {
            requirement_id: tr.requirement_id.clone(),
            status: if missing.is_empty() { CoverageStatus::Confirmed } else { CoverageStatus::Provisional },
            basis: format!(
                "Laboratory reproduces '{}'{}",
                tr.required_mechanism_id,
                if ev.map_or(false, |e| e.synthetic_placeholder) || lab.synthetic_placeholder { " [SYNTHETIC placeholder]" } else { "" }
            ),
            missing_for_confirmation: missing,
        });
    }
    CampaignAssetCapability {
        asset_id: format!("LAB:{}", lab.facility_id),
        kind: CampaignAssetKind::LaboratoryFacility { facility_id: lab.facility_id.clone() },
        time_window: None,
        coverage,
        estimated_cost_units: lab.cost_units,
        duration_days: lab.duration_days,
        synthetic_placeholder: lab.synthetic_placeholder,
    }
}

fn build_risk_register(
    spec: &MissionExperimentSpec,
    reqs: &[CampaignRequirement],
    assets: &[CampaignAssetCapability],
    p: &CampaignPortfolio,
) -> Vec<ResidualRiskEntry> {
    let selected: Vec<&CampaignAssetCapability> = assets.iter().filter(|a| p.selected_assets.contains(&a.asset_id)).collect();
    reqs.iter()
        .map(|r| {
            let id = &r.requirement_id;
            let tr = spec.transfer_requirements.iter().find(|t| &t.requirement_id == id);
            let param = spec.required_parameters.iter().find(|q| &q.requirement_id == id);
            let status = if r.irreducible {
                RehearsalStatus::IrreducibleUntilFlight
            } else if p.confirmed_requirements.contains(id) {
                RehearsalStatus::ConfirmedBeforeFlight
            } else if p.provisional_requirements.contains(id) {
                RehearsalStatus::ProvisionalBeforeFlight
            } else if p.uncovered_requirements.contains(id) {
                RehearsalStatus::NotRehearsedInPlan
            } else {
                RehearsalStatus::NoTerrestrialAsset
            };
            let claims: Vec<(&str, &CoverageClaim)> = selected
                .iter()
                .flat_map(|a| a.coverage.iter().filter(|c| &c.requirement_id == id).map(move |c| (a.asset_id.as_str(), c)))
                .collect();
            let best = claims.iter().map(|(_, c)| c.status).max();
            let mut missing: Vec<String> = if status == RehearsalStatus::ProvisionalBeforeFlight {
                claims.iter().filter(|(_, c)| Some(c.status) == best).flat_map(|(_, c)| c.missing_for_confirmation.clone()).collect()
            } else {
                Vec::new()
            };
            missing.sort();
            missing.dedup();
            ResidualRiskEntry {
                requirement_id: id.clone(),
                is_transfer_requirement: tr.is_some(),
                engineering_decision: tr
                    .map(|t| t.engineering_decision.clone())
                    .or_else(|| param.map(|q| format!("{:?} within [{}, {}] {}", q.feature, fmt_bound(q.acceptable_min), fmt_bound(q.acceptable_max), q.unit)))
                    .unwrap_or_default(),
                criticality: r.criticality,
                mechanism_id: tr.map(|t| t.required_mechanism_id.clone()),
                status,
                rehearsed_by: claims.iter().filter(|(_, c)| Some(c.status) == best).map(|(a, _)| a.to_string()).collect(),
                missing_for_confirmation: missing,
            }
        })
        .collect()
}

fn next_actions(register: &[ResidualRiskEntry], reqs: &[CampaignRequirement]) -> Vec<NextAction> {
    let weight: BTreeMap<&str, f64> = reqs.iter().map(|r| (r.requirement_id.as_str(), r.weight)).collect();
    let mut rows: Vec<(&ResidualRiskEntry, String)> = Vec::new();
    for e in register {
        match e.status {
            RehearsalStatus::ProvisionalBeforeFlight => {
                for m in &e.missing_for_confirmation {
                    rows.push((e, m.clone()));
                }
            }
            RehearsalStatus::NotRehearsedInPlan => rows.push((e, "Relax campaign constraints or add an asset: a capable asset exists but was not selected.".into())),
            RehearsalStatus::NoTerrestrialAsset => rows.push((e, "Find or build a field site / facility able to test this requirement.".into())),
            RehearsalStatus::IrreducibleUntilFlight => rows.push((e, "Cannot be rehearsed on Earth: carry as flight risk (design margin or in-situ verification).".into())),
            RehearsalStatus::ConfirmedBeforeFlight => {}
        }
    }
    rows.sort_by(|a, b| {
        a.0.criticality
            .cmp(&b.0.criticality)
            .then((b.0.status == RehearsalStatus::IrreducibleUntilFlight).cmp(&(a.0.status == RehearsalStatus::IrreducibleUntilFlight)).reverse())
            .then(weight[b.0.requirement_id.as_str()].total_cmp(&weight[a.0.requirement_id.as_str()]))
            .then(a.0.requirement_id.cmp(&b.0.requirement_id))
            .then(a.1.cmp(&b.1))
    });
    rows.into_iter()
        .map(|(e, a)| NextAction { requirement_id: e.requirement_id.clone(), criticality: e.criticality, action: a })
        .collect()
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Re-rank feasible cells with each requirement weight multiplied by a factor in [0.5, 2]
/// (log-uniform, deterministic seeds) and count how often the lead site (region) survives.
pub fn weight_sensitivity(spec: &MissionExperimentSpec, feasible: &[(&Cell, MissionDistance)], eps: f64, trials: usize) -> WeightSensitivity {
    let lead_of = |s: &MissionExperimentSpec| -> Option<String> {
        feasible
            .iter()
            .map(|(c, _)| (c, mission_distance(s, &c.features, &c.feature_uncertainty).d))
            .filter(|(_, d)| *d <= eps)
            .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.id.cmp(&b.0.id)))
            .map(|(c, _)| c.region.clone())
    };
    let base = lead_of(spec);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut same = 0usize;
    for t in 0..trials {
        let mut s = spec.clone();
        for (i, r) in s.required_parameters.iter_mut().enumerate() {
            let u = (splitmix64(0x5EED ^ ((t as u64) << 16) ^ i as u64) >> 11) as f64 / (1u64 << 53) as f64;
            r.weight *= 2f64.powf(2.0 * u - 1.0);
        }
        if let Some(l) = lead_of(&s) {
            if Some(&l) == base.as_ref() {
                same += 1;
            }
            *counts.entry(l).or_insert(0) += 1;
        }
    }
    WeightSensitivity {
        trials,
        perturbation: "each requirement weight × 2^U(-1,1), independent per trial".into(),
        lead_site_stability: (trials > 0 && base.is_some()).then(|| same as f64 / trials as f64),
        leaders: counts.into_iter().map(|(k, v)| (k, v as f64 / trials.max(1) as f64)).collect(),
    }
}

fn decide(
    lead: Option<&RankedEntry>,
    runner: Option<&RankedEntry>,
    udd: &UncertaintyRecommendation,
    ws: &WeightSensitivity,
    p: &CampaignPortfolio,
    actions: &[NextAction],
) -> DecisionSummary {
    let sites = || -> Vec<String> { lead.into_iter().chain(runner).map(|e| e.region.clone()).collect() };
    let lead_site = match lead {
        None => LeadSiteVerdict::NoCandidate,
        Some(l) => match &udd.flip {
            FlipAssessment::CanFlip { .. } => LeadSiteVerdict::TiedWithinUncertainty { sites: sites() },
            FlipAssessment::UncertaintyUnknown { .. } => LeadSiteVerdict::UndecidedUncertaintyUnknown { sites: sites() },
            _ if ws.lead_site_stability.map_or(false, |s| s < 0.8) => LeadSiteVerdict::WeightSensitive {
                leaders: ws.leaders.iter().filter(|(_, v)| **v >= 0.1).map(|(k, _)| k.clone()).collect(),
            },
            _ => LeadSiteVerdict::Resolved { site: l.region.clone(), cell_id: l.cell_id.clone() },
        },
    };
    let lead_txt = match &lead_site {
        LeadSiteVerdict::NoCandidate => "No feasible field site.".to_string(),
        LeadSiteVerdict::Resolved { site, .. } => format!("Lead field site: {site} (robust to uncertainty and weights)."),
        LeadSiteVerdict::TiedWithinUncertainty { sites } => format!("Lead field site NOT resolved: {} are equivalent within measurement uncertainty.", sites.join(" vs ")),
        LeadSiteVerdict::UndecidedUncertaintyUnknown { sites } => format!("Lead field site undecided between {}: a decisive uncertainty is unknown.", sites.join(" and ")),
        LeadSiteVerdict::WeightSensitive { leaders } => format!("Lead field site depends on requirement weights: {}.", leaders.join(", ")),
    };
    let statement = format!(
        "{} Rehearsal campaign: {}. Before flight {:.0}% of reducible requirements are confirmed, {:.0}% testable. Mission-critical not confirmed: {}. First action: {}",
        lead_txt,
        if p.selected_assets.is_empty() { "none".to_string() } else { p.selected_assets.join(" + ") },
        p.confirmed_coverage * 100.0,
        p.testable_coverage * 100.0,
        if p.critical_unconfirmed.is_empty() { "none".to_string() } else { p.critical_unconfirmed.join(", ") },
        actions.first().map(|a| format!("{} — {}", a.requirement_id, a.action)).unwrap_or_else(|| "none".into())
    );
    // First action: open reducible risks (critical first) > resolving the lead-site choice > carrying irreducible risk.
    let site_action = udd.recommended_measurement.as_ref().filter(|_| !matches!(lead_site, LeadSiteVerdict::Resolved { .. } | LeadSiteVerdict::NoCandidate)).map(|m| NextAction {
        requirement_id: m.clone(),
        criticality: Criticality::High,
        action: format!("Resolve the lead-site choice: {}", udd.rationale),
    });
    let first_action = actions
        .iter()
        .find(|a| !a.action.starts_with("Cannot be rehearsed"))
        .cloned()
        .or(site_action)
        .or_else(|| actions.first().cloned());
    let statement = statement.replace(
        &format!("First action: {}", actions.first().map(|a| format!("{} — {}", a.requirement_id, a.action)).unwrap_or_else(|| "none".into())),
        &format!("First action: {}", first_action.as_ref().map(|a| format!("{} — {}", a.requirement_id, a.action)).unwrap_or_else(|| "none".into())),
    );
    DecisionSummary {
        lead_site,
        campaign_assets: p.selected_assets.clone(),
        confirmed_coverage: p.confirmed_coverage,
        testable_coverage: p.testable_coverage,
        mission_critical_unconfirmed: p.critical_unconfirmed.clone(),
        irreducible_until_flight: p.irreducible_requirements.clone(),
        first_action,
        statement,
    }
}

// ---------------------------------------------------------------------------
// Multi-target (e.g. Artemis III regions) Earth-testability assessment
// ---------------------------------------------------------------------------

/// Same experiment for another lunar region: target values come from the region's profile,
/// acceptable-range widths and everything else stay as defined by the mission.
pub fn spec_for_target(base: &MissionExperimentSpec, t: &TargetProfile) -> MissionExperimentSpec {
    let mut s = base.clone();
    s.target_profile_id = Some(t.id.clone());
    s.target_region = t.region_name.clone();
    for r in &mut s.required_parameters {
        let new_t = t.features.get(r.feature);
        let (lo_phys, hi_phys) = match r.feature.family() {
            FeatureFamily::TerrainSlope => (0.0, 90.0),
            FeatureFamily::TerrainRoughness => (0.0, f64::INFINITY),
            FeatureFamily::IlluminationShadow => (0.0, 1.0),
        };
        r.acceptable_min = r.acceptable_min.map(|m| (new_t - (r.target_value - m)).clamp(lo_phys, new_t));
        r.acceptable_max = r.acceptable_max.map(|m| (new_t + (m - r.target_value)).clamp(new_t, hi_phys));
        r.target_value = new_t;
    }
    s
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetRehearsalSummary {
    pub target_id: String,
    pub region_name: String,
    pub joint_status: envelope::JointReachStatus,
    pub lead_site: LeadSiteVerdict,
    pub campaign_assets: Vec<String>,
    pub confirmed_coverage: f64,
    pub testable_coverage: f64,
    /// Mission-critical, reducible, but not confirmed by the best campaign.
    pub critical_reducible_unconfirmed: Vec<String>,
    pub irreducible_until_flight: Vec<String>,
    pub first_action: Option<NextAction>,
}

pub fn summarize_target(r: &DiscoveryReport) -> TargetRehearsalSummary {
    let p = &r.campaign_portfolio;
    TargetRehearsalSummary {
        target_id: r.target_id.clone(),
        region_name: r.mission_spec.target_region.clone(),
        joint_status: r.envelope.joint_status.clone(),
        lead_site: r.decision.lead_site.clone(),
        campaign_assets: p.selected_assets.clone(),
        confirmed_coverage: p.confirmed_coverage,
        testable_coverage: p.testable_coverage,
        critical_reducible_unconfirmed: p.critical_unconfirmed.iter().filter(|c| !p.irreducible_requirements.contains(c)).cloned().collect(),
        irreducible_until_flight: p.irreducible_requirements.clone(),
        first_action: r.decision.first_action.clone(),
    }
}

/// Order regions by pre-flight residual risk for THIS experiment (not a safety ranking):
/// fewer unconfirmed mission-critical requirements, then fewer irreducible ones, then
/// higher confirmed coverage, then higher testable coverage. No single blended score.
pub fn order_targets_by_residual_risk(v: &mut [TargetRehearsalSummary]) {
    v.sort_by(|a, b| {
        a.critical_reducible_unconfirmed
            .len()
            .cmp(&b.critical_reducible_unconfirmed.len())
            .then(a.irreducible_until_flight.len().cmp(&b.irreducible_until_flight.len()))
            .then(b.confirmed_coverage.total_cmp(&a.confirmed_coverage))
            .then(b.testable_coverage.total_cmp(&a.testable_coverage))
            .then(a.target_id.cmp(&b.target_id))
    });
}
