// Acceptance tests for the S.T.A.R. core (technical spec v4, §11 criteria + audit regressions).
use star_core::assessment::*;
use star_core::dna::build_analog_dna;
use star_core::envelope::*;
use star_core::novelty::*;
use star_core::pareto::*;
use star_core::schema::*;
use star_core::*;
use std::collections::BTreeMap;

fn feats(s: f64, r: f64, sh: f64) -> DemFeatures {
    DemFeatures {
        rms_slope_1x: s, rms_slope_3x: s * 0.8, rms_slope_9x: s * 0.6,
        roughness_1x: r, roughness_3x: r * 1.2, roughness_9x: r * 1.5,
        shadow_obstruction_1deg: sh, shadow_obstruction_2deg: sh * 0.7, shadow_obstruction_3deg: sh * 0.4,
    }
}

fn sig(s: f64, r: f64, sh: f64) -> BTreeMap<FeatureId, f64> {
    FeatureId::ALL
        .iter()
        .map(|f| (*f, match f.family() { FeatureFamily::TerrainSlope => s, FeatureFamily::TerrainRoughness => r, _ => sh }))
        .collect()
}

fn cell(id: &str, region: &str, lat: f64, lon: f64, f: DemFeatures) -> Cell {
    Cell {
        id: id.into(), region: region.into(), lat, lon, resolution_m: 2.0, valid_pixel_ratio: 0.95,
        vertical_uncertainty_m: 0.4, features: f, provenance: None,
        feature_uncertainty: sig(0.5, 0.03, 0.02), data_origin: DataOrigin::Synthetic,
    }
}

fn req(id: &str, f: FeatureId, t: f64, lo: f64, hi: f64) -> ParameterRequirement {
    ParameterRequirement { requirement_id: id.into(), feature: f, target_value: t, acceptable_min: Some(lo),
        acceptable_max: Some(hi), max_uncertainty: None, weight: 1.0, unit: f.unit().into(), criticality: Criticality::Normal }
}

fn mech(id: &str, venues: Vec<TestVenue>, lat: Option<f64>) -> MechanismSpec {
    MechanismSpec { mechanism_id: id.into(), description: id.into(), venues, field_min_abs_latitude_deg: lat, note: String::new(), requires_field_time_window: false }
}

fn tr(id: &str, mech: &str, corr: bool, deps: &[&str]) -> TransferRequirement {
    TransferRequirement { requirement_id: id.into(), engineering_decision: id.into(), required_mechanism_id: mech.into(),
        requires_validated_correction_model: corr, depends_on_parameters: deps.iter().map(|s| s.to_string()).collect(), weight: 1.0,
        criticality: Criticality::MissionCritical }
}

fn spec() -> MissionExperimentSpec {
    MissionExperimentSpec {
        mission_id: "M".into(), experiment_id: "E".into(), target_body: CelestialBody::Moon,
        target_region: "SP".into(), target_profile_id: Some("T".into()), objective: "test".into(),
        fallback_profile: TestProfile::P3CombinedTerrainShadow,
        required_parameters: vec![
            req("R_SLOPE", FeatureId::RmsSlope1x, 10.0, 6.0, 14.0),
            req("R_ROUGH", FeatureId::Roughness1x, 0.4, 0.2, 0.6),
            req("R_SHADOW", FeatureId::ShadowObstruction1Deg, 0.6, 0.4, 0.8),
        ],
        hard_constraints: vec![HardConstraint { constraint_id: "HC".into(), feature: FeatureId::RmsSlope1x,
            min_allowed: None, max_allowed: Some(25.0), max_resolution_mismatch_ratio: None }],
        transfer_requirements: vec![
            tr("TR_SHEAR", "shear", false, &["R_SLOPE", "R_ROUGH"]),
            tr("TR_LOWSUN", "lowsun", false, &["R_SHADOW"]),
            tr("TR_VAC", "vacuum", false, &[]),
            tr("TR_GRAV", "gravity", true, &[]),
        ],
        mechanisms: vec![
            mech("shear", vec![TestVenue::FieldSite, TestVenue::Laboratory], None),
            mech("lowsun", vec![TestVenue::FieldSite, TestVenue::Laboratory], Some(66.0)),
            mech("vacuum", vec![TestVenue::Laboratory], None),
            mech("gravity", vec![], None),
        ],
    }
}

fn target() -> TargetProfile {
    TargetProfile { id: "T".into(), body: CelestialBody::Moon, region_name: "SP".into(),
        test_profile: TestProfile::P3CombinedTerrainShadow, resolution_m: 2.0, vertical_uncertainty_m: 0.3,
        features: feats(10.0, 0.4, 0.6) }
}

fn manifest() -> RunManifest {
    RunManifest { run_id: "r".into(), timestamp_utc: "t".into(), code_revision: "c".into(), dataset_hash: "h".into(),
        calibration_version: "v".into(), random_seed: 1, is_synthetic_demo: true, validation_protocol: "p".into() }
}

fn no_limits() -> CampaignConstraints {
    CampaignConstraints { max_budget_units: None, max_total_duration_days: None, max_assets: None, period_start_utc: None, period_end_utc: None }
}

/// Dense synthetic world: good analogs at mid latitude, an arctic low-sun site, generic hills, flat plains.
fn world() -> Vec<Cell> {
    let mut v = Vec::new();
    for i in 0..4 { let k = i as f64; v.push(cell(&format!("GOOD_{i}"), "Good", 50.0 + 0.2 * k, 10.0, feats(10.0 + 0.3 * k, 0.4, 0.6))); }
    for i in 0..3 { let k = i as f64; v.push(cell(&format!("ARCTIC_{i}"), "Arctic", 75.0 + 0.2 * k, 0.0, feats(5.0, 0.15, 0.62))); }
    for i in 0..25 { let k = i as f64; v.push(cell(&format!("HILL_{i:02}"), "Hills", 30.0 + 0.3 * k, -50.0, feats(3.0 + 0.2 * k, 0.05 + 0.004 * k, 0.1 + 0.01 * k))); }
    for i in 0..4 {
        let mut c = cell(&format!("FLAT_{i}"), "Flats", 20.0 + 0.2 * i as f64, 20.0, feats(1.0, 0.02, 0.02));
        c.valid_pixel_ratio = 1.0; c.vertical_uncertainty_m = 0.0;
        v.push(c);
    }
    v
}

fn run(cells: &[Cell], catalog: &[KnownAnalogSite], evidence: &[SiteMechanismEvidence], labs: &[LabFacility]) -> DiscoveryReport {
    let (p, s, t, c) = (RunParameters::default(), spec(), target(), no_limits());
    discover_candidates_v4(DiscoveryInputs { manifest: manifest(), params: &p, spec: &s, target: &t, cells,
        catalog, precomputed_scales: None, evidence, labs, site_windows: &[], campaign_constraints: &c }).unwrap()
}

// ----------------------------------------------------------------- distance

#[test]
fn identical_values_give_zero_distance_whatever_the_uncertainty() {
    let s = spec();
    for sg in [0.0, 0.5, 5.0] {
        let md = mission_distance(&s, &feats(10.0, 0.4, 0.6), &sig(sg, sg, sg));
        assert_eq!(md.d, 0.0, "uncertainty must not create a distance floor");
        assert!(md.sigma_d.unwrap() >= 0.0);
    }
    assert!(mission_distance(&s, &feats(10.0, 0.4, 0.6), &BTreeMap::new()).sigma_d.is_none(), "unknown σ stays None");
}

#[test]
fn z_le_one_iff_inside_acceptable_range() {
    let r = req("R", FeatureId::RmsSlope1x, 10.0, 6.0, 16.0);
    for (x, inside) in [(6.0, true), (16.0, true), (5.9, false), (16.1, false), (12.0, true)] {
        assert_eq!(r.within_range(x), inside);
        assert_eq!(r.z(x) <= 1.0 + 1e-12, inside, "x = {x}");
    }
}

#[test]
fn multiscale_requirements_of_one_family_share_its_weight() {
    let mut s = spec();
    s.required_parameters = vec![
        req("S1", FeatureId::RmsSlope1x, 10.0, 6.0, 14.0),
        req("S3", FeatureId::RmsSlope3x, 8.0, 4.8, 11.2),
        req("S9", FeatureId::RmsSlope9x, 6.0, 3.6, 8.4),
        req("R1", FeatureId::Roughness1x, 0.4, 0.2, 0.6),
    ];
    let md = mission_distance(&s, &feats(10.0, 0.4, 0.6), &BTreeMap::new());
    let w: BTreeMap<_, _> = md.per_requirement.iter().map(|r| (r.requirement_id.clone(), r.effective_weight)).collect();
    assert!((w["S1"] + w["S3"] + w["S9"] - w["R1"]).abs() < 1e-12);
}

// ----------------------------------------------------------------- envelope

#[test]
fn marginal_ok_but_joint_unobserved_is_detected() {
    // Steep+smooth and gentle+rough exist; steep+rough (the target) never co-occurs.
    let cells = vec![
        cell("A", "X", 10.0, 0.0, feats(10.0, 0.05, 0.6)),
        cell("B", "Y", 20.0, 0.0, feats(2.0, 0.4, 0.6)),
        cell("C", "Z", 30.0, 0.0, feats(10.5, 0.06, 0.62)),
    ];
    let refs: Vec<&Cell> = cells.iter().collect();
    let env = evaluate_earth_envelope(&spec(), &refs, &RunParameters::default());
    assert!(env.marginal.iter().all(|m| m.cells_within_range > 0));
    assert_eq!(env.joint_status, JointReachStatus::MarginalOnlyJointUnobserved);
    assert_eq!(env.best_partial_joint_sets[0].requirement_ids, vec!["R_SLOPE".to_string(), "R_SHADOW".to_string()]);
}

#[test]
fn joint_support_needs_independent_sites() {
    let p = RunParameters::default();
    let same_site = vec![cell("A", "X", 10.0, 0.0, feats(10.0, 0.4, 0.6)), cell("B", "X", 10.0, 0.001, feats(10.1, 0.4, 0.6))];
    let refs: Vec<&Cell> = same_site.iter().collect();
    assert!(matches!(evaluate_earth_envelope(&spec(), &refs, &p).joint_status, JointReachStatus::JointObservedSingleSite { .. }));
    let two_sites = vec![cell("A", "X", 10.0, 0.0, feats(10.0, 0.4, 0.6)), cell("B", "Y", 40.0, 0.0, feats(10.1, 0.4, 0.6))];
    let refs: Vec<&Cell> = two_sites.iter().collect();
    assert!(matches!(evaluate_earth_envelope(&spec(), &refs, &p).joint_status,
        JointReachStatus::JointObservedWithSupport { independent_sites: 2, .. }));
}

#[test]
fn requirement_beyond_earth_range_is_outside_marginal_envelope() {
    let cells = vec![cell("A", "X", 10.0, 0.0, feats(2.0, 0.4, 0.6))];
    let refs: Vec<&Cell> = cells.iter().collect();
    match evaluate_earth_envelope(&spec(), &refs, &RunParameters::default()).joint_status {
        JointReachStatus::OutsideMarginalEnvelope { unreachable_requirements } => assert_eq!(unreachable_requirements, vec!["R_SLOPE"]),
        s => panic!("{s:?}"),
    }
    assert_eq!(evaluate_earth_envelope(&spec(), &[], &RunParameters::default()).joint_status, JointReachStatus::NoValidEarthData);
}

#[test]
fn mechanism_envelope_separates_field_lab_and_irreducible() {
    let me = evaluate_mechanism_envelope(&spec());
    assert_eq!(me.irreducible_mechanisms, vec!["gravity"]);
    assert!(!me.single_venue_reproduces_everything);
    assert_eq!(me.venues_hosting_all_reducible, vec![TestVenue::Laboratory]);
    let mut s = spec();
    s.mechanisms.retain(|m| m.mechanism_id != "vacuum");
    assert_eq!(evaluate_mechanism_envelope(&s).unknown_mechanisms, vec!["vacuum"], "missing registry entry is Unknown, not reproducible");
}

// ----------------------------------------------------------------- AET

fn aet_for(c: &Cell, ev: Option<&SiteMechanismEvidence>) -> AetAssessment {
    let s = spec();
    let md = mission_distance(&s, &c.features, &c.feature_uncertainty);
    evaluate_aet_transferability(&s, c, &build_analog_dna(&c.id, &md), ev)
}

fn status(a: &AetAssessment, id: &str) -> TransferabilityStatus {
    a.findings.iter().find(|f| f.requirement_id == id).unwrap().status
}

#[test]
fn aet_physics_first_lab_only_and_irreducible_are_not_transferable_from_field() {
    let c = cell("G", "Good", 50.0, 10.0, feats(10.0, 0.4, 0.6));
    let a = aet_for(&c, None);
    assert_eq!(status(&a, "TR_VAC"), TransferabilityStatus::NotTransferable);
    assert_eq!(status(&a, "TR_GRAV"), TransferabilityStatus::NotTransferable);
    assert!(matches!(a.findings.iter().find(|f| f.requirement_id == "TR_LOWSUN").unwrap().venue_check,
        VenueCheck::OutsideFieldLatitudeBand { .. }));
    assert_eq!(status(&a, "TR_SHEAR"), TransferabilityStatus::Unknown, "no evidence -> Unknown");
    assert_eq!(a.overall_status, TransferabilityStatus::Unknown);
}

#[test]
fn aet_requires_evidence_and_valid_correction_model() {
    let c = cell("G", "Good", 50.0, 10.0, feats(10.0, 0.4, 0.6));
    let mut ev = SiteMechanismEvidence { cell_id: "G".into(), mechanisms: vec![MechanismEvidence {
        mechanism_id: "shear".into(), level: EvidenceLevel::Literature, source: "x".into(), synthetic_placeholder: true, valid_feature_bounds: BTreeMap::new(), independent_validation: false }],
        validated_correction_models: vec![] };
    // Evidence without a stated validity domain: applicability unproven -> Partial.
    assert_eq!(status(&aet_for(&c, Some(&ev)), "TR_SHEAR"), TransferabilityStatus::PartialTransfer);
    // Domain stated and the site is inside it -> Transferable.
    ev.mechanisms[0].valid_feature_bounds = [(FeatureId::RmsSlope1x, (5.0, 15.0))].into_iter().collect();
    let a = aet_for(&c, Some(&ev));
    assert_eq!(status(&a, "TR_SHEAR"), TransferabilityStatus::Transferable);
    assert!(a.findings.iter().find(|f| f.requirement_id == "TR_SHEAR").unwrap().relies_on_synthetic_placeholder);
    assert_eq!(a.overall_status, TransferabilityStatus::PartialTransfer);
    // Site outside the evidence domain -> Unknown (the evidence does not speak for this site).
    ev.mechanisms[0].valid_feature_bounds = [(FeatureId::RmsSlope1x, (12.0, 15.0))].into_iter().collect();
    assert_eq!(status(&aet_for(&c, Some(&ev)), "TR_SHEAR"), TransferabilityStatus::Unknown);
    ev.mechanisms[0].valid_feature_bounds = [(FeatureId::RmsSlope1x, (5.0, 15.0))].into_iter().collect();
    ev.mechanisms[0].level = EvidenceLevel::ExpertJudgement;
    assert_eq!(status(&aet_for(&c, Some(&ev)), "TR_SHEAR"), TransferabilityStatus::PartialTransfer);
}

#[test]
fn aet_observable_dependency_outside_range_blocks_transfer() {
    let c = cell("F", "Flats", 50.0, 10.0, feats(1.0, 0.02, 0.6));
    let ev = SiteMechanismEvidence { cell_id: "F".into(), mechanisms: vec![MechanismEvidence {
        mechanism_id: "shear".into(), level: EvidenceLevel::FieldValidated, source: "x".into(), synthetic_placeholder: false, valid_feature_bounds: BTreeMap::new(), independent_validation: false }],
        validated_correction_models: vec![] };
    assert_eq!(status(&aet_for(&c, Some(&ev)), "TR_SHEAR"), TransferabilityStatus::NotTransferable);
}

// ----------------------------------------------------------------- E.C.O.

fn asset(id: &str, field: bool, covers: &[&str], cost: Option<f64>) -> CampaignAssetCapability {
    asset_s(id, field, covers, cost, CoverageStatus::Confirmed)
}

fn asset_s(id: &str, field: bool, covers: &[&str], cost: Option<f64>, st: CoverageStatus) -> CampaignAssetCapability {
    CampaignAssetCapability {
        asset_id: id.into(),
        kind: if field { CampaignAssetKind::FieldAnalogSite { cell_id: id.into(), region: id.into() } }
              else { CampaignAssetKind::LaboratoryFacility { facility_id: id.into() } },
        time_window: None,
        coverage: covers.iter().map(|r| CoverageClaim { requirement_id: r.to_string(), status: st, basis: "t".into(), missing_for_confirmation: vec![] }).collect(),
        estimated_cost_units: cost, duration_days: None, synthetic_placeholder: true,
    }
}

fn creq(id: &str, irreducible: bool) -> CampaignRequirement {
    CampaignRequirement { requirement_id: id.into(), weight: 1.0, criticality: Criticality::Normal, irreducible }
}

fn lab_ev(m: &str) -> MechanismEvidence {
    MechanismEvidence { mechanism_id: m.into(), level: EvidenceLevel::LabValidated, source: "x".into(), synthetic_placeholder: true,
        valid_feature_bounds: BTreeMap::new(), independent_validation: false }
}

#[test]
fn eco_portfolio_beats_single_best_site_and_reports_irreducible() {
    let reqs = vec![creq("A", false), creq("B", false), creq("C", false), creq("G", true)];
    let assets = vec![asset("SITE1", true, &["A", "B"], None), asset("SITE2", true, &["C"], None), asset("LAB", false, &["C"], None)];
    let p = optimize_campaign_portfolio(&reqs, &assets, &no_limits(), 16);
    assert_eq!(p.status, PortfolioOptimizationStatus::AllReducibleConfirmed);
    assert_eq!(p.cost_model, CostModel::UnknownCostsMinimisingAssetCount);
    assert_eq!(p.irreducible_requirements, vec!["G"]);
    assert_eq!(p.selected_assets.len(), 2);
    assert!(p.single_best_site_baseline.unwrap().confirmed_coverage < p.confirmed_coverage);
    assert_eq!(p.critical_unconfirmed, Vec::<String>::new());
}

#[test]
fn eco_tie_break_is_independent_of_input_order() {
    let reqs = vec![creq("A", false)];
    let a = vec![asset("ZZZ", false, &["A"], Some(10.0)), asset("AAA", false, &["A"], Some(10.0))];
    let b: Vec<_> = a.iter().rev().cloned().collect();
    let pa = optimize_campaign_portfolio(&reqs, &a, &no_limits(), 16);
    let pb = optimize_campaign_portfolio(&reqs, &b, &no_limits(), 16);
    assert_eq!(pa.selected_assets, vec!["AAA"]);
    assert_eq!(pa.selected_assets, pb.selected_assets);
}

#[test]
fn eco_respects_budget_and_says_what_is_left() {
    let reqs = vec![creq("A", false), creq("B", false)];
    let assets = vec![asset("L1", false, &["A"], Some(10.0)), asset("L2", false, &["B"], Some(50.0))];
    let c = CampaignConstraints { max_budget_units: Some(30.0), max_total_duration_days: None, max_assets: None, period_start_utc: None, period_end_utc: None };
    let p = optimize_campaign_portfolio(&reqs, &assets, &c, 16);
    assert_eq!(p.status, PortfolioOptimizationStatus::PartialCoverageUnderConstraints);
    assert_eq!(p.uncovered_requirements, vec!["B"]);
    assert_eq!(p.total_cost_units, Some(10.0));
    let none = optimize_campaign_portfolio(&reqs, &[], &no_limits(), 16);
    assert_eq!(none.status, PortfolioOptimizationStatus::PortfolioNotOptimizedNoCapabilityData);
}

// ----------------------------------------------------------------- Pareto / novelty

#[test]
fn pareto_never_compares_different_objective_masks() {
    let a = ParetoInput { cell_id: "A".into(), objectives: vec![Some(-0.2), Some(0.5)], tie_key: 0.2 };
    let b = ParetoInput { cell_id: "B".into(), objectives: vec![Some(-0.3), None], tie_key: 0.3 };
    assert!(!dominates(&a, &b) && !dominates(&b, &a));
    let c = ParetoInput { cell_id: "C".into(), objectives: vec![Some(-0.3), Some(0.4)], tie_key: 0.3 };
    assert!(dominates(&a, &c));
}

#[test]
fn spatial_thinning_is_order_independent() {
    let a = cell("A", "R", 0.0, 0.0, feats(1.0, 0.1, 0.1));
    let b = cell("B", "R", 0.0, 0.0072, feats(1.0, 0.1, 0.1));
    let c = cell("C", "R", 0.0, 0.0144, feats(1.0, 0.1, 0.1));
    let ids = |v: Vec<&Cell>| v.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(spatially_independent(&[&a, &b, &c], 1.0)), ids(spatially_independent(&[&b, &a, &c], 1.0)));
}

#[test]
fn rarity_ignores_measurement_uncertainty_and_detects_common_terrain() {
    let cells = world();
    let refs: Vec<&Cell> = cells.iter().collect();
    let scales = compute_feature_scales(&refs, "v").unwrap();
    let p = RunParameters::default();
    let active = spec().active_features();
    let hill = &cells[10];
    let mut noisy = hill.clone();
    noisy.vertical_uncertainty_m = 3.0;
    noisy.feature_uncertainty = sig(5.0, 0.5, 0.3);
    let a = compute_novelty_metrics(hill, &refs, &[], &active, &scales, &p);
    let b = compute_novelty_metrics(&noisy, &refs, &[], &active, &scales, &p);
    assert_eq!(a.rarity_status, RaritySupportStatus::CommonTerrestrialTerrain);
    assert_eq!(a.earth_rarity_knn_distance, b.earth_rarity_knn_distance);
    assert_eq!(a.catalog_novelty_distance, None, "no catalog -> None, never 0");
}

// ----------------------------------------------------------------- pipeline

#[test]
fn pipeline_is_deterministic_under_input_permutation() {
    let cells = world();
    let mut rev = cells.clone();
    rev.reverse();
    let (a, b) = (run(&cells, &[], &[], &[]), run(&rev, &[], &[], &[]));
    let ids = |r: &DiscoveryReport| r.mode_rankings.iter().flat_map(|m| m.entries.iter().map(|e| e.cell_id.clone())).collect::<Vec<_>>();
    assert_eq!(ids(&a), ids(&b));
    assert_eq!(serde_json::to_string(&a.campaign_portfolio).unwrap(), serde_json::to_string(&b.campaign_portfolio).unwrap());
}

#[test]
fn high_quality_negative_controls_never_reach_mission_front_one() {
    let r = run(&world(), &[], &[], &[]);
    let mm = &r.mode_rankings[0];
    assert!(mm.entries.iter().filter(|e| e.pareto_front_rank == 1).all(|e| e.region == "Good"));
    assert!(mm.entries.iter().filter(|e| e.region == "Flats").all(|e| !e.within_epsilon));
}

#[test]
fn pipeline_builds_complementary_campaign_from_dossiers() {
    let ev: Vec<SiteMechanismEvidence> = (0..3).map(|i| SiteMechanismEvidence { cell_id: format!("ARCTIC_{i}"),
        mechanisms: vec![MechanismEvidence { mechanism_id: "lowsun".into(), level: EvidenceLevel::Literature,
        source: "x".into(), synthetic_placeholder: true, valid_feature_bounds: BTreeMap::new(), independent_validation: false }], validated_correction_models: vec![] }).collect();
    let labs = vec![LabFacility { facility_id: "TVAC".into(), name: "tvac".into(), reproduces_mechanisms: vec!["vacuum".into(), "gravity".into()],
        reproduces_features: vec![], cost_units: None, duration_days: None, synthetic_placeholder: true,
        transfer_evidence: vec![lab_ev("vacuum")] },
        LabFacility { facility_id: "BIN".into(), name: "bin".into(), reproduces_mechanisms: vec!["shear".into()],
        reproduces_features: vec![], cost_units: None, duration_days: None, synthetic_placeholder: true,
        transfer_evidence: vec![] }];
    let r = run(&world(), &[], &ev, &labs);
    let p = &r.campaign_portfolio;
    assert_eq!(p.irreducible_requirements, vec!["TR_GRAV"], "a lab claiming gravity must not cover an irreducible mechanism");
    // No evidence for shear anywhere: testable (field + bin) but never Confirmed.
    assert!(p.provisional_requirements.contains(&"TR_SHEAR".to_string()));
    assert!(p.critical_unconfirmed.contains(&"TR_SHEAR".to_string()));
    assert!(p.selected_assets.iter().any(|a| a.starts_with("FIELD:Good")));
    assert!(p.selected_assets.iter().any(|a| a.starts_with("FIELD:Arctic")));
    assert!(p.selected_assets.iter().any(|a| a == "LAB:TVAC"));
    assert_eq!(p.status, PortfolioOptimizationStatus::AllReducibleTestableSomeProvisional);
    assert_eq!(r.mode_rankings[2].status, ModeStatus::NoCatalogProvided);
    let reg = r.residual_risk_register.iter().find(|e| e.requirement_id == "TR_GRAV").unwrap();
    assert_eq!(reg.status, RehearsalStatus::IrreducibleUntilFlight);
    assert!(!r.synthetic_content_warnings.is_empty());
}

#[test]
fn rejections_carry_specific_what_would_change() {
    let mut cells = world();
    cells.push(cell("STEEP", "Cliff", 5.0, 5.0, feats(30.0, 0.4, 0.6)));
    let mut low = cell("LOWCOV", "Cloud", 6.0, 6.0, feats(10.0, 0.4, 0.6));
    low.valid_pixel_ratio = 0.5;
    cells.push(low);
    let r = run(&cells, &[], &[], &[]);
    let steep = r.rejections.iter().find(|x| x.cell_id == "STEEP").unwrap();
    assert!(!steep.stage_qc && steep.what_would_change_decision[0].contains("<= 25"));
    assert!(r.rejections.iter().find(|x| x.cell_id == "LOWCOV").unwrap().stage_qc);
}

// ----------------------------------------------------------------- U.D.D.

#[test]
fn udd_detects_flip_risk_and_unknown_uncertainty() {
    let s = spec();
    let a = build_analog_dna("A", &mission_distance(&s, &feats(10.0, 0.40, 0.60), &sig(1.0, 0.05, 0.05)));
    let b = build_analog_dna("B", &mission_distance(&s, &feats(10.2, 0.41, 0.60), &sig(1.0, 0.05, 0.05)));
    assert!(matches!(build_udd_recommendation(Some(&a), Some(&b), false).flip, FlipAssessment::CanFlip { .. }));
    let far = build_analog_dna("C", &mission_distance(&s, &feats(13.5, 0.58, 0.79), &sig(0.1, 0.01, 0.01)));
    let a_tight = build_analog_dna("A", &mission_distance(&s, &feats(10.0, 0.40, 0.60), &sig(0.1, 0.01, 0.01)));
    assert!(matches!(build_udd_recommendation(Some(&a_tight), Some(&far), false).flip, FlipAssessment::Stable { .. }));
    let unk = build_analog_dna("U", &mission_distance(&s, &feats(10.2, 0.41, 0.60), &BTreeMap::new()));
    let u = build_udd_recommendation(Some(&a), Some(&unk), false);
    assert!(matches!(u.flip, FlipAssessment::UncertaintyUnknown { .. }));
    assert!(u.recommended_measurement.is_some() && u.expected_voi_score.is_none());
}

// ----------------------------------------------------------------- validation of inputs

#[test]
fn invalid_inputs_are_rejected_with_diagnostics() {
    let mut p = RunParameters::default();
    p.knn_k = 0;
    assert!(matches!(p.validate(), Err(SchemaError::InvalidParameters(_))));

    let mut s = spec();
    s.hard_constraints[0].min_allowed = Some(30.0);
    assert!(s.validate().is_err(), "min > max");
    let mut s = spec();
    s.hard_constraints[0].max_allowed = None;
    assert!(s.validate().is_err(), "no bound");
    let mut s = spec();
    s.required_parameters[0].unit = "m".into();
    assert!(s.validate().is_err(), "unit mismatch");
    let mut s = spec();
    s.required_parameters[0].target_value = 20.0;
    assert!(s.validate().is_err(), "target outside acceptable range");
    let mut s = spec();
    s.transfer_requirements[0].depends_on_parameters = vec!["NOPE".into()];
    assert!(s.validate().is_err(), "unknown dependency");

    let mut nan = cell("N", "X", 0.0, 0.0, feats(10.0, 0.4, 0.6));
    nan.features.rms_slope_1x = f64::NAN;
    assert!(validate_cells(&[nan]).is_err());
    let d = cell("D", "X", 0.0, 0.0, feats(10.0, 0.4, 0.6));
    assert_eq!(validate_cells(&[d.clone(), d]), Err(SchemaError::DuplicateCellId("D".into())));

    let mut t = target();
    t.body = CelestialBody::Mars;
    assert!(check_target_consistency(&spec(), &t).is_err());
}


// ----------------------------------------------------------------- decision layer (v0.6)

#[test]
fn lab_testability_is_not_transferability() {
    let reqs = vec![creq("A", false)];
    let p = optimize_campaign_portfolio(&reqs, &[asset_s("LAB", false, &["A"], None, CoverageStatus::Provisional)], &no_limits(), 16);
    assert_eq!(p.confirmed_coverage, 0.0, "being able to run a test is not evidence that its result transfers");
    assert_eq!(p.testable_coverage, 1.0);
    assert_eq!(p.status, PortfolioOptimizationStatus::AllReducibleTestableSomeProvisional);
}

#[test]
fn unknown_feature_uncertainty_gives_provisional_not_confirmed_coverage() {
    let mut cells = world();
    for c in cells.iter_mut().filter(|c| c.region == "Good") {
        c.feature_uncertainty.clear();
    }
    let r = run(&cells, &[], &[], &[]);
    let good = r.campaign_assets.iter().find(|a| a.asset_id.starts_with("FIELD:Good")).unwrap();
    let slope = good.coverage.iter().find(|c| c.requirement_id == "R_SLOPE").unwrap();
    assert_eq!(slope.status, CoverageStatus::Provisional);
    assert!(slope.missing_for_confirmation[0].contains("uncertainty"));
}

#[test]
fn critical_requirements_are_never_traded_for_more_normal_ones() {
    let mut reqs = vec![creq("CRIT", false), creq("N1", false), creq("N2", false), creq("N3", false)];
    reqs[0].criticality = Criticality::MissionCritical;
    let assets = vec![asset("A_CRIT", false, &["CRIT"], None), asset("B_MANY", false, &["N1", "N2", "N3"], None)];
    let c = CampaignConstraints { max_budget_units: None, max_total_duration_days: None, max_assets: Some(1), period_start_utc: None, period_end_utc: None };
    let p = optimize_campaign_portfolio(&reqs, &assets, &c, 16);
    assert_eq!(p.selected_assets, vec!["A_CRIT"]);
}

#[test]
fn unknown_cost_under_a_budget_is_reported_unverifiable() {
    let reqs = vec![creq("A", false)];
    let c = CampaignConstraints { max_budget_units: Some(10.0), max_total_duration_days: None, max_assets: None, period_start_utc: None, period_end_utc: None };
    let p = optimize_campaign_portfolio(&reqs, &[asset("SITE", true, &["A"], None)], &c, 16);
    assert!(matches!(p.budget_check, ConstraintCheck::Unverifiable { .. }));
}

#[test]
fn exact_check_detects_suboptimal_greedy() {
    let reqs: Vec<_> = (1..=6).map(|i| creq(&format!("R{i}"), false)).collect();
    let assets = vec![
        asset("A", false, &["R1", "R2", "R3", "R4"], None),
        asset("B", false, &["R1", "R2", "R5"], None),
        asset("C", false, &["R3", "R4", "R6"], None),
    ];
    let c = CampaignConstraints { max_budget_units: None, max_total_duration_days: None, max_assets: Some(2), period_start_utc: None, period_end_utc: None };
    let p = optimize_campaign_portfolio(&reqs, &assets, &c, 16);
    let x = p.exact_check.unwrap();
    assert!(!x.greedy_is_optimal);
    assert_eq!(x.best_selected_assets, vec!["B", "C"]);
}

#[test]
fn udd_recommends_the_measurement_that_resolves_the_choice() {
    let s = spec();
    // Roughness is noisy at both sites, slope is precise: re-measuring roughness resolves the tie.
    let mut sa = sig(0.05, 0.08, 0.005);
    sa.insert(FeatureId::Roughness1x, 0.08);
    let a = build_analog_dna("A", &mission_distance(&s, &feats(10.0, 0.40, 0.60), &sa));
    let b = build_analog_dna("B", &mission_distance(&s, &feats(10.0, 0.47, 0.60), &sa));
    let u = build_udd_recommendation(Some(&a), Some(&b), true);
    assert!(matches!(u.flip, FlipAssessment::CanFlip { .. }));
    assert_eq!(u.recommended_measurement.as_deref(), Some("R_ROUGH"));
    assert_eq!(u.measurement_can_resolve, Some(true));
    assert!(u.correlation_note.is_some());
}

#[test]
fn lead_site_decision_compares_different_sites_and_resolves_clear_leader() {
    let r = run(&world(), &[], &[], &[]);
    let (a, b) = r.udd_recommendation.decision_between.clone().unwrap();
    assert!(a.starts_with("GOOD") && !b.starts_with("GOOD"), "runner-up must be another site, got {a} vs {b}");
    assert!(matches!(r.decision.lead_site, LeadSiteVerdict::Resolved { ref site, .. } if site == "Good"));
    assert!(r.weight_sensitivity.lead_site_stability.unwrap() >= 0.8);
}

#[test]
fn marginal_hard_constraint_pass_is_flagged_and_not_confirmed() {
    let mut cells = world();
    let mut edge = cell("EDGE", "EdgeSite", -40.0, 100.0, feats(10.0, 0.4, 0.6));
    edge.features.rms_slope_1x = 13.9; // hard max set to 14.5 below, σ = 0.5 -> within 2σ of the bound
    edge.feature_uncertainty.insert(FeatureId::RmsSlope1x, 0.5);
    let mut s = spec();
    s.hard_constraints[0].max_allowed = Some(14.5);
    cells.push(edge);
    let (p, t, c) = (RunParameters::default(), target(), no_limits());
    let r = discover_candidates_v4(DiscoveryInputs { manifest: manifest(), params: &p, spec: &s, target: &t, cells: &cells,
        catalog: &[], precomputed_scales: None, evidence: &[], labs: &[], site_windows: &[], campaign_constraints: &c }).unwrap();
    let d = r.dossiers.iter().find(|d| d.cell_id == "EDGE").unwrap();
    assert!(!d.constraint_margin_warnings.is_empty());
    let a = r.campaign_assets.iter().find(|a| a.asset_id.contains("EDGE")).unwrap();
    assert!(a.coverage.iter().all(|c| c.status == CoverageStatus::Provisional));
}

#[test]
fn per_target_spec_keeps_mission_widths_and_orders_by_residual_risk() {
    let base = spec();
    let mut t2 = target();
    t2.id = "T2".into();
    t2.features = feats(30.0, 2.0, 0.95);
    let mut s2 = spec_for_target(&base, &t2);
    s2.target_profile_id = Some("T2".into());
    let r0 = &base.required_parameters[0];
    let r2 = &s2.required_parameters[0];
    assert_eq!(r2.target_value, 30.0);
    assert!(((r2.acceptable_max.unwrap() - r2.target_value) - (r0.acceptable_max.unwrap() - r0.target_value)).abs() < 1e-9);
    assert!(s2.required_parameters[2].acceptable_max.unwrap() <= 1.0, "shadow range clamped to [0, 1]");
    let cells = world();
    let (p, c) = (RunParameters::default(), no_limits());
    let mut sums: Vec<TargetRehearsalSummary> = [(&base, target()), (&s2, t2)]
        .into_iter()
        .map(|(s, t)| summarize_target(&discover_candidates_v4(DiscoveryInputs { manifest: manifest(), params: &p, spec: s, target: &t,
            cells: &cells, catalog: &[], precomputed_scales: None, evidence: &[], labs: &[], site_windows: &[], campaign_constraints: &c }).unwrap()))
        .collect();
    sums.reverse();
    order_targets_by_residual_risk(&mut sums);
    assert_eq!(sums[0].target_id, "T", "a region Earth can rehearse ranks above one it cannot");
    assert!(matches!(sums[1].joint_status, JointReachStatus::OutsideMarginalEnvelope { .. }));
}


// ----------------------------------------------------------------- v0.6.1: synthetic policy + time windows

fn run_with(cells: &[Cell], s: &MissionExperimentSpec, ev: &[SiteMechanismEvidence], labs: &[LabFacility],
            windows: &[SiteWindow], c: &CampaignConstraints, policy: SyntheticEvidencePolicy) -> DiscoveryReport {
    let mut p = RunParameters::default();
    p.synthetic_evidence_policy = policy;
    discover_candidates_v4(DiscoveryInputs { manifest: manifest(), params: &p, spec: s, target: &target(), cells,
        catalog: &[], precomputed_scales: None, evidence: ev, labs, site_windows: windows, campaign_constraints: c }).unwrap()
}

fn shear_evidence(placeholder: bool) -> Vec<SiteMechanismEvidence> {
    (0..4).map(|i| SiteMechanismEvidence { cell_id: format!("GOOD_{i}"), mechanisms: vec![MechanismEvidence {
        mechanism_id: "shear".into(), level: EvidenceLevel::FieldValidated, source: "x".into(), synthetic_placeholder: placeholder,
        valid_feature_bounds: [(FeatureId::RmsSlope1x, (5.0, 15.0))].into_iter().collect(), independent_validation: true }],
        validated_correction_models: vec![] }).collect()
}

fn real_world() -> Vec<Cell> {
    let mut w = world();
    for c in &mut w { c.data_origin = DataOrigin::ExternalMeasured; }
    w
}

fn claim<'a>(r: &'a DiscoveryReport, asset_prefix: &str, req: &str) -> Option<&'a CoverageClaim> {
    r.campaign_assets.iter().find(|a| a.asset_id.starts_with(asset_prefix))?.coverage.iter().find(|c| c.requirement_id == req)
}

#[test]
fn synthetic_field_evidence_is_never_confirmed_under_strict_policy() {
    let (s, c) = (spec(), no_limits());
    let strict = run_with(&real_world(), &s, &shear_evidence(true), &[], &[], &c, SyntheticEvidencePolicy::Strict);
    let cl = claim(&strict, "FIELD:Good", "TR_SHEAR").unwrap();
    assert_eq!(cl.status, CoverageStatus::Provisional);
    assert!(cl.missing_for_confirmation.iter().any(|m| m.contains("synthetic")));
    // Same evidence, real -> Confirmed. Same synthetic evidence in logic-demo mode -> Confirmed + warning.
    let real = run_with(&real_world(), &s, &shear_evidence(false), &[], &[], &c, SyntheticEvidencePolicy::Strict);
    assert_eq!(claim(&real, "FIELD:Good", "TR_SHEAR").unwrap().status, CoverageStatus::Confirmed);
    let demo = run_with(&real_world(), &s, &shear_evidence(true), &[], &[], &c, SyntheticEvidencePolicy::TreatAsConfirmedForLogicDemo);
    assert_eq!(claim(&demo, "FIELD:Good", "TR_SHEAR").unwrap().status, CoverageStatus::Confirmed);
    assert!(demo.synthetic_content_warnings.iter().any(|w| w.contains("LOGIC DEMO")));
}

#[test]
fn synthetic_earth_data_keeps_parameter_coverage_provisional() {
    let r = run_with(&world(), &spec(), &[], &[], &[], &no_limits(), SyntheticEvidencePolicy::Strict);
    let cl = claim(&r, "FIELD:Good", "R_SLOPE").unwrap();
    assert_eq!(cl.status, CoverageStatus::Provisional);
    let r = run_with(&real_world(), &spec(), &[], &[], &[], &no_limits(), SyntheticEvidencePolicy::Strict);
    assert_eq!(claim(&r, "FIELD:Good", "R_SLOPE").unwrap().status, CoverageStatus::Confirmed);
}

#[test]
fn synthetic_lab_catalog_and_lab_evidence_are_never_confirmed_under_strict_policy() {
    let mk = |placeholder_lab: bool, placeholder_ev: bool| LabFacility { facility_id: "TVAC".into(), name: "t".into(),
        reproduces_mechanisms: vec!["vacuum".into()], reproduces_features: vec![FeatureId::RmsSlope1x], cost_units: None, duration_days: None,
        synthetic_placeholder: placeholder_lab, transfer_evidence: vec![MechanismEvidence { synthetic_placeholder: placeholder_ev, ..lab_ev("vacuum") }] };
    let (s, c) = (spec(), no_limits());
    for (pl, pe) in [(true, false), (false, true)] {
        let r = run_with(&real_world(), &s, &[], &[mk(pl, pe)], &[], &c, SyntheticEvidencePolicy::Strict);
        assert_eq!(claim(&r, "LAB:TVAC", "TR_VAC").unwrap().status, CoverageStatus::Provisional, "lab={pl} ev={pe}");
    }
    let r = run_with(&real_world(), &s, &[], &[mk(false, false)], &[], &c, SyntheticEvidencePolicy::Strict);
    assert_eq!(claim(&r, "LAB:TVAC", "TR_VAC").unwrap().status, CoverageStatus::Confirmed);
    assert_eq!(claim(&r, "LAB:TVAC", "R_SLOPE").unwrap().status, CoverageStatus::Confirmed);
}

#[test]
fn time_dependent_mechanism_needs_a_known_window_inside_the_campaign_period() {
    let mut s = spec();
    s.mechanisms.iter_mut().find(|m| m.mechanism_id == "lowsun").unwrap().requires_field_time_window = true;
    let ev: Vec<SiteMechanismEvidence> = (0..3).map(|i| SiteMechanismEvidence { cell_id: format!("ARCTIC_{i}"), mechanisms: vec![MechanismEvidence {
        mechanism_id: "lowsun".into(), level: EvidenceLevel::Literature, source: "x".into(), synthetic_placeholder: false,
        valid_feature_bounds: [(FeatureId::ShadowObstruction1Deg, (0.3, 0.9))].into_iter().collect(), independent_validation: false }],
        validated_correction_models: vec![] }).collect();
    let win = |a: &str, b: &str| SiteWindow { region: "Arctic".into(), mechanism_id: "lowsun".into(), window: TemporalAnalogWindow {
        window_id: "W".into(), start_utc: a.into(), end_utc: b.into(), season_or_solar_regime: "t".into(), illumination_geometry_valid: true } };
    let spring = [win("2027-03-01T00:00:00Z", "2027-04-01T00:00:00Z")];
    let summer = CampaignConstraints { period_start_utc: Some("2027-06-01T00:00:00Z".into()), period_end_utc: Some("2027-08-31T00:00:00Z".into()), ..Default::default() };
    let march = CampaignConstraints { period_start_utc: Some("2027-03-10T00:00:00Z".into()), period_end_utc: Some("2027-03-20T00:00:00Z".into()), ..Default::default() };
    let w = real_world();
    // Unknown window -> Provisional with an explicit missing item.
    let r = run_with(&w, &s, &ev, &[], &[], &no_limits(), SyntheticEvidencePolicy::Strict);
    let cl = claim(&r, "FIELD:Arctic", "TR_LOWSUN").unwrap();
    assert_eq!(cl.status, CoverageStatus::Provisional);
    assert!(cl.missing_for_confirmation.iter().any(|m| m.contains("Time window")));
    // Window outside the campaign period -> the site cannot test it in this campaign.
    let r = run_with(&w, &s, &ev, &[], &spring, &summer, SyntheticEvidencePolicy::Strict);
    assert!(claim(&r, "FIELD:Arctic", "TR_LOWSUN").is_none());
    // Window overlaps the period -> Confirmed and the window is attached to the asset.
    let r = run_with(&w, &s, &ev, &[], &spring, &march, SyntheticEvidencePolicy::Strict);
    assert_eq!(claim(&r, "FIELD:Arctic", "TR_LOWSUN").unwrap().status, CoverageStatus::Confirmed);
    assert!(r.campaign_assets.iter().find(|a| a.asset_id.starts_with("FIELD:Arctic")).unwrap().time_window.is_some());
}

#[test]
fn equal_status_tie_prefers_the_asset_closer_to_confirmation() {
    let reqs = vec![creq("A", false)];
    let mut far = asset_s("AAA_FAR", true, &["A"], None, CoverageStatus::Provisional);
    far.coverage[0].missing_for_confirmation = vec!["x".into(), "y".into()];
    let mut near = asset_s("ZZZ_NEAR", true, &["A"], None, CoverageStatus::Provisional);
    near.coverage[0].missing_for_confirmation = vec!["x".into()];
    let p = optimize_campaign_portfolio(&reqs, &[far, near], &no_limits(), 16);
    assert_eq!(p.selected_assets, vec!["ZZZ_NEAR"]);
}
