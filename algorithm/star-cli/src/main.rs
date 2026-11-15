// star-cli/src/main.rs
//! Usage:
//!   star-cli --import MANIFEST.json --out-dir DIR [--force]   convert star-temp-0 exports (see import.rs)
//!   star-cli --lowsun-windows --region R --mechanism M --lat LAT --year Y [--max-elev 3]
//!            print site windows (JSON) where the noon sun stays within (0°, max] all day
//!   star-cli --demo [--validate]                      writes synthetic inputs to demo_data/ and runs on them
//!   star-cli --input-dir DIR [--output FILE] [--target-id ID] [--top-n N] [--validate] [--all-targets]
//!   --all-targets: run the same experiment for every region in targets.json and compare
//!   their Earth-testability (writes targets_testability.{json,md} next to the output).
//! Input files in DIR: cells.json, targets.json, mission_spec.json (required);
//! catalog.json, evidence.json, labs.json, labels.json, site_windows.json, params.json,
//! campaign_constraints.json (optional).
//! --trust-synthetic: LOGIC DEMO ONLY, counts synthetic evidence as confirmed (reports say so).
//! Input files are never overwritten; outputs go to --output (default DIR/report_v4.json) and a .md next to it.
mod import;
mod report;
mod synth;
mod validation;

use serde::de::DeserializeOwned;
use star_core::assessment::{CampaignConstraints, LabFacility, SiteMechanismEvidence, SiteWindow};
use star_core::schema::{Cell, KnownAnalogSite, MissionExperimentSpec, RunManifest, RunParameters, TargetProfile};
use star_core::{
    discover_candidates_v4, hard_constraint_violations, order_targets_by_residual_risk, qc_filter,
    spec_for_target, summarize_target, DiscoveryInputs,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use synth::{build_synthetic_test_bundle, KnownAnalogLabels};
use validation::{run_h1_known_analog_retrieval, run_h2_noise_robustness};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn read_required<T: DeserializeOwned>(dir: &Path, name: &str, hash: &mut Fnv) -> Res<T> {
    let raw = fs::read(dir.join(name)).map_err(|e| format!("cannot read {}: {e}", dir.join(name).display()))?;
    hash.update(name.as_bytes());
    hash.update(&raw);
    serde_json::from_slice(&raw).map_err(|e| format!("{name}: {e}").into())
}

fn read_optional<T: DeserializeOwned>(dir: &Path, name: &str, hash: &mut Fnv) -> Res<Option<T>> {
    if dir.join(name).exists() { read_required(dir, name, hash).map(Some) } else { Ok(None) }
}

/// FNV-1a 64-bit over all input bytes and run parameters (reproducibility fingerprint, not a security hash).
struct Fnv(u64);
impl Fnv {
    fn new() -> Self { Fnv(0xcbf2_9ce4_8422_2325) }
    fn update(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= *b as u64;
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

fn utc_now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // civil-from-days (H. Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn write_json<T: serde::Serialize>(dir: &Path, name: &str, v: &T) -> Res<()> {
    fs::write(dir.join(name), serde_json::to_string_pretty(v)?)?;
    Ok(())
}

fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--lowsun-windows") {
        let need = |k: &str| arg_value(&args, k).ok_or(format!("--lowsun-windows needs {k}"));
        let lat: f64 = need("--lat")?.parse()?;
        let year: i32 = need("--year")?.parse()?;
        let max_elev: f64 = arg_value(&args, "--max-elev").map(|v| v.parse()).transpose()?.unwrap_or(3.0);
        let w = import::lowsun_windows(&need("--region")?, &need("--mechanism")?, lat, year, 0.0, max_elev);
        println!("{}", serde_json::to_string_pretty(&w)?);
        return Ok(());
    }
    if let Some(manifest) = arg_value(&args, "--import") {
        let out = PathBuf::from(arg_value(&args, "--out-dir").ok_or("--import needs --out-dir DIR")?);
        let rep = import::run_import(Path::new(&manifest), &out, args.iter().any(|a| a == "--force"))?;
        println!("[S.T.A.R. import] earth cells: {} | targets: {} | catalog sites: {} | skipped: {} -> {}",
            rep.earth_cells_written, rep.targets_written, rep.catalog_sites_written, rep.skipped.len(), out.join("import_report.json").display());
        for w in &rep.warnings {
            println!("  note: {w}");
        }
        return Ok(());
    }
    let is_demo = args.iter().any(|a| a == "--demo");
    let run_validation = args.iter().any(|a| a == "--validate");

    let data_dir = if is_demo {
        // Demo inputs live in their own directory and are regenerated; real inputs are never touched.
        let dir = PathBuf::from("demo_data");
        fs::create_dir_all(&dir)?;
        let b = build_synthetic_test_bundle();
        write_json(&dir, "mission_spec.json", &b.spec)?;
        write_json(&dir, "targets.json", &b.targets)?;
        write_json(&dir, "cells.json", &b.cells)?;
        write_json(&dir, "catalog.json", &b.catalog)?;
        write_json(&dir, "evidence.json", &b.evidence)?;
        write_json(&dir, "labs.json", &b.labs)?;
        write_json(&dir, "labels.json", &b.labels)?;
        write_json(&dir, "site_windows.json", &b.site_windows)?;
        dir
    } else {
        PathBuf::from(arg_value(&args, "--input-dir").ok_or("production mode needs --input-dir DIR (or use --demo)")?)
    };
    let output_path = arg_value(&args, "--output").map(PathBuf::from).unwrap_or_else(|| data_dir.join("report_v4.json"));

    let mut hash = Fnv::new();
    let cells: Vec<Cell> = read_required(&data_dir, "cells.json", &mut hash)?;
    let targets: Vec<TargetProfile> = read_required(&data_dir, "targets.json", &mut hash)?;
    let spec: MissionExperimentSpec = read_required(&data_dir, "mission_spec.json", &mut hash)?;
    let catalog: Vec<KnownAnalogSite> = read_optional(&data_dir, "catalog.json", &mut hash)?.unwrap_or_default();
    let evidence: Vec<SiteMechanismEvidence> = read_optional(&data_dir, "evidence.json", &mut hash)?.unwrap_or_default();
    let labs: Vec<LabFacility> = read_optional(&data_dir, "labs.json", &mut hash)?.unwrap_or_default();
    let labels: Option<KnownAnalogLabels> = read_optional(&data_dir, "labels.json", &mut hash)?;
    let site_windows: Vec<SiteWindow> = read_optional(&data_dir, "site_windows.json", &mut hash)?.unwrap_or_default();
    let mut params: RunParameters = read_optional(&data_dir, "params.json", &mut hash)?.unwrap_or_default();
    let constraints: CampaignConstraints = read_optional(&data_dir, "campaign_constraints.json", &mut hash)?
        .unwrap_or(CampaignConstraints { max_assets: Some(5), ..Default::default() });
    if args.iter().any(|a| a == "--trust-synthetic") {
        params.synthetic_evidence_policy = star_core::schema::SyntheticEvidencePolicy::TreatAsConfirmedForLogicDemo;
    }
    if let Some(n) = arg_value(&args, "--top-n") {
        params.top_n_dossiers = n.parse()?;
    }
    hash.update(serde_json::to_string(&params)?.as_bytes());
    hash.update(serde_json::to_string(&constraints)?.as_bytes());

    // Target selection: --target-id, else spec.target_profile_id, else the only target.
    let all_targets = args.iter().any(|a| a == "--all-targets");
    let wanted = arg_value(&args, "--target-id").or_else(|| spec.target_profile_id.clone());
    let target = match &wanted {
        Some(id) => targets.iter().find(|t| &t.id == id).ok_or(format!("target '{id}' not found in targets.json"))?,
        None if targets.len() == 1 => &targets[0],
        // In --all-targets mode the detailed report is produced for the first target by id.
        None if all_targets && !targets.is_empty() => targets.iter().min_by(|a, b| a.id.cmp(&b.id)).unwrap(),
        None => return Err("several targets: pass --target-id, set spec.target_profile_id, or use --all-targets".into()),
    };
    // A spec without its own target values is adapted to the selected target (mission widths kept).
    let spec = if wanted.is_none() && targets.len() > 1 { spec_for_target(&spec, target) } else { spec };
    hash.update(target.id.as_bytes());

    let manifest = RunManifest {
        run_id: format!("star-{}-{:016x}", spec.experiment_id, hash.0),
        timestamp_utc: utc_now_rfc3339(),
        code_revision: format!("star-cli {}{}", env!("CARGO_PKG_VERSION"), option_env!("STAR_GIT_REV").map(|r| format!(" @ {r}")).unwrap_or_default()),
        dataset_hash: format!("fnv1a64:{:016x}", hash.0),
        calibration_version: format!("earth-qc-p01-p99@{:016x}", hash.0),
        random_seed: 42,
        is_synthetic_demo: is_demo,
        validation_protocol: "H1 leave-one-positive-region-out (P@k, R@k, R-precision, NDCG@k); H2 independent Gaussian noise".into(),
    };

    let report = discover_candidates_v4(DiscoveryInputs {
        manifest,
        params: &params,
        spec: &spec,
        target,
        cells: &cells,
        catalog: &catalog,
        precomputed_scales: None,
        evidence: &evidence,
        labs: &labs,
        site_windows: &site_windows,
        campaign_constraints: &constraints,
    })?;

    if all_targets {
        let mut summaries = Vec::new();
        for t in &targets {
            let s = spec_for_target(&spec, t);
            let mut m = report.manifest.clone();
            m.run_id = format!("{}-{}", m.run_id, t.id);
            let r = discover_candidates_v4(DiscoveryInputs {
                manifest: m,
                params: &params,
                spec: &s,
                target: t,
                cells: &cells,
                catalog: &catalog,
                precomputed_scales: None,
                evidence: &evidence,
                labs: &labs,
                site_windows: &site_windows,
                campaign_constraints: &constraints,
            })?;
            summaries.push(summarize_target(&r));
        }
        order_targets_by_residual_risk(&mut summaries);
        fs::write(output_path.with_file_name("targets_testability.json"), serde_json::to_string_pretty(&summaries)?)?;
        let md = report::targets_markdown(&summaries);
        fs::write(output_path.with_file_name("targets_testability.md"), &md)?;
        print!("{md}");
    }

    let mut validation_md = String::new();
    if is_demo || run_validation {
        match &labels {
            None => validation_md.push_str("Validation skipped: labels.json not provided.\n"),
            Some(l) => {
                let (qc, _) = qc_filter(&cells, &spec, target, &params);
                let feasible: Vec<&Cell> = qc.into_iter().filter(|c| hard_constraint_violations(&spec, c).is_empty()).collect();
                let pos: BTreeSet<String> = l.positive_regions.iter().cloned().collect();
                let neg: BTreeSet<String> = l.negative_control_regions.iter().cloned().collect();
                let h1 = run_h1_known_analog_retrieval(&spec, &feasible, &pos, &neg, 4);
                let h2 = run_h2_noise_robustness(&spec, &feasible, 200, 5, 42);
                validation_md = report::validation_markdown(&h1, &h2);
                let vpath = output_path.with_file_name("validation_v4.json");
                fs::write(&vpath, serde_json::to_string_pretty(&(h1, h2))?)?;
            }
        }
    }

    fs::write(&output_path, serde_json::to_string_pretty(&report)?)?;
    let md_path = output_path.with_extension("md");
    fs::write(&md_path, report::campaign_markdown(&report, &validation_md))?;

    println!("[S.T.A.R.] {} | synthetic_demo={}", report.manifest.run_id, is_demo);
    println!("  Joint envelope: {:?}", report.envelope.joint_status);
    println!("  DECISION: {}", report.decision.statement);
    println!("  Report: {} | {}", output_path.display(), md_path.display());
    if !validation_md.is_empty() {
        print!("{validation_md}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use star_core::envelope::JointReachStatus;
    use star_core::schema::{RunManifest, SyntheticEvidencePolicy};
    use star_core::{DiscoveryMode, DiscoveryReport};

    fn demo_report() -> DiscoveryReport {
        demo_report_with(SyntheticEvidencePolicy::Strict)
    }

    fn demo_report_with(policy: SyntheticEvidencePolicy) -> DiscoveryReport {
        let b = build_synthetic_test_bundle();
        let mut params = RunParameters::default();
        params.synthetic_evidence_policy = policy;
        discover_candidates_v4(DiscoveryInputs {
            manifest: RunManifest {
                run_id: "t".into(), timestamp_utc: "t".into(), code_revision: "t".into(), dataset_hash: "t".into(),
                calibration_version: "t".into(), random_seed: 1, is_synthetic_demo: true, validation_protocol: "t".into(),
            },
            params: &params,
            spec: &b.spec,
            target: &b.target,
            cells: &b.cells,
            catalog: &b.catalog,
            precomputed_scales: None,
            evidence: &b.evidence,
            labs: &b.labs,
            site_windows: &b.site_windows,
            campaign_constraints: &CampaignConstraints { max_budget_units: None, max_total_duration_days: None, max_assets: Some(5), period_start_utc: None, period_end_utc: None },
        })
        .unwrap()
    }

    #[test]
    fn demo_negative_controls_are_not_in_mission_top() {
        let r = demo_report();
        let mm = r.mode_rankings.iter().find(|m| m.mode == DiscoveryMode::MissionMatch).unwrap();
        for e in mm.entries.iter().take(10) {
            assert!(!e.cell_id.starts_with("RANK_NEG") && !e.cell_id.starts_with("GEN_") && !e.cell_id.starts_with("OUTLIER"),
                "{} must not be in the Mission Match top-10", e.cell_id);
        }
        assert!(mm.entries[0].cell_id.starts_with("KAM_") || mm.entries[0].cell_id.starts_with("ICE_"));
    }

    #[test]
    fn demo_target_is_jointly_observed_but_mechanisms_are_not() {
        let r = demo_report();
        assert!(matches!(r.envelope.joint_status, JointReachStatus::JointObservedWithSupport { .. }));
        assert!(!r.envelope.mechanism.single_venue_reproduces_everything);
        assert_eq!(r.envelope.mechanism.irreducible_mechanisms, vec!["sustained_lunar_gravity".to_string()]);
    }

    #[test]
    fn demo_portfolio_beats_single_best_site() {
        let r = demo_report_with(SyntheticEvidencePolicy::TreatAsConfirmedForLogicDemo);
        let p = &r.campaign_portfolio;
        let base = p.single_best_site_baseline.as_ref().unwrap();
        assert!(p.selected_assets.len() >= 3, "{:?}", p.selected_assets);
        assert!(p.confirmed_coverage > base.confirmed_coverage);
        assert!((p.testable_coverage - 1.0).abs() < 1e-9);
        assert_eq!(p.irreducible_requirements, vec!["TR_GRAVITY_SINKAGE".to_string()]);
        assert!(p.selected_assets.iter().any(|a| a.starts_with("FIELD:Kamchatka")));
        assert!(p.selected_assets.iter().any(|a| a == "LAB:TVAC_DUSTY_OPTICS"));
        assert!(r.synthetic_content_warnings.iter().any(|w| w.contains("LOGIC DEMO MODE")));
    }

    #[test]
    fn strict_policy_never_confirms_synthetic_content() {
        let r = demo_report();
        let p = &r.campaign_portfolio;
        assert_eq!(p.confirmed_coverage, 0.0, "every input of the demo is synthetic");
        assert!((p.testable_coverage - 1.0).abs() < 1e-9);
        for a in &r.campaign_assets {
            for c in &a.coverage {
                assert_eq!(c.status, star_core::assessment::CoverageStatus::Provisional, "{} {}", a.asset_id, c.requirement_id);
                assert!(c.missing_for_confirmation.iter().any(|m| m.contains("synthetic")), "{} {}", a.asset_id, c.requirement_id);
            }
        }
        // With equal (provisional) status the site closer to confirmation is preferred.
        assert!(p.selected_assets.iter().any(|a| a.starts_with("FIELD:Kamchatka")));
    }

    #[test]
    fn demo_rare_and_underexplored_modes_differ_from_mission_match() {
        let r = demo_report();
        let top = |m: DiscoveryMode| r.mode_rankings.iter().find(|x| x.mode == m).unwrap().entries.first().map(|e| e.region.clone());
        assert_eq!(top(DiscoveryMode::RareCombination).as_deref(), Some("Puna_Plateau"));
        assert_eq!(top(DiscoveryMode::UnderexploredRegion).as_deref(), Some("Puna_Plateau"));
        assert_ne!(top(DiscoveryMode::MissionMatch), top(DiscoveryMode::RareCombination));
    }

    #[test]
    fn demo_decision_is_honest_about_the_undecided_lead_site() {
        let r = demo_report_with(SyntheticEvidencePolicy::TreatAsConfirmedForLogicDemo);
        assert!(matches!(r.decision.lead_site, star_core::LeadSiteVerdict::UndecidedUncertaintyUnknown { .. }));
        assert_eq!(r.decision.first_action.as_ref().unwrap().requirement_id, "REQ_ROUGH_1X");
        assert_eq!(r.decision.mission_critical_unconfirmed, vec!["TR_GRAVITY_SINKAGE".to_string()]);
    }

    #[test]
    fn demo_regions_differ_in_earth_testability() {
        let b = build_synthetic_test_bundle();
        let mut params = RunParameters::default();
        params.synthetic_evidence_policy = SyntheticEvidencePolicy::TreatAsConfirmedForLogicDemo;
        let c = CampaignConstraints { max_budget_units: None, max_total_duration_days: None, max_assets: Some(5), period_start_utc: None, period_end_utc: None };
        let mut sums: Vec<_> = b.targets.iter().map(|t| {
            let s = spec_for_target(&b.spec, t);
            star_core::summarize_target(&discover_candidates_v4(DiscoveryInputs {
                manifest: RunManifest { run_id: "t".into(), timestamp_utc: "t".into(), code_revision: "t".into(), dataset_hash: "t".into(),
                    calibration_version: "t".into(), random_seed: 1, is_synthetic_demo: true, validation_protocol: "t".into() },
                params: &params, spec: &s, target: t, cells: &b.cells, catalog: &b.catalog, precomputed_scales: None,
                evidence: &b.evidence, labs: &b.labs, site_windows: &b.site_windows, campaign_constraints: &c }).unwrap())
        }).collect();
        order_targets_by_residual_risk(&mut sums);
        assert_eq!(sums.last().unwrap().target_id, "SYNTH_HAWORTH");
        assert!(sums.first().unwrap().critical_reducible_unconfirmed.is_empty());
    }

    #[test]
    fn demo_h1_recall_never_exceeds_one_and_counts_only_holdout() {
        let b = build_synthetic_test_bundle();
        let refs: Vec<&Cell> = b.cells.iter().filter(|c| c.valid_pixel_ratio >= 0.85 && c.resolution_m == 2.0).collect();
        let pos: BTreeSet<String> = b.labels.positive_regions.iter().cloned().collect();
        let neg: BTreeSet<String> = b.labels.negative_control_regions.iter().cloned().collect();
        let h1 = run_h1_known_analog_retrieval(&b.spec, &refs, &pos, &neg, 4);
        for f in &h1.folds {
            assert!(f.recall_at_k.unwrap() <= 1.0);
            assert!(f.r_precision.unwrap() <= 1.0);
        }
        assert_eq!(h1.negative_control_cells_in_global_top_k, 0);
    }
}
