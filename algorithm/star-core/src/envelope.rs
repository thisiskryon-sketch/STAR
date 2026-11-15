// star-core/src/envelope.rs
//! Mission distance (spec-driven) and the three-level Earth Envelope:
//! Marginal (each requirement somewhere on Earth), Joint (all requirements in one
//! place, supported by independent sites) and Mechanism (can the physics be
//! reproduced at a field site, in a laboratory, or nowhere on Earth).
use crate::novelty::spatially_independent;
use crate::schema::{
    Cell, FeatureFamily, FeatureId, FeatureScales, MissionExperimentSpec, ParameterRequirement,
    RunParameters, TestVenue,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// Mission distance
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementScore {
    pub requirement_id: String,
    pub feature: FeatureId,
    pub unit: String,
    pub target_value: f64,
    pub acceptable_min: Option<f64>,
    pub acceptable_max: Option<f64>,
    pub observed_value: f64,
    /// |x - target| / tolerance. <= 1 means inside the acceptable range.
    pub z: f64,
    /// Tolerance used for z on the side of the observed value (feature units).
    pub tolerance: f64,
    pub within_range: bool,
    /// 1-sigma of the observed value; None = unknown (not zero).
    pub sigma_value: Option<f64>,
    /// Whether sigma satisfies the requirement's max_uncertainty; None if either is unknown.
    pub uncertainty_ok: Option<bool>,
    pub effective_weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionDistance {
    /// Family-balanced weighted mean of z. Pure point estimate: identical values give 0.
    pub d: f64,
    /// Propagated 1-sigma of d; None if any active feature uncertainty is unknown.
    pub sigma_d: Option<f64>,
    pub per_requirement: Vec<RequirementScore>,
}

impl MissionDistance {
    pub fn all_within_range(&self) -> bool {
        self.per_requirement.iter().all(|r| r.within_range)
    }
}

/// Family-balanced weights: requirements on correlated multi-scale features of the
/// same family share their family's weight instead of multiplying it (P1-5).
fn effective_weights(reqs: &[ParameterRequirement]) -> Vec<f64> {
    let mut per_family: BTreeMap<FeatureFamily, usize> = BTreeMap::new();
    for r in reqs {
        *per_family.entry(r.feature.family()).or_insert(0) += 1;
    }
    reqs.iter()
        .map(|r| r.weight / per_family[&r.feature.family()] as f64)
        .collect()
}

pub fn mission_distance(
    spec: &MissionExperimentSpec,
    features: &crate::schema::DemFeatures,
    feature_uncertainty: &BTreeMap<FeatureId, f64>,
) -> MissionDistance {
    let reqs = &spec.required_parameters;
    let w = effective_weights(reqs);
    let w_sum: f64 = w.iter().sum::<f64>().max(1e-12);
    let mut d = 0.0;
    let mut var = 0.0;
    let mut sigma_known = true;
    let mut per_requirement = Vec::with_capacity(reqs.len());
    for (r, &wr) in reqs.iter().zip(&w) {
        let x = features.get(r.feature);
        let z = r.z(x);
        let sigma = feature_uncertainty.get(&r.feature).copied();
        match sigma {
            Some(s) => var += (wr * s / r.tolerance_for(x)).powi(2),
            None => sigma_known = false,
        }
        d += wr * z;
        per_requirement.push(RequirementScore {
            requirement_id: r.requirement_id.clone(),
            feature: r.feature,
            unit: r.unit.clone(),
            target_value: r.target_value,
            acceptable_min: r.acceptable_min,
            acceptable_max: r.acceptable_max,
            observed_value: x,
            z,
            tolerance: r.tolerance_for(x),
            within_range: r.within_range(x),
            sigma_value: sigma,
            uncertainty_ok: match (sigma, r.max_uncertainty) {
                (Some(s), Some(m)) => Some(s <= m),
                _ => None,
            },
            effective_weight: wr / w_sum,
        });
    }
    MissionDistance {
        d: d / w_sum,
        sigma_d: sigma_known.then(|| var.sqrt() / w_sum),
        per_requirement,
    }
}

// ---------------------------------------------------------------------------
// Earth-to-Earth feature distance (used only for rarity / catalog novelty)
// ---------------------------------------------------------------------------

pub fn compute_feature_scales(valid_cells: &[&Cell], calibration_version: &str) -> Option<FeatureScales> {
    if valid_cells.is_empty() {
        return None;
    }
    let mut p01 = BTreeMap::new();
    let mut p99 = BTreeMap::new();
    let mut degenerate = Vec::new();
    for feat in FeatureId::ALL {
        let mut vals: Vec<f64> = valid_cells.iter().map(|c| c.features.get(feat)).collect();
        vals.sort_by(|a, b| a.total_cmp(b));
        let n = vals.len();
        let lo = vals[((n - 1) as f64 * 0.01).round() as usize];
        let hi = vals[((n - 1) as f64 * 0.99).round() as usize];
        if hi - lo <= 0.0 {
            degenerate.push(feat);
        }
        p01.insert(feat, lo);
        p99.insert(feat, hi);
    }
    Some(FeatureScales {
        calibration_version: calibration_version.to_string(),
        sample_count: valid_cells.len(),
        p01,
        p99,
        degenerate_features: degenerate,
    })
}

/// Family-balanced mean of |Δ| / (P99 - P01). Degenerate/uncalibrated features are
/// skipped (never divided by an epsilon). Returns None if no feature is usable.
pub fn feature_distance(
    a: &crate::schema::DemFeatures,
    b: &crate::schema::DemFeatures,
    active: &[FeatureId],
    scales: &FeatureScales,
) -> Option<f64> {
    let usable: Vec<(FeatureId, f64)> =
        active.iter().filter_map(|&f| scales.range(f).map(|r| (f, r))).collect();
    if usable.is_empty() {
        return None;
    }
    let mut per_family: BTreeMap<FeatureFamily, usize> = BTreeMap::new();
    for (f, _) in &usable {
        *per_family.entry(f.family()).or_insert(0) += 1;
    }
    let n_fam = per_family.len() as f64;
    Some(
        usable
            .iter()
            .map(|&(f, r)| (a.get(f) - b.get(f)).abs() / r / (n_fam * per_family[&f.family()] as f64))
            .sum(),
    )
}

// ---------------------------------------------------------------------------
// Earth Envelope
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginalReach {
    pub requirement_id: String,
    pub feature: FeatureId,
    pub cells_within_range: usize,
    pub observed_min: f64,
    pub observed_max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialJointSet {
    pub requirement_ids: Vec<String>,
    pub supporting_cells: usize,
    pub independent_sites: usize,
    pub example_cell_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JointReachStatus {
    NoValidEarthData,
    /// At least one requirement is met by no QC-valid Earth cell.
    OutsideMarginalEnvelope { unreachable_requirements: Vec<String> },
    /// Every requirement is met somewhere, but never all in the same cell.
    MarginalOnlyJointUnobserved,
    /// All requirements met jointly, but only at one spatially independent site.
    JointObservedSingleSite { supporting_cells: usize },
    JointObservedWithSupport { supporting_cells: usize, independent_sites: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanismReach {
    pub mechanism_id: String,
    pub venues: Vec<TestVenue>,
    pub field_min_abs_latitude_deg: Option<f64>,
    pub irreducible_on_earth: bool,
    pub required_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanismEnvelope {
    pub per_mechanism: Vec<MechanismReach>,
    /// Mechanisms referenced by transfer requirements but absent from the registry: status Unknown.
    pub unknown_mechanisms: Vec<String>,
    pub irreducible_mechanisms: Vec<String>,
    /// Venues that can host every reducible mechanism at once (empty = no single venue does).
    pub venues_hosting_all_reducible: Vec<TestVenue>,
    pub single_venue_reproduces_everything: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeAssessment {
    pub qc_valid_cells: usize,
    pub marginal: Vec<MarginalReach>,
    pub joint_status: JointReachStatus,
    /// Maximal sets of requirements that are jointly met by at least one cell (largest first).
    pub best_partial_joint_sets: Vec<PartialJointSet>,
    pub nearest_cell_id: Option<String>,
    pub nearest_mission_distance: Option<f64>,
    pub mechanism: MechanismEnvelope,
}

pub fn evaluate_mechanism_envelope(spec: &MissionExperimentSpec) -> MechanismEnvelope {
    let mut required: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for tr in &spec.transfer_requirements {
        required.entry(tr.required_mechanism_id.clone()).or_default().push(tr.requirement_id.clone());
    }
    let mut per_mechanism = Vec::new();
    let mut unknown = Vec::new();
    let mut irreducible = Vec::new();
    let mut common: Option<BTreeSet<TestVenue>> = None;
    for (mid, by) in &required {
        match spec.mechanism(mid) {
            None => unknown.push(mid.clone()),
            Some(m) => {
                let venues: BTreeSet<TestVenue> = m.venues.iter().copied().collect();
                if m.is_irreducible_on_earth() {
                    irreducible.push(mid.clone());
                } else {
                    common = Some(match common {
                        None => venues.clone(),
                        Some(c) => c.intersection(&venues).copied().collect(),
                    });
                }
                per_mechanism.push(MechanismReach {
                    mechanism_id: mid.clone(),
                    venues: venues.into_iter().collect(),
                    field_min_abs_latitude_deg: m.field_min_abs_latitude_deg,
                    irreducible_on_earth: m.is_irreducible_on_earth(),
                    required_by: by.clone(),
                });
            }
        }
    }
    let venues_all: Vec<TestVenue> = common.unwrap_or_default().into_iter().collect();
    let single = irreducible.is_empty() && unknown.is_empty() && !venues_all.is_empty();
    MechanismEnvelope {
        per_mechanism,
        unknown_mechanisms: unknown,
        irreducible_mechanisms: irreducible,
        venues_hosting_all_reducible: venues_all,
        single_venue_reproduces_everything: single,
    }
}

/// Envelope over QC-valid Earth cells (before mission hard constraints, so that
/// "what Earth offers" does not depend on what the mission forbids).
pub fn evaluate_earth_envelope(
    spec: &MissionExperimentSpec,
    qc_valid_cells: &[&Cell],
    params: &RunParameters,
) -> EnvelopeAssessment {
    let mechanism = evaluate_mechanism_envelope(spec);
    let reqs = &spec.required_parameters;
    if qc_valid_cells.is_empty() {
        return EnvelopeAssessment {
            qc_valid_cells: 0,
            marginal: Vec::new(),
            joint_status: JointReachStatus::NoValidEarthData,
            best_partial_joint_sets: Vec::new(),
            nearest_cell_id: None,
            nearest_mission_distance: None,
            mechanism,
        };
    }

    // Satisfied-requirement mask per cell.
    let masks: Vec<u64> = qc_valid_cells
        .iter()
        .map(|c| {
            reqs.iter().enumerate().fold(0u64, |m, (i, r)| {
                if r.within_range(c.features.get(r.feature)) { m | (1 << i) } else { m }
            })
        })
        .collect();

    let marginal: Vec<MarginalReach> = reqs
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let vals = qc_valid_cells.iter().map(|c| c.features.get(r.feature));
            MarginalReach {
                requirement_id: r.requirement_id.clone(),
                feature: r.feature,
                cells_within_range: masks.iter().filter(|m| *m & (1 << i) != 0).count(),
                observed_min: vals.clone().fold(f64::INFINITY, f64::min),
                observed_max: vals.fold(f64::NEG_INFINITY, f64::max),
            }
        })
        .collect();

    let full: u64 = if reqs.len() >= 64 { u64::MAX } else { (1u64 << reqs.len()) - 1 };
    let sites_for = |mask: u64| -> (usize, usize, String) {
        let mut cells: Vec<&Cell> = qc_valid_cells
            .iter()
            .zip(&masks)
            .filter(|(_, m)| **m & mask == mask)
            .map(|(c, _)| *c)
            .collect();
        cells.sort_by(|a, b| a.id.cmp(&b.id));
        let independent = spatially_independent(&cells, params.spatial_thinning_radius_km).len();
        let example = cells.first().map(|c| c.id.clone()).unwrap_or_default();
        (cells.len(), independent, example)
    };

    // Maximal jointly-satisfied subsets.
    let mut distinct: BTreeSet<u64> = masks.iter().copied().filter(|m| *m != 0).collect();
    let all: Vec<u64> = distinct.iter().copied().collect();
    distinct.retain(|m| !all.iter().any(|o| o != m && o & m == *m));
    let mut best_partial: Vec<PartialJointSet> = distinct
        .into_iter()
        .map(|m| {
            let (n, ind, ex) = sites_for(m);
            PartialJointSet {
                requirement_ids: reqs
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| m & (1 << i) != 0)
                    .map(|(_, r)| r.requirement_id.clone())
                    .collect(),
                supporting_cells: n,
                independent_sites: ind,
                example_cell_id: ex,
            }
        })
        .collect();
    best_partial.sort_by(|a, b| {
        b.requirement_ids
            .len()
            .cmp(&a.requirement_ids.len())
            .then(b.supporting_cells.cmp(&a.supporting_cells))
            .then(a.requirement_ids.cmp(&b.requirement_ids))
    });
    best_partial.truncate(5);

    let unreachable: Vec<String> = marginal
        .iter()
        .filter(|m| m.cells_within_range == 0)
        .map(|m| m.requirement_id.clone())
        .collect();
    let joint_status = if !unreachable.is_empty() {
        JointReachStatus::OutsideMarginalEnvelope { unreachable_requirements: unreachable }
    } else {
        let (n, ind, _) = sites_for(full);
        if n == 0 {
            JointReachStatus::MarginalOnlyJointUnobserved
        } else if ind >= params.min_knn_support.max(2) {
            JointReachStatus::JointObservedWithSupport { supporting_cells: n, independent_sites: ind }
        } else {
            JointReachStatus::JointObservedSingleSite { supporting_cells: n }
        }
    };

    let nearest = qc_valid_cells
        .iter()
        .map(|c| (c, mission_distance(spec, &c.features, &c.feature_uncertainty).d))
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.id.cmp(&b.0.id)));

    EnvelopeAssessment {
        qc_valid_cells: qc_valid_cells.len(),
        marginal,
        joint_status,
        best_partial_joint_sets: best_partial,
        nearest_cell_id: nearest.map(|(c, _)| c.id.clone()),
        nearest_mission_distance: nearest.map(|(_, d)| d),
        mechanism,
    }
}
