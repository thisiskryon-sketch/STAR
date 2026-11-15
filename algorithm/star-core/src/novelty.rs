// star-core/src/novelty.rs
//! Earth Rarity, Catalog Novelty and Geographic Novelty (FR-05), kept separate.
use crate::envelope::feature_distance;
use crate::schema::{Cell, FeatureId, FeatureScales, KnownAnalogSite, RunParameters};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaritySupportStatus {
    /// Rare on Earth and confirmed by other spatially independent cells.
    SupportedRareCombination,
    /// Far from everything but not confirmed: data edge, artefact or a single odd cell.
    IsolatedOutlierCandidate,
    CommonTerrestrialTerrain,
    InsufficientSpatialNeighbours,
    /// No calibrated feature was usable for the distance.
    NotComputable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoveltyMetrics {
    pub earth_rarity_knn_distance: Option<f64>,
    pub effective_k: Option<usize>,
    pub support_within_rarity_radius: usize,
    pub rarity_status: RaritySupportStatus,
    pub catalog_novelty_distance: Option<f64>,
    pub nearest_catalog_site_id: Option<String>,
    pub geographic_distance_km: Option<f64>,
    pub heuristic_quality_score: f64,
}

pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0_f64;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    2.0 * r * a.clamp(0.0, 1.0).sqrt().asin()
}

/// Deterministic greedy spatial thinning: cells are visited in id order, so the
/// result does not depend on input order (technical spec v4, §9).
pub fn spatially_independent<'a>(cells: &[&'a Cell], min_separation_km: f64) -> Vec<&'a Cell> {
    let mut sorted: Vec<&'a Cell> = cells.to_vec();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let mut selected: Vec<&'a Cell> = Vec::new();
    for c in sorted {
        if !selected
            .iter()
            .any(|s| haversine_km(s.lat, s.lon, c.lat, c.lon) < min_separation_km)
        {
            selected.push(c);
        }
    }
    selected
}

/// Pool of neighbours for `cell`: all other cells farther than the thinning radius
/// from it, thinned among themselves.
pub fn spatially_thinned_neighbours<'a>(
    cell: &Cell,
    all_valid_cells: &[&'a Cell],
    min_separation_km: f64,
) -> Vec<&'a Cell> {
    let others: Vec<&'a Cell> = all_valid_cells
        .iter()
        .copied()
        .filter(|c| c.id != cell.id && haversine_km(cell.lat, cell.lon, c.lat, c.lon) >= min_separation_km)
        .collect();
    spatially_independent(&others, min_separation_km)
}

pub fn compute_heuristic_quality(cell: &Cell, max_unc: f64) -> f64 {
    let unc_ratio = (cell.vertical_uncertainty_m / max_unc.max(1e-6)).clamp(0.0, 1.0);
    (cell.valid_pixel_ratio * (1.0 - 0.5 * unc_ratio)).clamp(0.0, 1.0)
}

/// Rarity is computed on point values only: measurement uncertainty must not make a
/// cell look rarer (it did in v4.0 through the distance floor).
pub fn compute_novelty_metrics(
    cell: &Cell,
    valid_cells: &[&Cell],
    catalog: &[KnownAnalogSite],
    active_features: &[FeatureId],
    scales: &FeatureScales,
    params: &RunParameters,
) -> NoveltyMetrics {
    let quality = compute_heuristic_quality(cell, params.max_vertical_uncertainty_m);
    let pool = spatially_thinned_neighbours(cell, valid_cells, params.spatial_thinning_radius_km);

    let mut dists: Vec<f64> = pool
        .iter()
        .filter_map(|o| feature_distance(&cell.features, &o.features, active_features, scales))
        .collect();
    dists.sort_by(|a, b| a.total_cmp(b));

    let (rarity, eff_k, support, status) = if dists.is_empty() && !pool.is_empty() {
        (None, None, 0, RaritySupportStatus::NotComputable)
    } else if dists.len() < params.knn_k {
        (None, Some(dists.len()), dists.len(), RaritySupportStatus::InsufficientSpatialNeighbours)
    } else {
        let k_dist = dists[params.knn_k - 1];
        let radius = params.joint_support_radius;
        let support = dists.iter().filter(|&&d| d <= radius * 1.5).count();
        let status = if k_dist <= radius {
            RaritySupportStatus::CommonTerrestrialTerrain
        } else if support + 1 >= params.min_knn_support.max(2)
            && quality >= params.min_quality_for_supported_rarity
        {
            RaritySupportStatus::SupportedRareCombination
        } else {
            RaritySupportStatus::IsolatedOutlierCandidate
        };
        (Some(k_dist), Some(params.knn_k), support, status)
    };

    let (catalog_novelty, nearest_site_id, geo_dist_km) = if catalog.is_empty() {
        (None, None, None)
    } else {
        let mut best: Option<(f64, &str)> = None;
        let mut min_geo = f64::INFINITY;
        for site in catalog {
            if let Some(d) = feature_distance(&cell.features, &site.features, active_features, scales) {
                if best.map_or(true, |(bd, bid)| d < bd || (d == bd && site.id.as_str() < bid)) {
                    best = Some((d, site.id.as_str()));
                }
            }
            let geo = (haversine_km(cell.lat, cell.lon, site.lat, site.lon) - site.cluster_radius_km.max(0.0)).max(0.0);
            min_geo = min_geo.min(geo);
        }
        (best.map(|b| b.0), best.map(|b| b.1.to_string()), Some(min_geo))
    };

    NoveltyMetrics {
        earth_rarity_knn_distance: rarity,
        effective_k: eff_k,
        support_within_rarity_radius: support,
        rarity_status: status,
        catalog_novelty_distance: catalog_novelty,
        nearest_catalog_site_id: nearest_site_id,
        geographic_distance_km: geo_dist_km,
        heuristic_quality_score: quality,
    }
}
