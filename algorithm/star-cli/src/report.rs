// star-cli/src/report.rs
//! Human-readable campaign report (Markdown) for the demo and for the teammate's UI.
use crate::validation::{H1ValidationReport, H2PerturbationReport};
use star_core::envelope::JointReachStatus;
use star_core::{DiscoveryReport, LeadSiteVerdict, TargetRehearsalSummary};
use std::fmt::Write;

fn opt(v: Option<f64>, digits: usize) -> String {
    v.map(|x| format!("{x:.digits$}")).unwrap_or_else(|| "n/a".into())
}

pub fn validation_markdown(h1: &H1ValidationReport, h2: &H2PerturbationReport) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "\n## Validation (logic checks)\n");
    let _ = writeln!(s, "H1 known-analog retrieval, k = {}: P@k {}, R@k {}, R-precision {}, NDCG@k {}; negative controls in global top-k: {}.",
        h1.k, opt(h1.macro_precision_at_k, 2), opt(h1.macro_recall_at_k, 2), opt(h1.macro_r_precision, 2),
        opt(h1.macro_ndcg_at_k, 2), h1.negative_control_cells_in_global_top_k);
    let _ = writeln!(s, "H2 noise robustness, {} trials: mean Spearman {}, min {}, top-1 stability {} ({} cells used assumed σ).",
        h2.trials, opt(h2.mean_spearman_rho, 3), opt(h2.min_spearman_rho, 3), opt(h2.top1_stability, 2), h2.cells_with_assumed_sigma);
    let _ = writeln!(s, "\n_{}_", h1.caveat);
    s
}

pub fn targets_markdown(v: &[TargetRehearsalSummary]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# Earth-testability of candidate lunar regions\n");
    let _ = writeln!(s, "Same experiment, same requirement widths, same Earth data and labs for every region. Ordered by pre-flight residual risk for THIS experiment (unconfirmed mission-critical → irreducible → confirmed → testable). This is not a landing-safety ranking.\n");
    let _ = writeln!(s, "| # | Region | Joint envelope | Confirmed | Testable | Critical unconfirmed (reducible) | Irreducible | Lead site | Campaign |\n|---|---|---|---|---|---|---|---|---|");
    for (i, t) in v.iter().enumerate() {
        let joint = match &t.joint_status {
            JointReachStatus::JointObservedWithSupport { independent_sites, .. } => format!("joint at {independent_sites} sites"),
            JointReachStatus::JointObservedSingleSite { .. } => "joint, single site".into(),
            JointReachStatus::MarginalOnlyJointUnobserved => "never jointly".into(),
            JointReachStatus::OutsideMarginalEnvelope { unreachable_requirements } => format!("outside Earth: {}", unreachable_requirements.join(", ")),
            JointReachStatus::NoValidEarthData => "no data".into(),
        };
        let lead = match &t.lead_site {
            LeadSiteVerdict::Resolved { site, .. } => site.clone(),
            LeadSiteVerdict::TiedWithinUncertainty { sites } => format!("tie: {}", sites.join("/")),
            LeadSiteVerdict::UndecidedUncertaintyUnknown { sites } => format!("undecided: {}", sites.join("/")),
            LeadSiteVerdict::WeightSensitive { leaders } => format!("weight-sensitive: {}", leaders.join("/")),
            LeadSiteVerdict::NoCandidate => "none".into(),
        };
        let _ = writeln!(s, "| {} | {} | {} | {:.0}% | {:.0}% | {} | {} | {} | {} |", i + 1, t.region_name, joint,
            t.confirmed_coverage * 100.0, t.testable_coverage * 100.0, list(&t.critical_reducible_unconfirmed),
            list(&t.irreducible_until_flight), lead, t.campaign_assets.join(" + "));
    }
    s
}

pub fn campaign_markdown(r: &DiscoveryReport, validation_md: &str) -> String {
    let mut s = String::new();
    let spec = &r.mission_spec;
    let _ = writeln!(s, "# S.T.A.R. campaign report: {}\n", spec.experiment_id);
    for w in &r.synthetic_content_warnings {
        let _ = writeln!(s, "> **SYNTHETIC:** {w}");
    }
    let _ = writeln!(s, "\nTarget: {:?} / {} ({}). Objective: {}.", spec.target_body, spec.target_region, r.target_id, spec.objective);
    let _ = writeln!(s, "Run: `{}`, {}, data {}.", r.manifest.run_id, r.manifest.timestamp_utc, r.manifest.dataset_hash);
    for w in &r.consistency_warnings {
        let _ = writeln!(s, "\n⚠ {w}");
    }
    let _ = writeln!(s, "\n## Decision\n\n**{}**", r.decision.statement);

    let _ = writeln!(s, "\n## 1. What Earth can reproduce\n");
    let _ = writeln!(s, "| Requirement | Earth cells within range | Observed Earth range |\n|---|---|---|");
    for m in &r.envelope.marginal {
        let _ = writeln!(s, "| {} | {} | {:.3} … {:.3} |", m.requirement_id, m.cells_within_range, m.observed_min, m.observed_max);
    }
    let joint = match &r.envelope.joint_status {
        JointReachStatus::NoValidEarthData => "no valid Earth data".to_string(),
        JointReachStatus::OutsideMarginalEnvelope { unreachable_requirements } => format!("outside Earth's marginal envelope: {}", unreachable_requirements.join(", ")),
        JointReachStatus::MarginalOnlyJointUnobserved => "each requirement exists somewhere, but never all together".into(),
        JointReachStatus::JointObservedSingleSite { supporting_cells } => format!("all requirements met together, but only at one site ({supporting_cells} cells)"),
        JointReachStatus::JointObservedWithSupport { supporting_cells, independent_sites } => format!("all requirements met together at {independent_sites} independent sites ({supporting_cells} cells)"),
    };
    let _ = writeln!(s, "\n**Joint envelope:** {joint}.");
    let me = &r.envelope.mechanism;
    let _ = writeln!(s, "\n| Mechanism | Field | Lab | Note |\n|---|---|---|---|");
    for m in &me.per_mechanism {
        let has = |v| if m.venues.contains(&v) { "yes" } else { "no" };
        let note = if m.irreducible_on_earth {
            "irreducible on Earth".to_string()
        } else {
            m.field_min_abs_latitude_deg.map(|l| format!("field only at |lat| ≥ {l:.0}°")).unwrap_or_default()
        };
        let _ = writeln!(s, "| {} | {} | {} | {} |", m.mechanism_id, has(star_core::schema::TestVenue::FieldSite), has(star_core::schema::TestVenue::Laboratory), note);
    }
    if me.single_venue_reproduces_everything {
        let _ = writeln!(s, "\nOne venue type can host every required mechanism: {:?}.", me.venues_hosting_all_reducible);
    } else {
        let _ = writeln!(s, "\n**No single place or facility on Earth reproduces all required mechanisms** — the experiment has to be split into a campaign.");
    }
    if !me.unknown_mechanisms.is_empty() {
        let _ = writeln!(s, "Mechanisms with unknown reproducibility: {}.", me.unknown_mechanisms.join(", "));
    }

    let _ = writeln!(s, "\n## 2. Candidate sites by discovery mode\n");
    for mr in &r.mode_rankings {
        let _ = writeln!(s, "**{:?}** ({:?}): {}", mr.mode, mr.status, mr.eligibility_rule);
        for e in mr.entries.iter().take(5) {
            let front = if e.within_epsilon { format!("front {}", e.pareto_front_rank) } else { "beyond epsilon".into() };
            let _ = writeln!(s, "- {} ({}) d = {:.3}, {}{}", e.cell_id, e.region, e.d_mission, front,
                e.mode_metric.map(|m| format!(", metric {m:.3}")).unwrap_or_default());
        }
        let _ = writeln!(s);
    }

    let _ = writeln!(s, "## 3. Site dossiers\n");
    let _ = writeln!(s, "| Site | d ± σ | In range | Rarity | AET | Transferable | Not transferable |\n|---|---|---|---|---|---|---|");
    for d in &r.dossiers {
        let om = &d.observable_match;
        let _ = writeln!(s, "| {} ({}) | {:.3} ± {} | {}/{} | {:?} | {:?} | {} | {} |",
            d.cell_id, d.region, om.mission_distance, opt(om.mission_distance_sigma, 3),
            om.breakdown.iter().filter(|b| b.within_range).count(), om.breakdown.len(),
            d.novelty.rarity_status, d.aet.overall_status,
            d.aet.transferable_requirements.join(", "), d.aet.not_transferable_requirements.join(", "));
    }

    let p = &r.campaign_portfolio;
    let _ = writeln!(s, "\n## 4. Rehearsal campaign (E.C.O.)\n");
    let _ = writeln!(s, "Status: {:?}. Cost model: {:?}. Budget: {:?}. Duration: {:?}.\n", p.status, p.cost_model, p.budget_check, p.duration_check);
    for (i, st) in p.selection_steps.iter().enumerate() {
        let _ = writeln!(s, "{}. **{}** — confirms: {}; testable only: {}; upgrades: {}", i + 1, st.asset_id,
            list(&st.newly_confirmed), list(&st.newly_provisional), list(&st.upgraded_to_confirmed));
    }
    let _ = writeln!(s, "\nConfirmed before flight: **{:.0}%** of reducible requirements; testable: **{:.0}%**.", p.confirmed_coverage * 100.0, p.testable_coverage * 100.0);
    if let Some(b) = &p.single_best_site_baseline {
        let _ = writeln!(s, "Single most similar site ({}): confirmed {:.0}%, testable {:.0}%.", b.asset_id, b.confirmed_coverage * 100.0, b.testable_coverage * 100.0);
    }
    if let Some(x) = &p.exact_check {
        let _ = writeln!(s, "Exact check over {} assets: greedy {} (exact optimum: {}).", x.assets_considered,
            if x.greedy_is_optimal { "is optimal" } else { "is NOT optimal" }, x.best_selected_assets.join(" + "));
    }
    for a in &p.alternatives {
        let _ = writeln!(s, "- Without {}: {} — confirmed {:.0}%, testable {:.0}%{}", a.without_asset, a.selected_assets.join(" + "),
            a.confirmed_coverage * 100.0, a.testable_coverage * 100.0,
            if a.critical_confirmations_lost.is_empty() { String::new() } else { format!("; LOSES critical: {}", a.critical_confirmations_lost.join(", ")) });
    }

    let _ = writeln!(s, "\n## 5. Residual risk register\n");
    let _ = writeln!(s, "| Requirement | Criticality | Pre-flight status | Rehearsed by | Missing for confirmation |\n|---|---|---|---|---|");
    for e in &r.residual_risk_register {
        let _ = writeln!(s, "| {} — {} | {:?} | {:?} | {} | {} |", e.requirement_id, e.engineering_decision, e.criticality, e.status,
            list(&e.rehearsed_by), if e.missing_for_confirmation.is_empty() { "-".into() } else { e.missing_for_confirmation.join("; ") });
    }
    let _ = writeln!(s, "\n**Next actions (campaign U.D.D.):**");
    for a in r.campaign_next_actions.iter().take(8) {
        let _ = writeln!(s, "- [{:?}] {}: {}", a.criticality, a.requirement_id, a.action);
    }

    let u = &r.udd_recommendation;
    let _ = writeln!(s, "\n## 6. Lead-site decision (site U.D.D. + weight sensitivity)\n");
    let _ = writeln!(s, "{}", u.rationale);
    if let Some(n) = &u.correlation_note {
        let _ = writeln!(s, "\n_{n}_");
    }
    let ws = &r.weight_sensitivity;
    let _ = writeln!(s, "\nWeight sensitivity ({} trials, {}): lead-site stability {}; leaders {:?}.", ws.trials, ws.perturbation, opt(ws.lead_site_stability, 2), ws.leaders);

    let _ = writeln!(s, "\n## 7. Rejected cells and what would change the decision\n");
    for rej in &r.rejections {
        let _ = writeln!(s, "- {} ({}): {}", rej.cell_id, if rej.stage_qc { "QC" } else { "mission" }, rej.what_would_change_decision.join(" "));
    }
    s.push_str(validation_md);
    s
}

fn list(v: &[String]) -> String {
    if v.is_empty() { "none".into() } else { v.join(", ") }
}
