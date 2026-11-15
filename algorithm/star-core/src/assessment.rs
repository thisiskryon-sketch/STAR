// star-core/src/assessment.rs
//! AET (Analog-to-Experiment Transfer), E.C.O. (Experimental Coverage Optimization)
//! and U.D.D. (Uncertainty-Driven Discovery).
use crate::dna::AnalogDna;
use crate::schema::{Cell, Criticality, FeatureId, MissionExperimentSpec, TestVenue};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// Evidence
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EvidenceLevel {
    None,
    ExpertJudgement,
    Literature,
    LabValidated,
    FieldValidated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanismEvidence {
    pub mechanism_id: String,
    pub level: EvidenceLevel,
    pub source: String,
    /// True for demo placeholders: never presentable as real evidence.
    #[serde(default)]
    pub synthetic_placeholder: bool,
    /// Parameter domain in which the evidence was obtained. Empty = domain not stated.
    #[serde(default)]
    pub valid_feature_bounds: BTreeMap<FeatureId, (f64, f64)>,
    /// Confirmed by a source independent of the one that proposed it.
    #[serde(default)]
    pub independent_validation: bool,
}

/// Does a piece of evidence apply to this particular site / test condition?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceApplicability {
    NoEvidence,
    /// Evidence exists, but the parameter domain it covers is not stated.
    DomainNotStated,
    OutsideValidatedDomain,
    Applicable,
}

pub fn evidence_applicability(ev: Option<&MechanismEvidence>, cell: &Cell) -> EvidenceApplicability {
    match ev {
        None => EvidenceApplicability::NoEvidence,
        Some(e) if e.valid_feature_bounds.is_empty() => EvidenceApplicability::DomainNotStated,
        Some(e) => {
            let inside = e.valid_feature_bounds.iter().all(|(&f, &(lo, hi))| {
                let v = cell.features.get(f);
                v >= lo && v <= hi
            });
            if inside { EvidenceApplicability::Applicable } else { EvidenceApplicability::OutsideValidatedDomain }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedCorrectionModel {
    pub model_id: String,
    pub mechanism_id: String,
    pub citation_or_source: String,
    pub evidence_level: EvidenceLevel,
    /// Validity domain. A model with an empty domain is not applicable anywhere.
    pub valid_feature_bounds: BTreeMap<FeatureId, (f64, f64)>,
    pub residual_relative_uncertainty: f64,
    #[serde(default)]
    pub synthetic_placeholder: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteMechanismEvidence {
    pub cell_id: String,
    pub mechanisms: Vec<MechanismEvidence>,
    pub validated_correction_models: Vec<ValidatedCorrectionModel>,
}

// ---------------------------------------------------------------------------
// AET
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferabilityStatus {
    Transferable,
    TransferableWithCorrection,
    PartialTransfer,
    NotTransferable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VenueCheck {
    ReproducibleAtFieldSite,
    LaboratoryOnly,
    IrreducibleOnEarth,
    OutsideFieldLatitudeBand { required_min_abs_lat: f64, site_abs_lat: f64 },
    MechanismNotInRegistry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AetFinding {
    pub requirement_id: String,
    pub engineering_decision: String,
    pub mechanism_id: String,
    pub venue_check: VenueCheck,
    pub observable_match_satisfied: bool,
    pub evidence_level: EvidenceLevel,
    pub evidence_applicability: EvidenceApplicability,
    pub applied_correction_model: Option<String>,
    pub residual_relative_uncertainty: Option<f64>,
    pub status: TransferabilityStatus,
    pub limitation_note: String,
    pub relies_on_synthetic_placeholder: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AetAssessment {
    pub cell_id: String,
    pub experiment_id: String,
    pub overall_status: TransferabilityStatus,
    pub transferable_requirements: Vec<String>,
    pub not_transferable_requirements: Vec<String>,
    pub unknown_requirements: Vec<String>,
    pub findings: Vec<AetFinding>,
    pub transfer_limits: Vec<String>,
}

/// AET for one (experiment, field site, evidence) triple. The order of checks is
/// physics first (can the mechanism exist here at all?), then observables, then evidence.
pub fn evaluate_aet_transferability(
    spec: &MissionExperimentSpec,
    cell: &Cell,
    dna: &AnalogDna,
    evidence: Option<&SiteMechanismEvidence>,
) -> AetAssessment {
    let mut findings = Vec::new();
    let mut transfer_limits = Vec::new();
    let in_range: BTreeMap<&str, bool> = dna
        .breakdown
        .iter()
        .map(|r| (r.requirement_id.as_str(), r.within_range))
        .collect();
    for r in dna.breakdown.iter().filter(|r| !r.within_range) {
        transfer_limits.push(format!(
            "{} = {:.3} {} outside acceptable range [{:?}, {:?}]",
            r.requirement_id, r.observed_value, r.unit, r.acceptable_min, r.acceptable_max
        ));
    }

    for tr in &spec.transfer_requirements {
        let mut f = AetFinding {
            requirement_id: tr.requirement_id.clone(),
            engineering_decision: tr.engineering_decision.clone(),
            mechanism_id: tr.required_mechanism_id.clone(),
            venue_check: VenueCheck::MechanismNotInRegistry,
            observable_match_satisfied: false,
            evidence_level: EvidenceLevel::None,
            evidence_applicability: EvidenceApplicability::NoEvidence,
            applied_correction_model: None,
            residual_relative_uncertainty: None,
            status: TransferabilityStatus::Unknown,
            limitation_note: String::new(),
            relies_on_synthetic_placeholder: false,
        };
        let deps: Vec<&str> = if tr.depends_on_parameters.is_empty() {
            in_range.keys().copied().collect()
        } else {
            tr.depends_on_parameters.iter().map(|s| s.as_str()).collect()
        };
        f.observable_match_satisfied = deps.iter().all(|d| in_range.get(d).copied().unwrap_or(false));

        // 1. Mechanism Envelope: can this physics happen at a field site at all?
        let Some(mech) = spec.mechanism(&tr.required_mechanism_id) else {
            f.limitation_note = "Mechanism is not described in the registry; transferability is Unknown.".into();
            findings.push(f);
            continue;
        };
        if mech.is_irreducible_on_earth() {
            f.venue_check = VenueCheck::IrreducibleOnEarth;
            f.status = TransferabilityStatus::NotTransferable;
            f.limitation_note = format!(
                "'{}' cannot be reproduced on Earth in a sustained way: irreducible gap of any terrestrial campaign.",
                mech.mechanism_id
            );
            findings.push(f);
            continue;
        }
        if !mech.venues.contains(&TestVenue::FieldSite) {
            f.venue_check = VenueCheck::LaboratoryOnly;
            f.status = TransferabilityStatus::NotTransferable;
            f.limitation_note = format!(
                "'{}' is reproducible only in a laboratory; no field site can provide it.",
                mech.mechanism_id
            );
            findings.push(f);
            continue;
        }
        if let Some(min_lat) = mech.field_min_abs_latitude_deg {
            if cell.lat.abs() < min_lat {
                f.venue_check = VenueCheck::OutsideFieldLatitudeBand {
                    required_min_abs_lat: min_lat,
                    site_abs_lat: cell.lat.abs(),
                };
                f.status = TransferabilityStatus::NotTransferable;
                f.limitation_note = format!(
                    "Field condition for '{}' requires |lat| >= {:.1}°, site is at {:.1}°.",
                    mech.mechanism_id, min_lat, cell.lat.abs()
                );
                findings.push(f);
                continue;
            }
        }
        f.venue_check = VenueCheck::ReproducibleAtFieldSite;

        // 2. Observables.
        if !f.observable_match_satisfied {
            f.status = TransferabilityStatus::NotTransferable;
            f.limitation_note = "Required observable conditions are outside the acceptable range at this site.".into();
            findings.push(f);
            continue;
        }

        // 3. Evidence.
        let ev = evidence.and_then(|e| e.mechanisms.iter().find(|m| m.mechanism_id == tr.required_mechanism_id));
        let Some(ev) = ev else {
            f.limitation_note = "No mechanism evidence for this site; status defaults to Unknown.".into();
            findings.push(f);
            continue;
        };
        f.evidence_level = ev.level;
        f.relies_on_synthetic_placeholder = ev.synthetic_placeholder;
        f.evidence_applicability = evidence_applicability(Some(ev), cell);
        if f.evidence_applicability == EvidenceApplicability::OutsideValidatedDomain {
            f.limitation_note = "Evidence exists, but this site lies outside the parameter domain it was obtained in.".into();
            findings.push(f);
            continue;
        }
        if ev.level < EvidenceLevel::Literature || f.evidence_applicability == EvidenceApplicability::DomainNotStated {
            f.status = TransferabilityStatus::PartialTransfer;
            f.limitation_note = if ev.level < EvidenceLevel::Literature {
                "Mechanism supported only by expert judgement; results are indicative.".into()
            } else {
                "Evidence does not state its validity domain; applicability to this site is unproven.".into()
            };
            findings.push(f);
            continue;
        }
        if !tr.requires_validated_correction_model {
            f.status = TransferabilityStatus::Transferable;
            f.limitation_note = format!("Mechanism evidenced ({:?}: {}).", ev.level, ev.source);
            findings.push(f);
            continue;
        }
        let model = evidence.and_then(|e| {
            e.validated_correction_models.iter().find(|m| {
                m.mechanism_id == tr.required_mechanism_id
                    && m.evidence_level >= EvidenceLevel::Literature
                    && m.residual_relative_uncertainty.is_finite()
                    && m.residual_relative_uncertainty >= 0.0
                    && !m.valid_feature_bounds.is_empty()
                    && m.valid_feature_bounds.iter().all(|(&feat, &(lo, hi))| {
                        let v = cell.features.get(feat);
                        v >= lo && v <= hi
                    })
            })
        });
        match model {
            Some(m) => {
                f.status = TransferabilityStatus::TransferableWithCorrection;
                f.applied_correction_model = Some(m.model_id.clone());
                f.residual_relative_uncertainty = Some(m.residual_relative_uncertainty);
                f.relies_on_synthetic_placeholder |= m.synthetic_placeholder;
                f.limitation_note = format!(
                    "Correction model '{}' valid at this site (residual {:.0}%).",
                    m.model_id,
                    m.residual_relative_uncertainty * 100.0
                );
            }
            None => {
                f.limitation_note =
                    "Correction required but no validated model covers this site; no coefficient is invented.".into();
            }
        }
        findings.push(f);
    }

    let pick = |st: &[TransferabilityStatus]| -> Vec<String> {
        findings.iter().filter(|f| st.contains(&f.status)).map(|f| f.requirement_id.clone()).collect()
    };
    let transferable = pick(&[TransferabilityStatus::Transferable, TransferabilityStatus::TransferableWithCorrection]);
    let not_tr = pick(&[TransferabilityStatus::NotTransferable]);
    let unknown = pick(&[TransferabilityStatus::Unknown]);
    let partial = pick(&[TransferabilityStatus::PartialTransfer]);
    let overall = if findings.is_empty() {
        TransferabilityStatus::Unknown
    } else if findings.iter().all(|f| f.status == TransferabilityStatus::Transferable) {
        TransferabilityStatus::Transferable
    } else if transferable.len() == findings.len() {
        TransferabilityStatus::TransferableWithCorrection
    } else if !transferable.is_empty() || !partial.is_empty() {
        TransferabilityStatus::PartialTransfer
    } else if unknown.is_empty() {
        TransferabilityStatus::NotTransferable
    } else {
        TransferabilityStatus::Unknown
    };

    AetAssessment {
        cell_id: cell.id.clone(),
        experiment_id: spec.experiment_id.clone(),
        overall_status: overall,
        transferable_requirements: transferable,
        not_transferable_requirements: not_tr,
        unknown_requirements: unknown,
        findings,
        transfer_limits,
    }
}

// ---------------------------------------------------------------------------
// E.C.O. — rehearsal campaign: testability and transferability kept apart
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalAnalogWindow {
    pub window_id: String,
    pub start_utc: String,
    pub end_utc: String,
    pub season_or_solar_regime: String,
    pub illumination_geometry_valid: bool,
}

/// Laboratory capability catalog entry (external input; demo entries are placeholders).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabFacility {
    pub facility_id: String,
    pub name: String,
    pub reproduces_mechanisms: Vec<String>,
    /// Features the facility can set to the required value (e.g. a tilt-bed soil bin sets slope).
    #[serde(default)]
    pub reproduces_features: Vec<FeatureId>,
    pub cost_units: Option<f64>,
    pub duration_days: Option<f64>,
    #[serde(default)]
    pub synthetic_placeholder: bool,
    /// Evidence that results obtained in this facility transfer to the target environment.
    /// Reproducing a mechanism is testability; this is transferability.
    #[serde(default)]
    pub transfer_evidence: Vec<MechanismEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CampaignAssetKind {
    FieldAnalogSite { cell_id: String, region: String },
    LaboratoryFacility { facility_id: String },
}

/// Testability vs confirmed transferability of one requirement by one asset.
/// Ordering: Provisional < Confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CoverageStatus {
    /// The test can be run, but the value's uncertainty or the transfer of its result is unproven.
    Provisional,
    /// Testable, uncertainty within limits and (for transfer requirements) evidence-supported transfer.
    Confirmed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageClaim {
    pub requirement_id: String,
    pub status: CoverageStatus,
    pub basis: String,
    /// What would upgrade a Provisional claim to Confirmed.
    #[serde(default)]
    pub missing_for_confirmation: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignAssetCapability {
    pub asset_id: String,
    pub kind: CampaignAssetKind,
    pub time_window: Option<TemporalAnalogWindow>,
    pub coverage: Vec<CoverageClaim>,
    pub estimated_cost_units: Option<f64>,
    pub duration_days: Option<f64>,
    pub synthetic_placeholder: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignRequirement {
    pub requirement_id: String,
    pub weight: f64,
    pub criticality: Criticality,
    /// True if its mechanism cannot be reproduced anywhere on Earth.
    pub irreducible: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CampaignConstraints {
    pub max_budget_units: Option<f64>,
    pub max_total_duration_days: Option<f64>,
    pub max_assets: Option<usize>,
    /// Campaign period, RFC3339 UTC strings (compared lexicographically). None = unconstrained.
    #[serde(default)]
    pub period_start_utc: Option<String>,
    #[serde(default)]
    pub period_end_utc: Option<String>,
}

/// Known time window in which a field site offers a time-dependent mechanism (input data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteWindow {
    /// Matches a cell's region (site) name.
    pub region: String,
    pub mechanism_id: String,
    pub window: TemporalAnalogWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortfolioOptimizationStatus {
    /// Every reducible requirement confirmed.
    AllReducibleConfirmed,
    /// Every reducible requirement at least testable, some only provisionally.
    AllReducibleTestableSomeProvisional,
    PartialCoverageUnderConstraints,
    PartialCoverageNoCapableAsset,
    PortfolioNotOptimizedNoCapabilityData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CostModel {
    KnownCosts,
    /// Some costs are unknown: they are not invented; the optimiser minimises the number of assets.
    UnknownCostsMinimisingAssetCount,
}

/// Feasibility of a resource constraint for the selected portfolio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstraintCheck {
    NotConstrained,
    Satisfied,
    /// Some selected assets have unknown values: compliance cannot be confirmed.
    Unverifiable { assets_with_unknown_values: Vec<String> },
}

/// Lexicographic objective: critical confirmed > critical provisional > confirmed > provisional.
/// Critical requirements can never be traded for more lower-class coverage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct CoverageObjective {
    pub critical_confirmed: f64,
    pub critical_provisional: f64,
    pub confirmed: f64,
    pub provisional: f64,
}

impl CoverageObjective {
    fn key(&self) -> [f64; 4] {
        [self.critical_confirmed, self.critical_provisional, self.confirmed, self.provisional]
    }
    fn sub(&self, o: &Self) -> Self {
        Self {
            critical_confirmed: self.critical_confirmed - o.critical_confirmed,
            critical_provisional: self.critical_provisional - o.critical_provisional,
            confirmed: self.confirmed - o.confirmed,
            provisional: self.provisional - o.provisional,
        }
    }
    fn scale(&self, k: f64) -> Self {
        Self {
            critical_confirmed: self.critical_confirmed * k,
            critical_provisional: self.critical_provisional * k,
            confirmed: self.confirmed * k,
            provisional: self.provisional * k,
        }
    }
    /// -1 / 0 / 1 lexicographic comparison with tolerance.
    fn cmp_lex(&self, o: &Self) -> std::cmp::Ordering {
        for (a, b) in self.key().iter().zip(o.key()) {
            if (a - b).abs() > 1e-9 {
                return a.total_cmp(&b);
            }
        }
        std::cmp::Ordering::Equal
    }
    fn is_positive(&self) -> bool {
        self.cmp_lex(&Self::default()) == std::cmp::Ordering::Greater
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionStep {
    pub asset_id: String,
    pub newly_confirmed: Vec<String>,
    pub newly_provisional: Vec<String>,
    pub upgraded_to_confirmed: Vec<String>,
    pub cost_units: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineComparison {
    pub asset_id: String,
    pub confirmed_coverage: f64,
    pub testable_coverage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativePortfolio {
    pub without_asset: String,
    pub selected_assets: Vec<String>,
    pub confirmed_coverage: f64,
    pub testable_coverage: f64,
    /// Mission-critical requirements confirmed in the main plan but not in this one.
    pub critical_confirmations_lost: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExactCheck {
    pub assets_considered: usize,
    pub best_selected_assets: Vec<String>,
    pub best_objective: CoverageObjective,
    pub greedy_objective: CoverageObjective,
    /// Same objective and no more assets than the exact optimum.
    pub greedy_is_optimal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignPortfolio {
    pub status: PortfolioOptimizationStatus,
    pub algorithm: String,
    pub cost_model: CostModel,
    pub selected_assets: Vec<String>,
    pub selection_steps: Vec<SelectionStep>,
    pub confirmed_requirements: Vec<String>,
    /// Testable but not confirmed (provisional only).
    pub provisional_requirements: Vec<String>,
    /// Some asset could test it but none was selected under the constraints.
    pub uncovered_requirements: Vec<String>,
    /// Reducible in principle, but no available asset can test it.
    pub no_capable_asset_requirements: Vec<String>,
    pub irreducible_requirements: Vec<String>,
    /// Mission-critical requirements (incl. irreducible) that are not confirmed before flight.
    pub critical_unconfirmed: Vec<String>,
    /// Weight share of reducible requirements that are confirmed.
    pub confirmed_coverage: f64,
    /// Weight share of reducible requirements that are at least testable.
    pub testable_coverage: f64,
    pub objective: CoverageObjective,
    pub total_cost_units: Option<f64>,
    pub total_duration_days: Option<f64>,
    pub budget_check: ConstraintCheck,
    pub duration_check: ConstraintCheck,
    pub single_best_site_baseline: Option<BaselineComparison>,
    pub alternatives: Vec<AlternativePortfolio>,
    pub exact_check: Option<ExactCheck>,
}

type State<'a> = BTreeMap<&'a str, CoverageStatus>;

fn apply<'a>(state: &mut State<'a>, asset: &'a CampaignAssetCapability, reducible: &BTreeMap<&str, &CampaignRequirement>) {
    for c in &asset.coverage {
        if reducible.contains_key(c.requirement_id.as_str()) {
            let e = state.entry(c.requirement_id.as_str()).or_insert(c.status);
            if c.status > *e {
                *e = c.status;
            }
        }
    }
}

fn objective(state: &State, reducible: &BTreeMap<&str, &CampaignRequirement>) -> CoverageObjective {
    let mut o = CoverageObjective::default();
    for (id, st) in state {
        let r = reducible[id];
        let crit = r.criticality == Criticality::MissionCritical;
        match st {
            CoverageStatus::Confirmed => {
                o.confirmed += r.weight;
                if crit {
                    o.critical_confirmed += r.weight;
                }
            }
            CoverageStatus::Provisional => {
                o.provisional += r.weight;
                if crit {
                    o.critical_provisional += r.weight;
                }
            }
        }
    }
    o
}

fn state_of<'a>(sel: &[usize], assets: &'a [CampaignAssetCapability], reducible: &BTreeMap<&str, &CampaignRequirement>) -> State<'a> {
    let mut st = State::new();
    for &i in sel {
        apply(&mut st, &assets[i], reducible);
    }
    st
}

fn within_constraints(sel: &[usize], assets: &[CampaignAssetCapability], c: &CampaignConstraints) -> bool {
    if c.max_assets.map_or(false, |m| sel.len() > m) {
        return false;
    }
    let cost: f64 = sel.iter().filter_map(|&i| assets[i].estimated_cost_units).sum();
    let days: f64 = sel.iter().filter_map(|&i| assets[i].duration_days).sum();
    c.max_budget_units.map_or(true, |b| cost <= b + 1e-9) && c.max_total_duration_days.map_or(true, |d| days <= d + 1e-9)
}

fn greedy(
    reducible: &BTreeMap<&str, &CampaignRequirement>,
    assets: &[CampaignAssetCapability],
    c: &CampaignConstraints,
    known_costs: bool,
    excluded: Option<usize>,
) -> Vec<usize> {
    let mut sel: Vec<usize> = Vec::new();
    loop {
        let st = state_of(&sel, assets, reducible);
        let base = objective(&st, reducible);
        let mut best: Option<(usize, CoverageObjective)> = None;
        for i in 0..assets.len() {
            if sel.contains(&i) || Some(i) == excluded {
                continue;
            }
            let mut trial = sel.clone();
            trial.push(i);
            if !within_constraints(&trial, assets, c) {
                continue;
            }
            let gain = objective(&state_of(&trial, assets, reducible), reducible).sub(&base);
            if !gain.is_positive() {
                continue;
            }
            let denom = if known_costs { assets[i].estimated_cost_units.unwrap_or(1.0).max(1e-9) } else { 1.0 };
            let eff = gain.scale(1.0 / denom);
            // Tie-break: fewer open items before confirmation (closer to Confirmed), then asset_id.
            let open_items = |a: &CampaignAssetCapability| -> usize {
                a.coverage.iter().filter(|c| reducible.contains_key(c.requirement_id.as_str())).map(|c| c.missing_for_confirmation.len()).sum()
            };
            let better = match &best {
                None => true,
                Some((bi, be)) => match eff.cmp_lex(be) {
                    std::cmp::Ordering::Greater => true,
                    std::cmp::Ordering::Equal => {
                        let (oi, ob) = (open_items(&assets[i]), open_items(&assets[*bi]));
                        oi < ob || (oi == ob && assets[i].asset_id < assets[*bi].asset_id)
                    }
                    std::cmp::Ordering::Less => false,
                },
            };
            if better {
                best = Some((i, eff));
            }
        }
        match best {
            Some((i, _)) => sel.push(i),
            None => break,
        }
    }
    sel
}

fn constraint_check(sel: &[usize], assets: &[CampaignAssetCapability], limit: Option<f64>, value: impl Fn(&CampaignAssetCapability) -> Option<f64>) -> ConstraintCheck {
    if limit.is_none() {
        return ConstraintCheck::NotConstrained;
    }
    let unknown: Vec<String> = sel.iter().filter(|&&i| value(&assets[i]).is_none()).map(|&i| assets[i].asset_id.clone()).collect();
    if unknown.is_empty() {
        ConstraintCheck::Satisfied
    } else {
        ConstraintCheck::Unverifiable { assets_with_unknown_values: unknown }
    }
}

/// Rehearsal campaign optimiser: greedy constrained portfolio heuristic with a
/// lexicographic objective (critical confirmed > critical provisional > confirmed >
/// provisional) per unit cost; ties broken by asset_id. No approximation guarantee is
/// claimed under budget/duration/count constraints; an exact enumeration is run as a
/// check when the asset set is small.
pub fn optimize_campaign_portfolio(
    reqs: &[CampaignRequirement],
    assets: &[CampaignAssetCapability],
    constraints: &CampaignConstraints,
    exact_check_max_assets: usize,
) -> CampaignPortfolio {
    let reducible: BTreeMap<&str, &CampaignRequirement> =
        reqs.iter().filter(|r| !r.irreducible).map(|r| (r.requirement_id.as_str(), r)).collect();
    let reducible_weight: f64 = reducible.values().map(|r| r.weight).sum::<f64>().max(1e-12);
    let irreducible: Vec<String> = reqs.iter().filter(|r| r.irreducible).map(|r| r.requirement_id.clone()).collect();
    let known_costs = !assets.is_empty() && assets.iter().all(|a| a.estimated_cost_units.is_some());
    let cost_model = if known_costs { CostModel::KnownCosts } else { CostModel::UnknownCostsMinimisingAssetCount };
    let algorithm = "greedy constrained portfolio heuristic; lexicographic objective (critical confirmed > critical provisional > confirmed > provisional) per unit cost; tie-break: fewer open items, then asset_id; exact enumeration check for small asset sets".to_string();

    let sel = if assets.is_empty() || reducible.is_empty() { Vec::new() } else { greedy(&reducible, assets, constraints, known_costs, None) };
    let st = state_of(&sel, assets, &reducible);
    let obj = objective(&st, &reducible);
    let cov = |o: &CoverageObjective| (o.confirmed / reducible_weight, (o.confirmed + o.provisional) / reducible_weight);

    let mut steps = Vec::new();
    let mut running = State::new();
    for &i in &sel {
        let before = running.clone();
        apply(&mut running, &assets[i], &reducible);
        let mut s = SelectionStep {
            asset_id: assets[i].asset_id.clone(),
            newly_confirmed: Vec::new(),
            newly_provisional: Vec::new(),
            upgraded_to_confirmed: Vec::new(),
            cost_units: assets[i].estimated_cost_units,
        };
        for (id, now) in &running {
            match (before.get(id), now) {
                (None, CoverageStatus::Confirmed) => s.newly_confirmed.push(id.to_string()),
                (None, CoverageStatus::Provisional) => s.newly_provisional.push(id.to_string()),
                (Some(CoverageStatus::Provisional), CoverageStatus::Confirmed) => s.upgraded_to_confirmed.push(id.to_string()),
                _ => {}
            }
        }
        steps.push(s);
    }

    let claimable: BTreeSet<&str> = assets.iter().flat_map(|a| a.coverage.iter().map(|c| c.requirement_id.as_str())).collect();
    let pick = |f: &dyn Fn(&str) -> bool| -> Vec<String> { reducible.keys().filter(|k| f(k)).map(|k| k.to_string()).collect() };
    let confirmed = pick(&|k| st.get(k) == Some(&CoverageStatus::Confirmed));
    let provisional = pick(&|k| st.get(k) == Some(&CoverageStatus::Provisional));
    let uncovered = pick(&|k| !st.contains_key(k) && claimable.contains(k));
    let no_capable = pick(&|k| !claimable.contains(k));
    let critical_unconfirmed: Vec<String> = reqs
        .iter()
        .filter(|r| r.criticality == Criticality::MissionCritical)
        .filter(|r| r.irreducible || st.get(r.requirement_id.as_str()) != Some(&CoverageStatus::Confirmed))
        .map(|r| r.requirement_id.clone())
        .collect();

    let status = if assets.is_empty() || reducible.is_empty() {
        PortfolioOptimizationStatus::PortfolioNotOptimizedNoCapabilityData
    } else if !uncovered.is_empty() {
        PortfolioOptimizationStatus::PartialCoverageUnderConstraints
    } else if !no_capable.is_empty() {
        PortfolioOptimizationStatus::PartialCoverageNoCapableAsset
    } else if !provisional.is_empty() {
        PortfolioOptimizationStatus::AllReducibleTestableSomeProvisional
    } else {
        PortfolioOptimizationStatus::AllReducibleConfirmed
    };

    let baseline = assets
        .iter()
        .enumerate()
        .filter(|(_, a)| matches!(a.kind, CampaignAssetKind::FieldAnalogSite { .. }))
        .map(|(i, a)| (a, objective(&state_of(&[i], assets, &reducible), &reducible)))
        .max_by(|x, y| x.1.cmp_lex(&y.1).then_with(|| y.0.asset_id.cmp(&x.0.asset_id)))
        .map(|(a, o)| {
            let (c, t) = cov(&o);
            BaselineComparison { asset_id: a.asset_id.clone(), confirmed_coverage: c, testable_coverage: t }
        });

    let alternatives = sel
        .iter()
        .map(|&ex| {
            let alt = greedy(&reducible, assets, constraints, known_costs, Some(ex));
            let ast = state_of(&alt, assets, &reducible);
            let (c, t) = cov(&objective(&ast, &reducible));
            AlternativePortfolio {
                without_asset: assets[ex].asset_id.clone(),
                selected_assets: alt.iter().map(|&i| assets[i].asset_id.clone()).collect(),
                confirmed_coverage: c,
                testable_coverage: t,
                critical_confirmations_lost: reqs
                    .iter()
                    .filter(|r| r.criticality == Criticality::MissionCritical && !r.irreducible)
                    .filter(|r| st.get(r.requirement_id.as_str()) == Some(&CoverageStatus::Confirmed)
                        && ast.get(r.requirement_id.as_str()) != Some(&CoverageStatus::Confirmed))
                    .map(|r| r.requirement_id.clone())
                    .collect(),
            }
        })
        .collect();

    let exact_check = (!assets.is_empty() && assets.len() <= exact_check_max_assets).then(|| {
        let n = assets.len();
        let mut best: Option<(Vec<usize>, CoverageObjective, f64)> = None;
        for mask in 0u32..(1u32 << n) {
            let s: Vec<usize> = (0..n).filter(|i| mask & (1 << i) != 0).collect();
            if !within_constraints(&s, assets, constraints) {
                continue;
            }
            let o = objective(&state_of(&s, assets, &reducible), &reducible);
            let cost: f64 = s.iter().filter_map(|&i| assets[i].estimated_cost_units).sum();
            let better = match &best {
                None => true,
                Some((bs, bo, bc)) => match o.cmp_lex(bo) {
                    std::cmp::Ordering::Greater => true,
                    std::cmp::Ordering::Equal => s.len() < bs.len() || (s.len() == bs.len() && cost < bc - 1e-9),
                    std::cmp::Ordering::Less => false,
                },
            };
            if better {
                best = Some((s, o, cost));
            }
        }
        let (bs, bo, _) = best.unwrap_or((Vec::new(), CoverageObjective::default(), 0.0));
        ExactCheck {
            assets_considered: n,
            best_selected_assets: bs.iter().map(|&i| assets[i].asset_id.clone()).collect(),
            greedy_is_optimal: obj.cmp_lex(&bo) == std::cmp::Ordering::Equal && sel.len() <= bs.len(),
            best_objective: bo,
            greedy_objective: obj,
        }
    });

    let sum_opt = |f: &dyn Fn(&CampaignAssetCapability) -> Option<f64>| -> Option<f64> { sel.iter().map(|&i| f(&assets[i])).sum::<Option<f64>>() };
    let (cc, tc) = cov(&obj);
    CampaignPortfolio {
        status,
        algorithm,
        cost_model,
        selected_assets: sel.iter().map(|&i| assets[i].asset_id.clone()).collect(),
        selection_steps: steps,
        confirmed_requirements: confirmed,
        provisional_requirements: provisional,
        uncovered_requirements: uncovered,
        no_capable_asset_requirements: no_capable,
        irreducible_requirements: irreducible,
        critical_unconfirmed,
        confirmed_coverage: cc,
        testable_coverage: tc,
        objective: obj,
        total_cost_units: sum_opt(&|a| a.estimated_cost_units),
        total_duration_days: sum_opt(&|a| a.duration_days),
        budget_check: constraint_check(&sel, assets, constraints.max_budget_units, |a| a.estimated_cost_units),
        duration_check: constraint_check(&sel, assets, constraints.max_total_duration_days, |a| a.duration_days),
        single_best_site_baseline: baseline,
        alternatives,
        exact_check,
    }
}

// ---------------------------------------------------------------------------
// U.D.D. (site level)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FlipAssessment {
    NoCandidates,
    NoRunnerUp,
    /// Uncertainty of at least one decisive feature is unknown: flip risk cannot be quantified.
    UncertaintyUnknown { delta_d: f64 },
    /// |d2 - d1| < 2 sigma: the order of #1 and #2 is not resolved by the data.
    CanFlip { delta_d: f64, sigma_delta: f64 },
    Stable { delta_d: f64, sigma_delta: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UncertaintyRecommendation {
    pub decision_between: Option<(String, String)>,
    pub flip: FlipAssessment,
    pub recommended_measurement: Option<String>,
    pub recommended_feature: Option<FeatureId>,
    /// Expected reduction of σ_Δ if the recommended requirement were measured perfectly at both sites.
    pub expected_sigma_delta_after: Option<f64>,
    /// Would that measurement alone make the ranking resolvable (Δd >= 2σ after)?
    pub measurement_can_resolve: Option<bool>,
    /// σ_Δ assumes independent errors at the two sites (no covariance supplied).
    pub independence_assumed: bool,
    pub correlation_note: Option<String>,
    pub rationale: String,
    /// Always None until a decision-value and measurement-cost model is supplied (no fictitious VOI).
    pub expected_voi_score: Option<f64>,
}

/// `shared_source`: both sites come from the same DEM source/processing. Errors are then
/// likely positively correlated, so the independent σ_Δ over-states flip risk (conservative).
pub fn build_udd_recommendation(top: Option<&AnalogDna>, second: Option<&AnalogDna>, shared_source: bool) -> UncertaintyRecommendation {
    let base = |flip: FlipAssessment, why: String| UncertaintyRecommendation {
        decision_between: None,
        flip,
        recommended_measurement: None,
        recommended_feature: None,
        expected_sigma_delta_after: None,
        measurement_can_resolve: None,
        independence_assumed: true,
        correlation_note: None,
        rationale: why,
        expected_voi_score: None,
    };
    let Some(a) = top else {
        return base(FlipAssessment::NoCandidates, "No feasible candidate: acquire Earth data covering the requirements.".into());
    };
    let Some(b) = second else {
        return base(FlipAssessment::NoRunnerUp, "Single feasible candidate: no ranking decision to protect.".into());
    };
    let delta = b.mission_distance - a.mission_distance;
    // (id, feature, |Δ contribution|, variance contribution or None)
    let mut rows: Vec<(String, FeatureId, f64, Option<f64>)> = Vec::new();
    for ra in &a.breakdown {
        if let Some(rb) = b.breakdown.iter().find(|r| r.requirement_id == ra.requirement_id) {
            let diff = (ra.z - rb.z).abs() * ra.effective_weight;
            let var = match (ra.sigma_value, rb.sigma_value) {
                (Some(sa), Some(sb)) => Some(ra.effective_weight.powi(2) * ((sa / ra.tolerance).powi(2) + (sb / rb.tolerance).powi(2))),
                _ => None,
            };
            rows.push((ra.requirement_id.clone(), ra.feature, diff, var));
        }
    }
    let mut rec = base(FlipAssessment::NoRunnerUp, String::new());
    rec.decision_between = Some((a.cell_id.clone(), b.cell_id.clone()));
    if rows.iter().any(|r| r.3.is_none()) {
        let pick = rows.iter().filter(|r| r.3.is_none()).max_by(|x, y| x.2.total_cmp(&y.2).then_with(|| y.0.cmp(&x.0)));
        rec.flip = FlipAssessment::UncertaintyUnknown { delta_d: delta };
        rec.recommended_measurement = pick.map(|r| r.0.clone());
        rec.recommended_feature = pick.map(|r| r.1);
        rec.rationale = format!(
            "#1 and #2 differ by Δd = {:.3}; the uncertainty of {} is unknown, so whether this gap is real cannot be tested. Measure its uncertainty at both sites first.",
            delta,
            pick.map(|r| r.0.as_str()).unwrap_or("-")
        );
        return rec;
    }
    rec.correlation_note = Some(if shared_source {
        "Assumption: σ_Δ treats the two sites' errors as independent (no covariance supplied). Both sites share one DEM source/processing, so errors are likely positively correlated and this σ_Δ over-states the flip risk.".to_string()
    } else {
        "Assumption: σ_Δ treats the two sites' errors as independent (no covariance supplied).".to_string()
    });
    let var_total: f64 = rows.iter().filter_map(|r| r.3).sum();
    let sigma_delta = var_total.sqrt();
    let can_flip = delta < 2.0 * sigma_delta;
    rec.flip = if can_flip {
        FlipAssessment::CanFlip { delta_d: delta, sigma_delta }
    } else {
        FlipAssessment::Stable { delta_d: delta, sigma_delta }
    };
    if can_flip {
        // Choose the measurement that resolves the decision, not merely the noisiest feature:
        // prefer requirements whose perfect measurement makes Δd >= 2σ; among them the largest σ reduction.
        let mut best: Option<(&(String, FeatureId, f64, Option<f64>), f64, bool)> = None;
        for r in &rows {
            let after = (var_total - r.3.unwrap_or(0.0)).max(0.0).sqrt();
            let resolves = delta >= 2.0 * after;
            let better = match &best {
                None => true,
                Some((br, ba, bres)) => (resolves && !bres) || (resolves == *bres && (after < *ba - 1e-12 || ((after - ba).abs() <= 1e-12 && r.0 < br.0))),
            };
            if better {
                best = Some((r, after, resolves));
            }
        }
        if let Some((r, after, resolves)) = best {
            rec.recommended_measurement = Some(r.0.clone());
            rec.recommended_feature = Some(r.1);
            rec.expected_sigma_delta_after = Some(after);
            rec.measurement_can_resolve = Some(resolves);
            rec.rationale = format!(
                "Δd = {:.3} < 2σ = {:.3}: #1 vs #2 is not resolved. Re-measuring {} at both sites lowers σ_Δ to {:.3}{}",
                delta,
                2.0 * sigma_delta,
                r.0,
                after,
                if resolves { ", which resolves the choice." } else { "; no single measurement resolves it - treat both as equivalent candidates." }
            );
        }
    } else {
        rec.rationale = format!("Δd = {:.3} >= 2σ = {:.3}: the choice of #1 over #2 is robust to measured uncertainty.", delta, 2.0 * sigma_delta);
    }
    rec
}
