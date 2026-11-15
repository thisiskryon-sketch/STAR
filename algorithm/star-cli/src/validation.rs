// star-cli/src/validation.rs
//! H1: known-analog retrieval with leave-one-positive-region-out folds.
//! H2: robustness of the Mission Match ranking to independent measurement noise.
use crate::synth::uniform01;
use serde::{Deserialize, Serialize};
use star_core::envelope::mission_distance;
use star_core::schema::{Cell, FeatureFamily, FeatureId, MissionExperimentSpec};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct H1FoldResult {
    pub holdout_region: String,
    pub pool_size: usize,
    pub relevant_cells: usize,
    pub precision_at_k: Option<f64>,
    pub recall_at_k: Option<f64>,
    pub r_precision: Option<f64>,
    pub ndcg_at_k: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct H1ValidationReport {
    pub protocol: String,
    pub k: usize,
    pub folds: Vec<H1FoldResult>,
    pub macro_precision_at_k: Option<f64>,
    pub macro_recall_at_k: Option<f64>,
    pub macro_r_precision: Option<f64>,
    pub macro_ndcg_at_k: Option<f64>,
    pub negative_control_cells_in_global_top_k: usize,
    pub caveat: String,
}

fn mean(v: &[Option<f64>]) -> Option<f64> {
    let x: Vec<f64> = v.iter().flatten().copied().collect();
    (!x.is_empty()).then(|| x.iter().sum::<f64>() / x.len() as f64)
}

fn rank_by_mission<'a>(spec: &MissionExperimentSpec, cells: &[&'a Cell]) -> Vec<&'a Cell> {
    let mut s: Vec<(&Cell, f64)> = cells
        .iter()
        .map(|c| (*c, mission_distance(spec, &c.features, &c.feature_uncertainty).d))
        .collect();
    s.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.id.cmp(&b.0.id)));
    s.into_iter().map(|x| x.0).collect()
}

/// `cells` must already be QC-valid and pass hard constraints (same stage as the pipeline).
pub fn run_h1_known_analog_retrieval(
    spec: &MissionExperimentSpec,
    cells: &[&Cell],
    positive_regions: &BTreeSet<String>,
    negative_regions: &BTreeSet<String>,
    k: usize,
) -> H1ValidationReport {
    let mut folds = Vec::new();
    for holdout in positive_regions {
        // Pool: the held-out region + every non-positive region. Other positive regions
        // are excluded, so their cells can neither count as hits nor as misses.
        let pool: Vec<&Cell> = cells
            .iter()
            .copied()
            .filter(|c| &c.region == holdout || !positive_regions.contains(&c.region))
            .collect();
        let relevant = pool.iter().filter(|c| &c.region == holdout).count();
        if relevant == 0 {
            continue;
        }
        let ranked = rank_by_mission(spec, &pool);
        let is_rel = |c: &&&Cell| &c.region == holdout;
        let k_eval = k.min(ranked.len());
        let hits_k = ranked.iter().take(k_eval).filter(is_rel).count();
        let hits_r = ranked.iter().take(relevant).filter(is_rel).count();
        let dcg: f64 = ranked
            .iter()
            .take(k_eval)
            .enumerate()
            .filter(|(_, c)| &c.region == holdout)
            .map(|(i, _)| 1.0 / ((i + 2) as f64).log2())
            .sum();
        let idcg: f64 = (0..relevant.min(k_eval)).map(|i| 1.0 / ((i + 2) as f64).log2()).sum();
        folds.push(H1FoldResult {
            holdout_region: holdout.clone(),
            pool_size: pool.len(),
            relevant_cells: relevant,
            precision_at_k: (k_eval > 0).then(|| hits_k as f64 / k_eval as f64),
            recall_at_k: Some(hits_k as f64 / relevant as f64),
            r_precision: Some(hits_r as f64 / relevant as f64),
            ndcg_at_k: (idcg > 0.0).then(|| dcg / idcg),
        });
    }
    let global = rank_by_mission(spec, cells);
    let neg_in_top = global.iter().take(k).filter(|c| negative_regions.contains(&c.region)).count();
    H1ValidationReport {
        protocol: "Leave-one-positive-region-out: pool = held-out region + all non-positive regions; \
                   relevant = held-out region only. Mission distance uses spec tolerances, so no scale is fitted on the pool."
            .into(),
        k,
        macro_precision_at_k: mean(&folds.iter().map(|f| f.precision_at_k).collect::<Vec<_>>()),
        macro_recall_at_k: mean(&folds.iter().map(|f| f.recall_at_k).collect::<Vec<_>>()),
        macro_r_precision: mean(&folds.iter().map(|f| f.r_precision).collect::<Vec<_>>()),
        macro_ndcg_at_k: mean(&folds.iter().map(|f| f.ndcg_at_k).collect::<Vec<_>>()),
        folds,
        negative_control_cells_in_global_top_k: neg_in_top,
        caveat: "Positive labels are known-analog roles, not proof of analogy for this experiment; on synthetic data this checks code logic only, not scientific accuracy.".into(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct H2PerturbationReport {
    pub trials: usize,
    pub noise_model: String,
    pub cells_with_assumed_sigma: usize,
    pub mean_spearman_rho: Option<f64>,
    pub min_spearman_rho: Option<f64>,
    pub top1_stability: Option<f64>,
    pub top_k_inclusion_probability: BTreeMap<String, f64>,
}

/// Fallback 1-sigma when a cell does not report feature uncertainty (declared assumption).
pub fn assumed_sigma(f: FeatureId) -> f64 {
    match f.family() {
        FeatureFamily::TerrainSlope => 1.0,
        FeatureFamily::TerrainRoughness => 0.05,
        FeatureFamily::IlluminationShadow => 0.03,
    }
}

pub fn gaussian(seed: u64) -> f64 {
    let u1 = uniform01(seed).max(1e-300);
    let u2 = uniform01(seed ^ 0xD1B5_4A32_D192_ED03);
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

pub fn noise_seed(seed: u64, trial: usize, cell: usize, feature: usize) -> u64 {
    seed ^ (trial as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (cell as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (feature as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
}

fn spearman(a: &[usize], b: &[usize]) -> Option<f64> {
    let n = a.len();
    if n < 2 || a.len() != b.len() {
        return None;
    }
    let d2: f64 = a.iter().zip(b).map(|(x, y)| (*x as f64 - *y as f64).powi(2)).sum();
    let nf = n as f64;
    Some(1.0 - 6.0 * d2 / (nf * (nf * nf - 1.0)))
}

pub fn run_h2_noise_robustness(
    spec: &MissionExperimentSpec,
    cells: &[&Cell],
    trials: usize,
    top_k: usize,
    seed: u64,
) -> H2PerturbationReport {
    let active = spec.active_features();
    let base = rank_by_mission(spec, cells);
    let base_rank: BTreeMap<&str, usize> = base.iter().enumerate().map(|(i, c)| (c.id.as_str(), i)).collect();
    let assumed = cells.iter().filter(|c| active.iter().any(|f| !c.feature_uncertainty.contains_key(f))).count();
    let mut rhos = Vec::new();
    let mut top1_same = 0usize;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for t in 0..trials {
        let mut scored: Vec<(&Cell, f64)> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut f = c.features.clone();
                for (fi, feat) in active.iter().enumerate() {
                    let s = c.feature_uncertainty.get(feat).copied().unwrap_or_else(|| assumed_sigma(*feat));
                    let v = f.get(*feat) + s * gaussian(noise_seed(seed, t, i, fi));
                    *f.get_mut(*feat) = match feat.family() {
                        FeatureFamily::TerrainSlope => v.clamp(0.0, 90.0),
                        FeatureFamily::TerrainRoughness => v.max(0.0),
                        FeatureFamily::IlluminationShadow => v.clamp(0.0, 1.0),
                    };
                }
                (*c, mission_distance(spec, &f, &c.feature_uncertainty).d)
            })
            .collect();
        scored.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.id.cmp(&b.0.id)));
        if let (Some(b0), Some(p0)) = (base.first(), scored.first()) {
            if b0.id == p0.0.id {
                top1_same += 1;
            }
        }
        for (c, _) in scored.iter().take(top_k) {
            *counts.entry(c.id.clone()).or_insert(0) += 1;
        }
        let a: Vec<usize> = scored.iter().map(|(c, _)| base_rank[c.id.as_str()]).collect();
        let b: Vec<usize> = (0..scored.len()).collect();
        if let Some(r) = spearman(&a, &b) {
            rhos.push(r);
        }
    }
    H2PerturbationReport {
        trials,
        noise_model: "Independent Gaussian per (trial, cell, feature) with the cell's reported 1-sigma; \
                      assumed sigma (slope 1°, roughness 0.05 m, shadow 0.03) where unreported."
            .into(),
        cells_with_assumed_sigma: assumed,
        mean_spearman_rho: (!rhos.is_empty()).then(|| rhos.iter().sum::<f64>() / rhos.len() as f64),
        min_spearman_rho: rhos.iter().copied().reduce(f64::min),
        top1_stability: (trials > 0 && !base.is_empty()).then(|| top1_same as f64 / trials as f64),
        top_k_inclusion_probability: counts.into_iter().map(|(k, v)| (k, v as f64 / trials.max(1) as f64)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_independent_across_cells() {
        // The v4.0 generator gave all cells of a trial almost the same perturbation.
        let xs: Vec<f64> = (0..200).map(|i| gaussian(noise_seed(42, 0, i, 0))).collect();
        let ys: Vec<f64> = (0..200).map(|i| gaussian(noise_seed(42, 0, i + 1, 0))).collect();
        let m = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let (mx, my) = (m(&xs), m(&ys));
        let cov: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum::<f64>() / 200.0;
        let sx = (xs.iter().map(|x| (x - mx).powi(2)).sum::<f64>() / 200.0).sqrt();
        let sy = (ys.iter().map(|y| (y - my).powi(2)).sum::<f64>() / 200.0).sqrt();
        assert!((cov / (sx * sy)).abs() < 0.2, "neighbouring cells must get uncorrelated noise");
        assert!((sx - 1.0).abs() < 0.2, "unit variance expected, got {sx}");
    }

    #[test]
    fn spearman_is_none_when_undefined() {
        assert_eq!(spearman(&[0], &[0]), None);
        assert_eq!(spearman(&[0, 1], &[0]), None);
        assert_eq!(spearman(&[0, 1, 2], &[2, 1, 0]), Some(-1.0));
    }
}
