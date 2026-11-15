// star-core/src/dna.rs
//! Observable Match profile of a candidate (requirement level, FR-06).
use crate::envelope::{MissionDistance, RequirementScore};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalogDna {
    pub cell_id: String,
    pub mission_distance: f64,
    pub mission_distance_sigma: Option<f64>,
    /// Share of requirement weight whose acceptable range is met (0..1).
    pub weighted_requirement_coverage: f64,
    /// Requirements outside their acceptable range, worst (weighted z) first.
    pub limiting_requirements: Vec<String>,
    pub bottleneck_requirement: Option<String>,
    pub breakdown: Vec<RequirementScore>,
}

pub fn build_analog_dna(cell_id: &str, md: &MissionDistance) -> AnalogDna {
    let coverage: f64 = md
        .per_requirement
        .iter()
        .filter(|r| r.within_range)
        .map(|r| r.effective_weight)
        .sum();
    let mut by_impact: Vec<&RequirementScore> = md.per_requirement.iter().collect();
    by_impact.sort_by(|a, b| {
        (b.z * b.effective_weight)
            .total_cmp(&(a.z * a.effective_weight))
            .then(a.requirement_id.cmp(&b.requirement_id))
    });
    AnalogDna {
        cell_id: cell_id.to_string(),
        mission_distance: md.d,
        mission_distance_sigma: md.sigma_d,
        weighted_requirement_coverage: coverage,
        limiting_requirements: by_impact
            .iter()
            .filter(|r| !r.within_range)
            .map(|r| r.requirement_id.clone())
            .collect(),
        bottleneck_requirement: by_impact.first().map(|r| r.requirement_id.clone()),
        breakdown: md.per_requirement.clone(),
    }
}
