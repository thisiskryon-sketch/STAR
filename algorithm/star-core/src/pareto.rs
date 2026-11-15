// star-core/src/pareto.rs
//! Generic Pareto fronts. All objectives are maximised; a missing objective (None)
//! is never treated as better or worse: candidates are compared only with others
//! that have the same objective mask (P0-7).
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParetoInput {
    pub cell_id: String,
    pub objectives: Vec<Option<f64>>,
    /// Secondary ordering inside a front (lower is listed first). Not a scalar optimum.
    pub tie_key: f64,
}

impl ParetoInput {
    pub fn mask(&self) -> Vec<bool> {
        self.objectives.iter().map(|o| o.is_some()).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParetoAssignment {
    pub cell_id: String,
    pub pareto_front_rank: usize,
    pub objective_mask: Vec<bool>,
    pub objective_names: Vec<String>,
}

pub fn dominates(a: &ParetoInput, b: &ParetoInput) -> bool {
    if a.mask() != b.mask() {
        return false;
    }
    let mut strictly = false;
    for (x, y) in a.objectives.iter().zip(&b.objectives) {
        if let (Some(x), Some(y)) = (x, y) {
            if x < y {
                return false;
            }
            if x > y {
                strictly = true;
            }
        }
    }
    strictly
}

/// Non-dominated sorting; output ordered by (front, tie_key, cell_id).
pub fn compute_pareto_fronts(inputs: &[ParetoInput], objective_names: &[&str]) -> Vec<ParetoAssignment> {
    let n = inputs.len();
    let mut rank = vec![0usize; n];
    let mut remaining: Vec<usize> = (0..n).collect();
    let mut current = 1;
    while !remaining.is_empty() {
        let front: Vec<usize> = remaining
            .iter()
            .copied()
            .filter(|&i| !remaining.iter().any(|&j| j != i && dominates(&inputs[j], &inputs[i])))
            .collect();
        for &i in &front {
            rank[i] = current;
        }
        remaining.retain(|i| !front.contains(i));
        current += 1;
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        rank[a]
            .cmp(&rank[b])
            .then(inputs[a].tie_key.total_cmp(&inputs[b].tie_key))
            .then(inputs[a].cell_id.cmp(&inputs[b].cell_id))
    });
    order
        .into_iter()
        .map(|i| ParetoAssignment {
            cell_id: inputs[i].cell_id.clone(),
            pareto_front_rank: rank[i],
            objective_mask: inputs[i].mask(),
            objective_names: objective_names.iter().map(|s| s.to_string()).collect(),
        })
        .collect()
}
