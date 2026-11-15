// star-core/src/schema.rs
//! Data contract of the S.T.A.R. core.
//!
//! Units: slopes in degrees, roughness in metres, shadow obstruction as a
//! dimensionless ratio in [0, 1]. Coordinates are WGS84 degrees.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CelestialBody {
    Moon,
    Mars,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TestProfile {
    P1TerrainMobility,
    P2LowSunIllumination,
    P3CombinedTerrainShadow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FeatureId {
    RmsSlope1x,
    RmsSlope3x,
    RmsSlope9x,
    Roughness1x,
    Roughness3x,
    Roughness9x,
    ShadowObstruction1Deg,
    ShadowObstruction2Deg,
    ShadowObstruction3Deg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FeatureFamily {
    TerrainSlope,
    TerrainRoughness,
    IlluminationShadow,
}

impl FeatureId {
    pub const ALL: [FeatureId; 9] = [
        FeatureId::RmsSlope1x,
        FeatureId::RmsSlope3x,
        FeatureId::RmsSlope9x,
        FeatureId::Roughness1x,
        FeatureId::Roughness3x,
        FeatureId::Roughness9x,
        FeatureId::ShadowObstruction1Deg,
        FeatureId::ShadowObstruction2Deg,
        FeatureId::ShadowObstruction3Deg,
    ];

    pub fn family(&self) -> FeatureFamily {
        match self {
            FeatureId::RmsSlope1x | FeatureId::RmsSlope3x | FeatureId::RmsSlope9x => {
                FeatureFamily::TerrainSlope
            }
            FeatureId::Roughness1x | FeatureId::Roughness3x | FeatureId::Roughness9x => {
                FeatureFamily::TerrainRoughness
            }
            FeatureId::ShadowObstruction1Deg
            | FeatureId::ShadowObstruction2Deg
            | FeatureId::ShadowObstruction3Deg => FeatureFamily::IlluminationShadow,
        }
    }

    pub fn unit(&self) -> &'static str {
        match self.family() {
            FeatureFamily::TerrainSlope => "deg",
            FeatureFamily::TerrainRoughness => "m",
            FeatureFamily::IlluminationShadow => "ratio",
        }
    }

    pub fn active_for_profile(profile: TestProfile) -> Vec<FeatureId> {
        match profile {
            TestProfile::P1TerrainMobility => FeatureId::ALL[..6].to_vec(),
            TestProfile::P2LowSunIllumination => FeatureId::ALL[6..].to_vec(),
            TestProfile::P3CombinedTerrainShadow => FeatureId::ALL.to_vec(),
        }
    }
}

/// How much an unverified requirement endangers the mission. Critical requirements are
/// never traded against a larger number of lower-class requirements (lexicographic objective).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub enum Criticality {
    MissionCritical,
    High,
    #[default]
    Normal,
    Exploratory,
}

/// Where a physical mechanism can be reproduced on Earth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TestVenue {
    FieldSite,
    Laboratory,
}

/// Mechanism registry entry (Mechanism Envelope, FR-04 / FR-06).
/// An empty `venues` list means the mechanism cannot be reproduced on Earth in a
/// sustained way: it is an irreducible gap of any terrestrial campaign.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanismSpec {
    pub mechanism_id: String,
    pub description: String,
    pub venues: Vec<TestVenue>,
    /// Declared field condition: the mechanism is reproducible at a field site only
    /// if |latitude| >= this value (e.g. sustained low-sun geometry). Input data, not physics derived in code.
    #[serde(default)]
    pub field_min_abs_latitude_deg: Option<f64>,
    #[serde(default)]
    pub note: String,
    /// The mechanism exists at a field site only during specific time windows (e.g. low sun):
    /// a field test needs a known, valid window inside the campaign period.
    #[serde(default)]
    pub requires_field_time_window: bool,
}

impl MechanismSpec {
    pub fn is_irreducible_on_earth(&self) -> bool {
        self.venues.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionExperimentSpec {
    pub mission_id: String,
    pub experiment_id: String,
    pub target_body: CelestialBody,
    pub target_region: String,
    /// Id of the TargetProfile this spec was derived from (cross-checked, optional).
    #[serde(default)]
    pub target_profile_id: Option<String>,
    pub objective: String,
    pub fallback_profile: TestProfile,
    pub required_parameters: Vec<ParameterRequirement>,
    pub hard_constraints: Vec<HardConstraint>,
    pub transfer_requirements: Vec<TransferRequirement>,
    #[serde(default)]
    pub mechanisms: Vec<MechanismSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterRequirement {
    pub requirement_id: String,
    pub feature: FeatureId,
    pub target_value: f64,
    pub acceptable_min: Option<f64>,
    pub acceptable_max: Option<f64>,
    /// Maximum acceptable 1-sigma measurement uncertainty of the analog value (feature units).
    pub max_uncertainty: Option<f64>,
    pub weight: f64,
    pub unit: String,
    #[serde(default)]
    pub criticality: Criticality,
}

impl ParameterRequirement {
    /// Normalised deviation z = |x - target| / tolerance on the side of x.
    /// z <= 1  <=>  x is inside the acceptable range (when both bounds are given).
    pub fn tolerance_for(&self, value: f64) -> f64 {
        let above = self.acceptable_max.map(|m| m - self.target_value);
        let below = self.acceptable_min.map(|m| self.target_value - m);
        let pick = if value >= self.target_value { above.or(below) } else { below.or(above) };
        let tol = match pick {
            Some(t) if t > 0.0 => t,
            _ => above.into_iter().chain(below).fold(0.0_f64, f64::max),
        };
        tol.max(1e-12)
    }

    pub fn z(&self, value: f64) -> f64 {
        (value - self.target_value).abs() / self.tolerance_for(value)
    }

    pub fn within_range(&self, value: f64) -> bool {
        self.acceptable_min.map_or(true, |m| value >= m)
            && self.acceptable_max.map_or(true, |m| value <= m)
            && (self.acceptable_min.is_some() || self.acceptable_max.is_some() || self.z(value) <= 1.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardConstraint {
    pub constraint_id: String,
    pub feature: FeatureId,
    pub min_allowed: Option<f64>,
    pub max_allowed: Option<f64>,
    /// Per-constraint override of RunParameters::max_resolution_ratio.
    pub max_resolution_mismatch_ratio: Option<f64>,
}

fn default_weight() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequirement {
    pub requirement_id: String,
    pub engineering_decision: String,
    pub required_mechanism_id: String,
    pub requires_validated_correction_model: bool,
    /// Parameter requirements whose observable range must hold for the result to apply.
    /// Empty = all parameter requirements.
    #[serde(default)]
    pub depends_on_parameters: Vec<String>,
    #[serde(default = "default_weight")]
    pub weight: f64,
    #[serde(default)]
    pub criticality: Criticality,
}

impl MissionExperimentSpec {
    pub fn active_features(&self) -> Vec<FeatureId> {
        if self.required_parameters.is_empty() {
            FeatureId::active_for_profile(self.fallback_profile)
        } else {
            let mut feats: Vec<FeatureId> =
                self.required_parameters.iter().map(|r| r.feature).collect();
            feats.sort();
            feats.dedup();
            feats
        }
    }

    pub fn mechanism(&self, id: &str) -> Option<&MechanismSpec> {
        self.mechanisms.iter().find(|m| m.mechanism_id == id)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        let err = |m: String| Err(SchemaError::InvalidMissionSpec(m));
        if self.mission_id.trim().is_empty() || self.experiment_id.trim().is_empty() {
            return err("mission_id and experiment_id must not be empty".into());
        }
        if self.required_parameters.is_empty() {
            return err("required_parameters must contain at least one measurable parameter".into());
        }
        if self.required_parameters.len() > 63 {
            return err("at most 63 parameter requirements are supported (joint masks are u64)".into());
        }
        let mut ids = HashSet::new();
        for req in &self.required_parameters {
            if !ids.insert(req.requirement_id.clone()) {
                return err(format!("Duplicate requirement id {}", req.requirement_id));
            }
            if !(req.target_value.is_finite() && req.weight.is_finite() && req.weight > 0.0) {
                return err(format!("Invalid target_value or weight in parameter {}", req.requirement_id));
            }
            if req.acceptable_min.is_none() && req.acceptable_max.is_none() {
                return err(format!(
                    "Parameter {} needs at least one acceptable bound (tolerance cannot be invented)",
                    req.requirement_id
                ));
            }
            for b in [req.acceptable_min, req.acceptable_max].into_iter().flatten() {
                if !b.is_finite() {
                    return err(format!("Non-finite bound in parameter {}", req.requirement_id));
                }
            }
            if req.acceptable_min.map_or(false, |m| m > req.target_value)
                || req.acceptable_max.map_or(false, |m| m < req.target_value)
            {
                return err(format!("target_value outside acceptable range in {}", req.requirement_id));
            }
            if req.tolerance_for(req.target_value + 1e-9) <= 1e-12
                && req.tolerance_for(req.target_value - 1e-9) <= 1e-12
            {
                return err(format!("Zero-width acceptable range in {}", req.requirement_id));
            }
            if let Some(u) = req.max_uncertainty {
                if !(u.is_finite() && u > 0.0) {
                    return err(format!("Invalid max_uncertainty in {}", req.requirement_id));
                }
            }
            if req.unit != req.feature.unit() {
                return err(format!(
                    "Unit mismatch in {}: '{}' given, feature {:?} is in '{}'",
                    req.requirement_id, req.unit, req.feature, req.feature.unit()
                ));
            }
        }
        for hc in &self.hard_constraints {
            if hc.min_allowed.is_none() && hc.max_allowed.is_none() {
                return err(format!("Hard constraint {} has no bound", hc.constraint_id));
            }
            for b in [hc.min_allowed, hc.max_allowed].into_iter().flatten() {
                if !b.is_finite() {
                    return err(format!("Non-finite bound in hard constraint {}", hc.constraint_id));
                }
            }
            if let (Some(a), Some(b)) = (hc.min_allowed, hc.max_allowed) {
                if a > b {
                    return err(format!("min_allowed > max_allowed in {}", hc.constraint_id));
                }
            }
            if let Some(r) = hc.max_resolution_mismatch_ratio {
                if !(r.is_finite() && r >= 1.0) {
                    return err(format!("max_resolution_mismatch_ratio must be >= 1 in {}", hc.constraint_id));
                }
            }
        }
        let mut mech_ids = HashSet::new();
        for m in &self.mechanisms {
            if !mech_ids.insert(m.mechanism_id.clone()) {
                return err(format!("Duplicate mechanism id {}", m.mechanism_id));
            }
            if let Some(l) = m.field_min_abs_latitude_deg {
                if !(l.is_finite() && (0.0..=90.0).contains(&l)) {
                    return err(format!("Invalid latitude condition in mechanism {}", m.mechanism_id));
                }
            }
        }
        for tr in &self.transfer_requirements {
            if !ids.insert(tr.requirement_id.clone()) {
                return err(format!("Duplicate requirement id {}", tr.requirement_id));
            }
            if !(tr.weight.is_finite() && tr.weight > 0.0) {
                return err(format!("Invalid weight in {}", tr.requirement_id));
            }
            for dep in &tr.depends_on_parameters {
                if !self.required_parameters.iter().any(|p| &p.requirement_id == dep) {
                    return err(format!("{} depends on unknown parameter {}", tr.requirement_id, dep));
                }
            }
        }
        Ok(())
    }

    /// Smallest resolution ratio allowed by the run parameters and any hard-constraint override.
    pub fn effective_max_resolution_ratio(&self, default_ratio: f64) -> f64 {
        self.hard_constraints
            .iter()
            .filter_map(|h| h.max_resolution_mismatch_ratio)
            .fold(default_ratio, f64::min)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemFeatures {
    pub rms_slope_1x: f64,
    pub rms_slope_3x: f64,
    pub rms_slope_9x: f64,
    pub roughness_1x: f64,
    pub roughness_3x: f64,
    pub roughness_9x: f64,
    pub shadow_obstruction_1deg: f64,
    pub shadow_obstruction_2deg: f64,
    pub shadow_obstruction_3deg: f64,
}

impl DemFeatures {
    pub fn get(&self, id: FeatureId) -> f64 {
        match id {
            FeatureId::RmsSlope1x => self.rms_slope_1x,
            FeatureId::RmsSlope3x => self.rms_slope_3x,
            FeatureId::RmsSlope9x => self.rms_slope_9x,
            FeatureId::Roughness1x => self.roughness_1x,
            FeatureId::Roughness3x => self.roughness_3x,
            FeatureId::Roughness9x => self.roughness_9x,
            FeatureId::ShadowObstruction1Deg => self.shadow_obstruction_1deg,
            FeatureId::ShadowObstruction2Deg => self.shadow_obstruction_2deg,
            FeatureId::ShadowObstruction3Deg => self.shadow_obstruction_3deg,
        }
    }

    pub fn get_mut(&mut self, id: FeatureId) -> &mut f64 {
        match id {
            FeatureId::RmsSlope1x => &mut self.rms_slope_1x,
            FeatureId::RmsSlope3x => &mut self.rms_slope_3x,
            FeatureId::RmsSlope9x => &mut self.rms_slope_9x,
            FeatureId::Roughness1x => &mut self.roughness_1x,
            FeatureId::Roughness3x => &mut self.roughness_3x,
            FeatureId::Roughness9x => &mut self.roughness_9x,
            FeatureId::ShadowObstruction1Deg => &mut self.shadow_obstruction_1deg,
            FeatureId::ShadowObstruction2Deg => &mut self.shadow_obstruction_2deg,
            FeatureId::ShadowObstruction3Deg => &mut self.shadow_obstruction_3deg,
        }
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        for f in FeatureId::ALL {
            let v = self.get(f);
            let ok = v.is_finite()
                && match f.family() {
                    FeatureFamily::TerrainSlope => (0.0..=90.0).contains(&v),
                    FeatureFamily::TerrainRoughness => v >= 0.0,
                    FeatureFamily::IlluminationShadow => (0.0..=1.0).contains(&v),
                };
            if !ok {
                return Err(SchemaError::InvalidFeatureValue(match f.family() {
                    FeatureFamily::TerrainSlope => "Slope outside [0, 90] deg",
                    FeatureFamily::TerrainRoughness => "Roughness must be >= 0",
                    FeatureFamily::IlluminationShadow => "Shadow obstruction outside [0, 1]",
                }));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DataOrigin {
    Synthetic,
    ExternalMeasured,
    #[default]
    Unspecified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceMetadata {
    pub source_id: String,
    pub source_version: String,
    pub processing_version: String,
    pub observation_window_utc: Option<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub id: String,
    pub region: String,
    pub lat: f64,
    pub lon: f64,
    pub resolution_m: f64,
    pub valid_pixel_ratio: f64,
    pub vertical_uncertainty_m: f64,
    pub features: DemFeatures,
    pub provenance: Option<ProvenanceMetadata>,
    /// 1-sigma uncertainty per feature in feature units. Missing = Unknown (never 0).
    #[serde(default)]
    pub feature_uncertainty: BTreeMap<FeatureId, f64>,
    #[serde(default)]
    pub data_origin: DataOrigin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetProfile {
    pub id: String,
    pub body: CelestialBody,
    pub region_name: String,
    pub test_profile: TestProfile,
    pub resolution_m: f64,
    pub vertical_uncertainty_m: f64,
    pub features: DemFeatures,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownAnalogSite {
    pub id: String,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub cluster_radius_km: f64,
    pub features: DemFeatures,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureScales {
    pub calibration_version: String,
    pub sample_count: usize,
    pub p01: BTreeMap<FeatureId, f64>,
    pub p99: BTreeMap<FeatureId, f64>,
    /// Features whose P01..P99 range is zero in the calibration set; excluded from
    /// Earth-to-Earth distances instead of being divided by an epsilon.
    #[serde(default)]
    pub degenerate_features: Vec<FeatureId>,
}

impl FeatureScales {
    /// None if the feature is uncalibrated or degenerate.
    pub fn range(&self, id: FeatureId) -> Option<f64> {
        if self.degenerate_features.contains(&id) {
            return None;
        }
        let low = self.p01.get(&id)?;
        let high = self.p99.get(&id)?;
        let r = high - low;
        (r > 0.0).then_some(r)
    }
}

/// How synthetic placeholders (data, evidence, lab catalog) are counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SyntheticEvidencePolicy {
    /// Synthetic content can never produce Confirmed coverage (default, production).
    #[default]
    Strict,
    /// Logic demonstration only: synthetic content is counted as if real, and every report says so.
    TreatAsConfirmedForLogicDemo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunParameters {
    pub min_valid_pixel_ratio: f64,
    pub max_vertical_uncertainty_m: f64,
    /// Soft/hard limit on mission distance, in tolerance units (1.0 = on average at the edge of the acceptable range).
    pub epsilon_mission_distance: f64,
    pub epsilon_is_hard_constraint: bool,
    pub knn_k: usize,
    /// Minimum number of spatially independent sites for "supported" joint/rarity statuses.
    pub min_knn_support: usize,
    pub spatial_thinning_radius_km: f64,
    /// Earth-to-Earth feature distance (range units) below which a combination is common.
    pub joint_support_radius: f64,
    pub max_resolution_ratio: f64,
    pub clean_ray_ratio_threshold: f64,
    #[serde(default = "default_top_n")]
    pub top_n_dossiers: usize,
    #[serde(default = "default_min_quality")]
    pub min_quality_for_supported_rarity: f64,
    /// A value closer than k·σ to a hard-constraint bound is flagged as a marginal pass.
    #[serde(default = "default_hc_margin")]
    pub hard_constraint_sigma_margin: f64,
    /// Number of weight perturbations for the decision-sensitivity analysis.
    #[serde(default = "default_sens_trials")]
    pub sensitivity_trials: usize,
    /// Exact enumeration of portfolios is run as a check when there are at most this many assets.
    #[serde(default = "default_exact_max")]
    pub exact_check_max_assets: usize,
    #[serde(default)]
    pub synthetic_evidence_policy: SyntheticEvidencePolicy,
}

fn default_hc_margin() -> f64 {
    2.0
}
fn default_sens_trials() -> usize {
    200
}
fn default_exact_max() -> usize {
    16
}

fn default_top_n() -> usize {
    10
}
fn default_min_quality() -> f64 {
    0.75
}

impl Default for RunParameters {
    fn default() -> Self {
        Self {
            min_valid_pixel_ratio: 0.85,
            max_vertical_uncertainty_m: 5.0,
            epsilon_mission_distance: 1.0,
            epsilon_is_hard_constraint: false,
            knn_k: 5,
            min_knn_support: 2,
            spatial_thinning_radius_km: 1.0,
            joint_support_radius: 0.10,
            max_resolution_ratio: 4.0,
            clean_ray_ratio_threshold: 0.85,
            top_n_dossiers: default_top_n(),
            min_quality_for_supported_rarity: default_min_quality(),
            hard_constraint_sigma_margin: default_hc_margin(),
            sensitivity_trials: default_sens_trials(),
            exact_check_max_assets: default_exact_max(),
            synthetic_evidence_policy: SyntheticEvidencePolicy::Strict,
        }
    }
}

impl RunParameters {
    pub fn validate(&self) -> Result<(), SchemaError> {
        let bad = |m: &str| Err(SchemaError::InvalidParameters(m.to_string()));
        let unit = |v: f64| v.is_finite() && (0.0..=1.0).contains(&v);
        if !unit(self.min_valid_pixel_ratio) {
            return bad("min_valid_pixel_ratio must be in [0, 1]");
        }
        if !(self.max_vertical_uncertainty_m.is_finite() && self.max_vertical_uncertainty_m > 0.0) {
            return bad("max_vertical_uncertainty_m must be > 0");
        }
        if !(self.epsilon_mission_distance.is_finite() && self.epsilon_mission_distance > 0.0) {
            return bad("epsilon_mission_distance must be > 0");
        }
        if self.knn_k == 0 {
            return bad("knn_k must be >= 1");
        }
        if self.min_knn_support == 0 {
            return bad("min_knn_support must be >= 1");
        }
        if !(self.spatial_thinning_radius_km.is_finite() && self.spatial_thinning_radius_km >= 0.0) {
            return bad("spatial_thinning_radius_km must be >= 0");
        }
        if !(self.joint_support_radius.is_finite() && self.joint_support_radius > 0.0) {
            return bad("joint_support_radius must be > 0");
        }
        if !(self.max_resolution_ratio.is_finite() && self.max_resolution_ratio >= 1.0) {
            return bad("max_resolution_ratio must be >= 1");
        }
        if !unit(self.clean_ray_ratio_threshold) || !unit(self.min_quality_for_supported_rarity) {
            return bad("ratio thresholds must be in [0, 1]");
        }
        if self.top_n_dossiers == 0 {
            return bad("top_n_dossiers must be >= 1");
        }
        if !(self.hard_constraint_sigma_margin.is_finite() && self.hard_constraint_sigma_margin >= 0.0) {
            return bad("hard_constraint_sigma_margin must be >= 0");
        }
        if self.exact_check_max_assets > 20 {
            return bad("exact_check_max_assets must be <= 20 (2^n enumeration)");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub run_id: String,
    pub timestamp_utc: String,
    pub code_revision: String,
    pub dataset_hash: String,
    pub calibration_version: String,
    pub random_seed: u64,
    pub is_synthetic_demo: bool,
    pub validation_protocol: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RejectionReason {
    LowValidPixelCoverage { observed: f64, required: f64 },
    ExcessiveVerticalUncertainty { observed: f64, max_allowed: f64 },
    IncompatibleSpatialResolution { cell_res_m: f64, target_res_m: f64, max_ratio: f64 },
    HardConstraintViolation {
        constraint_id: String,
        feature: FeatureId,
        observed: f64,
        min_allowed: Option<f64>,
        max_allowed: Option<f64>,
    },
    MissionDistanceExceedsHardEpsilon { d_mission: f64, epsilon: f64 },
}

impl RejectionReason {
    /// Negative Knowledge (FR-11): what would have to change for this reason to disappear.
    pub fn what_would_change_decision(&self) -> String {
        match self {
            RejectionReason::LowValidPixelCoverage { observed, required } => format!(
                "Valid-pixel coverage must rise from {:.2} to >= {:.2} (re-acquire or gap-fill the DEM).",
                observed, required
            ),
            RejectionReason::ExcessiveVerticalUncertainty { observed, max_allowed } => format!(
                "Vertical uncertainty must drop from {:.2} m to <= {:.2} m (higher-grade DEM).",
                observed, max_allowed
            ),
            RejectionReason::IncompatibleSpatialResolution { cell_res_m, target_res_m, max_ratio } => format!(
                "Resolution {:.1} m vs target {:.1} m exceeds ratio {:.1}; resample or use a DEM closer to {:.1} m.",
                cell_res_m, target_res_m, max_ratio, target_res_m
            ),
            RejectionReason::HardConstraintViolation { constraint_id, feature, observed, min_allowed, max_allowed } => {
                let need = match (min_allowed, max_allowed) {
                    (Some(a), Some(b)) => format!("[{a}, {b}]"),
                    (Some(a), None) => format!(">= {a}"),
                    (None, Some(b)) => format!("<= {b}"),
                    (None, None) => "-".into(),
                };
                format!(
                    "{:?} = {:.3} {} must be {} ({}); only a different site or a mission-approved relaxation changes this.",
                    feature, observed, feature.unit(), need, constraint_id
                )
            }
            RejectionReason::MissionDistanceExceedsHardEpsilon { d_mission, epsilon } => format!(
                "Mission distance {:.2} > hard epsilon {:.2}; switch epsilon to soft to review it as a near-miss.",
                d_mission, epsilon
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRejection {
    pub cell_id: String,
    pub stage_qc: bool,
    pub reasons: Vec<RejectionReason>,
    pub what_would_change_decision: Vec<String>,
}

impl CandidateRejection {
    pub fn new(cell_id: &str, stage_qc: bool, reasons: Vec<RejectionReason>) -> Self {
        let what = reasons.iter().map(|r| r.what_would_change_decision()).collect();
        Self { cell_id: cell_id.to_string(), stage_qc, reasons, what_would_change_decision: what }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SchemaError {
    DuplicateCellId(String),
    InvalidCoordinates(String),
    InvalidResolution(String),
    InvalidUncertainty(String),
    InvalidCoverage(String),
    InvalidFeatureValue(&'static str),
    InvalidMissionSpec(String),
    InvalidParameters(String),
    InconsistentTarget(String),
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaError::DuplicateCellId(id) => write!(f, "Duplicate cell ID: {}", id),
            SchemaError::InvalidCoordinates(id) => write!(f, "Invalid coordinates in cell: {}", id),
            SchemaError::InvalidResolution(id) => write!(f, "Invalid spatial resolution in cell: {}", id),
            SchemaError::InvalidUncertainty(id) => write!(f, "Invalid uncertainty in cell: {}", id),
            SchemaError::InvalidCoverage(id) => write!(f, "Invalid valid_pixel_ratio in cell: {}", id),
            SchemaError::InvalidFeatureValue(msg) => write!(f, "Invalid DEM feature value: {}", msg),
            SchemaError::InvalidMissionSpec(msg) => write!(f, "Invalid MissionExperimentSpec: {}", msg),
            SchemaError::InvalidParameters(msg) => write!(f, "Invalid RunParameters: {}", msg),
            SchemaError::InconsistentTarget(msg) => write!(f, "Inconsistent target: {}", msg),
        }
    }
}

impl std::error::Error for SchemaError {}

pub fn validate_cells(cells: &[Cell]) -> Result<(), SchemaError> {
    let mut seen = HashSet::new();
    for cell in cells {
        if !seen.insert(cell.id.clone()) {
            return Err(SchemaError::DuplicateCellId(cell.id.clone()));
        }
        let lat_ok = cell.lat.is_finite() && (-90.0..=90.0).contains(&cell.lat);
        let lon_ok = cell.lon.is_finite() && (-180.0..=180.0).contains(&cell.lon);
        if !(lat_ok && lon_ok) {
            return Err(SchemaError::InvalidCoordinates(cell.id.clone()));
        }
        if !(cell.resolution_m.is_finite() && cell.resolution_m > 0.0) {
            return Err(SchemaError::InvalidResolution(cell.id.clone()));
        }
        if !(cell.vertical_uncertainty_m.is_finite() && cell.vertical_uncertainty_m >= 0.0) {
            return Err(SchemaError::InvalidUncertainty(cell.id.clone()));
        }
        if cell.feature_uncertainty.values().any(|s| !(s.is_finite() && *s >= 0.0)) {
            return Err(SchemaError::InvalidUncertainty(cell.id.clone()));
        }
        if !(cell.valid_pixel_ratio.is_finite() && (0.0..=1.0).contains(&cell.valid_pixel_ratio)) {
            return Err(SchemaError::InvalidCoverage(cell.id.clone()));
        }
        cell.features.validate()?;
    }
    Ok(())
}

/// Cross-check TargetProfile against the spec. Hard errors for body / id mismatch,
/// warnings for target values that disagree with the spec (the spec is the source of truth).
pub fn check_target_consistency(
    spec: &MissionExperimentSpec,
    target: &TargetProfile,
) -> Result<Vec<String>, SchemaError> {
    if target.body != spec.target_body {
        return Err(SchemaError::InconsistentTarget(format!(
            "spec.target_body {:?} != target.body {:?}",
            spec.target_body, target.body
        )));
    }
    if let Some(id) = &spec.target_profile_id {
        if id != &target.id {
            return Err(SchemaError::InconsistentTarget(format!(
                "spec.target_profile_id '{}' != target.id '{}'",
                id, target.id
            )));
        }
    }
    if !(target.resolution_m.is_finite() && target.resolution_m > 0.0) {
        return Err(SchemaError::InconsistentTarget("target resolution must be > 0".into()));
    }
    let mut warnings = Vec::new();
    for r in &spec.required_parameters {
        let tv = target.features.get(r.feature);
        if !r.within_range(tv) {
            warnings.push(format!(
                "TargetProfile {:?} = {:.3} lies outside the spec acceptable range of {} (spec is used for ranking).",
                r.feature, tv, r.requirement_id
            ));
        }
    }
    Ok(warnings)
}
