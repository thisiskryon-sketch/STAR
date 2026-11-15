// star-cli/src/synth.rs
//! Synthetic demo bundle. EVERYTHING here is synthetic: it exercises the logic of the
//! pipeline and must never be presented as real analog sites or real evidence.
use star_core::assessment::{
    EvidenceLevel, LabFacility, SiteWindow, TemporalAnalogWindow, MechanismEvidence, SiteMechanismEvidence, ValidatedCorrectionModel,
};
use star_core::schema::{
    CelestialBody, Cell, Criticality, DataOrigin, DemFeatures, FeatureId, HardConstraint, KnownAnalogSite,
    MechanismSpec, MissionExperimentSpec, ParameterRequirement, ProvenanceMetadata, TargetProfile,
    TestProfile, TestVenue, TransferRequirement,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownAnalogLabels {
    pub target_id: String,
    /// Regions that are known analogs FOR THIS TARGET/EXPERIMENT (not generic analogs).
    pub positive_regions: Vec<String>,
    pub negative_control_regions: Vec<String>,
}

pub struct SyntheticTestBundle {
    pub spec: MissionExperimentSpec,
    #[cfg_attr(not(test), allow(dead_code))]
    pub target: TargetProfile,
    /// Additional lunar regions for the multi-target assessment (SYNTHETIC values).
    pub targets: Vec<TargetProfile>,
    pub cells: Vec<Cell>,
    pub catalog: Vec<KnownAnalogSite>,
    pub labels: KnownAnalogLabels,
    pub evidence: Vec<SiteMechanismEvidence>,
    pub labs: Vec<LabFacility>,
    pub site_windows: Vec<SiteWindow>,
}

/// splitmix64: independent, reproducible pseudo-random stream.
pub fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn uniform01(seed: u64) -> f64 {
    (splitmix64(seed) >> 11) as f64 / (1u64 << 53) as f64
}

fn req(id: &str, f: FeatureId, t: f64, lo: f64, hi: f64, max_u: f64, c: Criticality) -> ParameterRequirement {
    ParameterRequirement {
        requirement_id: id.into(),
        feature: f,
        target_value: t,
        acceptable_min: Some(lo),
        acceptable_max: Some(hi),
        max_uncertainty: Some(max_u),
        weight: 1.0,
        unit: f.unit().into(),
        criticality: c,
    }
}

fn feats(s: f64, r: f64, sh: f64) -> DemFeatures {
    DemFeatures {
        rms_slope_1x: s,
        rms_slope_3x: s * 0.85,
        rms_slope_9x: s * 0.65,
        roughness_1x: r,
        roughness_3x: r * 1.4,
        roughness_9x: r * 2.2,
        shadow_obstruction_1deg: sh,
        shadow_obstruction_2deg: sh * 0.7,
        shadow_obstruction_3deg: sh * 0.45,
    }
}

fn sigma(slope: f64, rough: f64, shadow: f64) -> BTreeMap<FeatureId, f64> {
    let mut m = BTreeMap::new();
    for f in FeatureId::ALL {
        let v = match f.family() {
            star_core::schema::FeatureFamily::TerrainSlope => slope,
            star_core::schema::FeatureFamily::TerrainRoughness => rough,
            star_core::schema::FeatureFamily::IlluminationShadow => shadow,
        };
        m.insert(f, v);
    }
    m
}

#[allow(clippy::too_many_arguments)]
fn cell(
    id: String,
    region: &str,
    lat: f64,
    lon: f64,
    f: DemFeatures,
    unc_m: f64,
    vpr: f64,
    res: f64,
    sig: BTreeMap<FeatureId, f64>,
) -> Cell {
    Cell {
        id,
        region: region.into(),
        lat,
        lon,
        resolution_m: res,
        valid_pixel_ratio: vpr,
        vertical_uncertainty_m: unc_m,
        features: f,
        provenance: Some(ProvenanceMetadata {
            source_id: "SYNTHETIC_GENERATOR".into(),
            source_version: "star-synth-4.1".into(),
            processing_version: "none".into(),
            observation_window_utc: None,
        }),
        feature_uncertainty: sig,
        data_origin: DataOrigin::Synthetic,
    }
}

pub fn build_synthetic_test_bundle() -> SyntheticTestBundle {
    let mechanisms = vec![
        MechanismSpec {
            mechanism_id: "granular_regolith_shear".into(),
            description: "Wheel-soil shear in loose granular material".into(),
            venues: vec![TestVenue::FieldSite, TestVenue::Laboratory],
            field_min_abs_latitude_deg: None,
            note: "Volcanic tephra fields and simulant soil bins approximate shear; 1/6 g needs a correction model.".into(),
            requires_field_time_window: false,
        },
        MechanismSpec {
            mechanism_id: "sustained_low_sun_illumination".into(),
            description: "Multi-hour sun elevation below ~3° with long shadows sweeping in azimuth".into(),
            venues: vec![TestVenue::FieldSite, TestVenue::Laboratory],
            field_min_abs_latitude_deg: Some(66.0),
            note: "Declared condition (input data): sustained in the field only at high latitude, in specific seasonal windows.".into(),
            requires_field_time_window: true,
        },
        MechanismSpec {
            mechanism_id: "high_vacuum_radiative_transfer".into(),
            description: "Radiative-only heat exchange and dust behaviour in high vacuum".into(),
            venues: vec![TestVenue::Laboratory],
            field_min_abs_latitude_deg: None,
            note: "Only thermal-vacuum chambers; no field site has vacuum.".into(),
            requires_field_time_window: false,
        },
        MechanismSpec {
            mechanism_id: "sustained_lunar_gravity".into(),
            description: "Continuous 1/6 g loading of soil and vehicle".into(),
            venues: vec![],
            field_min_abs_latitude_deg: None,
            note: "Parabolic flights give seconds only; sustained 1/6 g is irreducible on Earth.".into(),
            requires_field_time_window: false,
        },
    ];

    let spec = MissionExperimentSpec {
        mission_id: "DEMO_LUNAR_SOUTH_POLE".into(),
        experiment_id: "EXP_ROVER_LOWSUN_MOBILITY_01".into(),
        target_body: CelestialBody::Moon,
        target_region: "Shackleton_Connecting_Ridge".into(),
        target_profile_id: Some("LUNAR_SP_SHACKLETON_01".into()),
        objective: "Rover traction and navigation on rough slopes under low sun (synthetic demo)".into(),
        fallback_profile: TestProfile::P3CombinedTerrainShadow,
        required_parameters: vec![
            req("REQ_SLOPE_1X", FeatureId::RmsSlope1x, 11.5, 7.0, 16.0, 1.5, Criticality::High),
            req("REQ_ROUGH_1X", FeatureId::Roughness1x, 0.35, 0.15, 0.60, 0.10, Criticality::Normal),
            req("REQ_SHADOW_1DEG", FeatureId::ShadowObstruction1Deg, 0.62, 0.40, 0.85, 0.10, Criticality::High),
        ],
        hard_constraints: vec![HardConstraint {
            constraint_id: "HC_SAFE_SLOPE".into(),
            feature: FeatureId::RmsSlope1x,
            min_allowed: Some(2.0),
            max_allowed: Some(24.0),
            max_resolution_mismatch_ratio: Some(4.0),
        }],
        transfer_requirements: vec![
            TransferRequirement {
                requirement_id: "TR_TRACTION".into(),
                engineering_decision: "Wheel slip threshold calibration".into(),
                required_mechanism_id: "granular_regolith_shear".into(),
                requires_validated_correction_model: true,
                depends_on_parameters: vec!["REQ_SLOPE_1X".into(), "REQ_ROUGH_1X".into()],
                weight: 1.0,
                criticality: Criticality::MissionCritical,
            },
            TransferRequirement {
                requirement_id: "TR_LOWSUN_NAV".into(),
                engineering_decision: "Navigation camera / LiDAR performance in long shadows".into(),
                required_mechanism_id: "sustained_low_sun_illumination".into(),
                requires_validated_correction_model: false,
                depends_on_parameters: vec!["REQ_SHADOW_1DEG".into()],
                weight: 1.0,
                criticality: Criticality::MissionCritical,
            },
            TransferRequirement {
                requirement_id: "TR_VACUUM_OPTICS".into(),
                engineering_decision: "Optics and dust behaviour in vacuum".into(),
                required_mechanism_id: "high_vacuum_radiative_transfer".into(),
                requires_validated_correction_model: false,
                depends_on_parameters: vec![],
                weight: 1.0,
                criticality: Criticality::High,
            },
            TransferRequirement {
                requirement_id: "TR_GRAVITY_SINKAGE".into(),
                engineering_decision: "Sinkage and slip under sustained 1/6 g".into(),
                required_mechanism_id: "sustained_lunar_gravity".into(),
                requires_validated_correction_model: true,
                depends_on_parameters: vec![],
                weight: 1.0,
                criticality: Criticality::MissionCritical,
            },
        ],
        mechanisms,
    };

    let target = TargetProfile {
        id: "LUNAR_SP_SHACKLETON_01".into(),
        body: CelestialBody::Moon,
        region_name: "Shackleton_Connecting_Ridge".into(),
        test_profile: TestProfile::P3CombinedTerrainShadow,
        resolution_m: 2.0,
        vertical_uncertainty_m: 0.35,
        features: feats(11.5, 0.35, 0.62),
    };

    let mut cells = Vec::new();
    let s_good = || sigma(0.8, 0.04, 0.03);
    // Known analogs for this experiment: terrain matches, but mid latitude (no sustained low sun).
    for i in 0..4 {
        let k = i as f64;
        cells.push(cell(format!("KAM_POS_{}", i + 1), "Kamchatka_Volcanic", 55.80 + 0.15 * k, 160.30 + 0.15 * k,
            feats(11.2 + 0.25 * k, 0.33 + 0.02 * k, 0.60 + 0.012 * k), 0.45, 0.96, 2.0, s_good()));
    }
    for i in 0..3 {
        let k = i as f64;
        // No per-feature uncertainty supplied for Iceland: it stays Unknown, never 0.
        cells.push(cell(format!("ICE_POS_{}", i + 1), "Iceland_Highlands", 64.80 + 0.15 * k, -16.80 - 0.15 * k,
            feats(12.0 - 0.3 * k, 0.37, 0.64 - 0.01 * k), 0.50, 0.94, 2.0, BTreeMap::new()));
    }
    // High-arctic site: low-sun geometry and shadows, but terrain too gentle for traction tests.
    for i in 0..3 {
        let k = i as f64;
        cells.push(cell(format!("DEVON_{}", i + 1), "Devon_Island_Arctic", 75.30 + 0.12 * k, -89.20 - 0.4 * k,
            feats(5.6 + 0.2 * k, 0.12, 0.58 + 0.01 * k), 0.40, 0.95, 2.0, s_good()));
    }
    // Uncatalogued, mission-feasible combination that is rare in this dataset.
    for i in 0..3 {
        let k = i as f64;
        cells.push(cell(format!("PUNA_{}", i + 1), "Puna_Plateau", -24.20 - 0.15 * k, -67.60 + 0.15 * k,
            feats(14.6 + 0.2 * k, 0.55, 0.45 + 0.01 * k), 0.50, 0.95, 2.0, s_good()));
    }
    // Generic terrain population (what Earth mostly looks like).
    for i in 0..40u64 {
        let u = |j: u64| uniform01(1000 + i * 16 + j);
        let region = ["Generic_Hills_A", "Generic_Hills_B", "Generic_Hills_C", "Generic_Hills_D"][(i % 4) as usize];
        cells.push(cell(format!("GEN_{:02}", i + 1), region, 40.0 + 10.0 * u(0), -100.0 + 40.0 * u(1),
            feats(3.0 + 5.0 * u(2), 0.05 + 0.10 * u(3), 0.05 + 0.20 * u(4)), 0.5, 0.93, 2.0, s_good()));
    }
    // Ranking negatives: pass QC, physically unsuitable (flat plains).
    for i in 0..4 {
        let k = i as f64;
        cells.push(cell(format!("RANK_NEG_FLAT_{}", i + 1), "Alluvial_Plains", 25.0 + 0.2 * k, 15.0 + 0.2 * k,
            feats(2.5 + 0.2 * k, 0.05, 0.05), 0.30, 0.98, 2.0, s_good()));
    }
    // Isolated artefact-like cell.
    cells.push(cell("OUTLIER_ARTIFACT_1".into(), "Isolated_Spire", -23.5, -67.8,
        feats(23.5, 1.45, 0.92), 1.8, 0.86, 2.0, sigma(2.5, 0.3, 0.1)));
    // QC negatives with exact rejection reasons.
    let t = target.features.clone();
    cells.push(cell("QC_FAIL_LOW_COVERAGE".into(), "Cloudy_Basin", 10.0, 20.0, t.clone(), 0.5, 0.55, 2.0, s_good()));
    cells.push(cell("QC_FAIL_RESOLUTION".into(), "Coarse_Global_Grid", 12.0, 22.0, t.clone(), 0.5, 0.99, 30.0, s_good()));
    cells.push(cell("HC_FAIL_STEEP_CLIFF".into(), "Steep_Canyon", 36.0, -112.0,
        DemFeatures { rms_slope_1x: 34.0, ..t }, 0.5, 0.95, 2.0, s_good()));

    let catalog = vec![
        KnownAnalogSite { id: "CAT_TOLBACHIK".into(), name: "Tolbachik volcanic field".into(), lat: 55.83, lon: 160.33,
            cluster_radius_km: 80.0, features: feats(11.0, 0.32, 0.58) },
        KnownAnalogSite { id: "CAT_HAUGHTON".into(), name: "Haughton crater, Devon Island".into(), lat: 75.4, lon: -89.7,
            cluster_radius_km: 30.0, features: feats(6.0, 0.12, 0.55) },
        KnownAnalogSite { id: "CAT_ICELAND".into(), name: "Icelandic highlands".into(), lat: 64.9, lon: -16.9,
            cluster_radius_km: 40.0, features: feats(12.0, 0.36, 0.63) },
    ];

    let mut evidence = Vec::new();
    for i in 1..=4 {
        let mut bounds = BTreeMap::new();
        bounds.insert(FeatureId::RmsSlope1x, (5.0, 20.0));
        bounds.insert(FeatureId::Roughness1x, (0.1, 0.8));
        evidence.push(SiteMechanismEvidence {
            cell_id: format!("KAM_POS_{i}"),
            mechanisms: vec![MechanismEvidence {
                mechanism_id: "granular_regolith_shear".into(),
                level: EvidenceLevel::Literature,
                source: "SYNTHETIC PLACEHOLDER - replace with a real reference".into(),
                synthetic_placeholder: true,
                valid_feature_bounds: [(FeatureId::RmsSlope1x, (5.0, 20.0)), (FeatureId::Roughness1x, (0.1, 0.8))].into_iter().collect(),
                independent_validation: false,
            }],
            validated_correction_models: vec![ValidatedCorrectionModel {
                model_id: "SYNTH_GRAVITY_SLIP_CORRECTION".into(),
                mechanism_id: "granular_regolith_shear".into(),
                citation_or_source: "SYNTHETIC PLACEHOLDER - not a real model".into(),
                evidence_level: EvidenceLevel::Literature,
                valid_feature_bounds: bounds,
                residual_relative_uncertainty: 0.15,
                synthetic_placeholder: true,
            }],
        });
    }
    for i in 1..=3 {
        evidence.push(SiteMechanismEvidence {
            cell_id: format!("DEVON_{i}"),
            mechanisms: vec![MechanismEvidence {
                mechanism_id: "sustained_low_sun_illumination".into(),
                level: EvidenceLevel::Literature,
                source: "SYNTHETIC PLACEHOLDER - replace with a real reference".into(),
                synthetic_placeholder: true,
                valid_feature_bounds: [(FeatureId::ShadowObstruction1Deg, (0.3, 0.9))].into_iter().collect(),
                independent_validation: false,
            }],
            validated_correction_models: vec![],
        });
    }

    let labs = vec![
        LabFacility {
            facility_id: "TVAC_DUSTY_OPTICS".into(),
            name: "Thermal-vacuum chamber with dust (placeholder)".into(),
            reproduces_mechanisms: vec!["high_vacuum_radiative_transfer".into()],
            reproduces_features: vec![],
            cost_units: Some(25.0),
            duration_days: Some(5.0),
            synthetic_placeholder: true,
            transfer_evidence: vec![MechanismEvidence {
                mechanism_id: "high_vacuum_radiative_transfer".into(),
                level: EvidenceLevel::LabValidated,
                source: "SYNTHETIC PLACEHOLDER - replace with a real reference".into(),
                synthetic_placeholder: true,
                valid_feature_bounds: BTreeMap::new(),
                independent_validation: false,
            }],
        },
        LabFacility {
            facility_id: "SOILBIN_TILTBED".into(),
            name: "Simulant soil bin with tilt bed (placeholder)".into(),
            reproduces_mechanisms: vec!["granular_regolith_shear".into()],
            reproduces_features: vec![FeatureId::RmsSlope1x],
            cost_units: Some(20.0),
            duration_days: Some(7.0),
            synthetic_placeholder: true,
            transfer_evidence: vec![MechanismEvidence {
                mechanism_id: "granular_regolith_shear".into(),
                level: EvidenceLevel::Literature,
                source: "SYNTHETIC PLACEHOLDER - replace with a real reference".into(),
                synthetic_placeholder: true,
                valid_feature_bounds: BTreeMap::new(),
                independent_validation: false,
            }],
        },
        LabFacility {
            facility_id: "LOWSUN_LIGHT_RIG".into(),
            name: "Low-angle collimated light rig (placeholder)".into(),
            reproduces_mechanisms: vec!["sustained_low_sun_illumination".into()],
            reproduces_features: vec![],
            cost_units: Some(10.0),
            duration_days: Some(3.0),
            synthetic_placeholder: true,
            transfer_evidence: vec![],
        },
    ];

    let labels = KnownAnalogLabels {
        target_id: target.id.clone(),
        positive_regions: vec!["Kamchatka_Volcanic".into(), "Iceland_Highlands".into()],
        negative_control_regions: vec!["Alluvial_Plains".into()],
    };

    // Other south-polar regions for the multi-target assessment. Names are real Artemis III
    // candidate regions, VALUES ARE SYNTHETIC placeholders to be replaced by LOLA-derived features.
    let synth_target = |id: &str, name: &str, f: DemFeatures| TargetProfile {
        id: id.into(),
        body: CelestialBody::Moon,
        region_name: format!("{name} (SYNTHETIC values)"),
        test_profile: TestProfile::P3CombinedTerrainShadow,
        resolution_m: 2.0,
        vertical_uncertainty_m: 0.35,
        features: f,
    };
    let targets = vec![
        target.clone(),
        synth_target("SYNTH_MALAPERT", "Malapert Massif", feats(15.0, 0.50, 0.70)),
        synth_target("SYNTH_DE_GERLACHE_RIM", "de Gerlache Rim", feats(9.0, 0.25, 0.50)),
        synth_target("SYNTH_HAWORTH", "Haworth", feats(21.0, 0.85, 0.90)),
    ];

    // Declared low-sun windows at the arctic site (SYNTHETIC dates: sun skimming the horizon for
    // hours around the equinoxes). Kamchatka/Iceland have no declared window.
    let w = |id: &str, a: &str, b: &str| SiteWindow {
        region: "Devon_Island_Arctic".into(),
        mechanism_id: "sustained_low_sun_illumination".into(),
        window: TemporalAnalogWindow {
            window_id: id.into(),
            start_utc: a.into(),
            end_utc: b.into(),
            season_or_solar_regime: "Sun elevation below ~3° for several hours (SYNTHETIC window)".into(),
            illumination_geometry_valid: true,
        },
    };
    let site_windows = vec![
        w("DEVON_SPRING_2027", "2027-03-05T00:00:00Z", "2027-04-05T00:00:00Z"),
        w("DEVON_AUTUMN_2027", "2027-09-08T00:00:00Z", "2027-10-08T00:00:00Z"),
    ];

    SyntheticTestBundle { spec, target, targets, cells, catalog, labels, evidence, labs, site_windows }
}
