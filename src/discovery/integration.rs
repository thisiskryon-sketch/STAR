//! Adapter between the desktop application and the v0.6.2 discovery engine.
//!
//! The algorithm lives in `algorithm/star-core` unchanged. This module only loads
//! the documented input bundle and prepares its run manifest.

use star_core::assessment::{CampaignConstraints, LabFacility, SiteMechanismEvidence, SiteWindow};
use star_core::schema::{
    Cell, DataOrigin, KnownAnalogSite, MissionExperimentSpec, RunManifest, RunParameters,
    TargetProfile,
};
use star_core::{DiscoveryInputs, DiscoveryReport, discover_candidates_v4, spec_for_target};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

type Result<T> = std::result::Result<T, String>;

fn read_required<T: serde::de::DeserializeOwned>(
    dir: &Path,
    name: &str,
    hash: &mut u64,
) -> Result<T> {
    let path = dir.join(name);
    let bytes = fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    hash_bytes(hash, name.as_bytes());
    hash_bytes(hash, &bytes);
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_optional<T: serde::de::DeserializeOwned>(
    dir: &Path,
    name: &str,
    hash: &mut u64,
) -> Result<Option<T>> {
    if dir.join(name).exists() {
        read_required(dir, name, hash).map(Some)
    } else {
        Ok(None)
    }
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= *byte as u64;
        *hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
}

fn timestamp_utc() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default();
    let (days, rem) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Run the bundled v0.6.2 algorithm using an imported production data directory.
/// Required inputs are the exact files documented in `algorithm/ALGORITHM.md`.
pub fn run_discovery(data_dir: &Path, requested_target: Option<&str>) -> Result<DiscoveryReport> {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    let cells: Vec<Cell> = read_required(data_dir, "cells.json", &mut hash)?;
    let targets: Vec<TargetProfile> = read_required(data_dir, "targets.json", &mut hash)?;
    let base_spec: MissionExperimentSpec = read_required(data_dir, "mission_spec.json", &mut hash)?;
    let catalog: Vec<KnownAnalogSite> =
        read_optional(data_dir, "catalog.json", &mut hash)?.unwrap_or_default();
    let evidence: Vec<SiteMechanismEvidence> =
        read_optional(data_dir, "evidence.json", &mut hash)?.unwrap_or_default();
    let labs: Vec<LabFacility> =
        read_optional(data_dir, "labs.json", &mut hash)?.unwrap_or_default();
    let site_windows: Vec<SiteWindow> =
        read_optional(data_dir, "site_windows.json", &mut hash)?.unwrap_or_default();
    let params: RunParameters =
        read_optional(data_dir, "params.json", &mut hash)?.unwrap_or_default();
    let constraints: CampaignConstraints =
        read_optional(data_dir, "campaign_constraints.json", &mut hash)?.unwrap_or(
            CampaignConstraints {
                max_assets: Some(5),
                ..Default::default()
            },
        );

    let target_id = requested_target
        .map(str::to_owned)
        .or_else(|| base_spec.target_profile_id.clone());
    let target = match target_id.as_deref() {
        Some(id) => targets.iter().find(|target| target.id == id)
            .ok_or_else(|| format!("target '{id}' was not found in targets.json"))?,
        None if targets.len() == 1 => &targets[0],
        None => return Err("several targets are present; set target_profile_id in mission_spec.json or choose one in the app".into()),
    };
    let spec = if target_id.is_none() && targets.len() > 1 {
        spec_for_target(&base_spec, target)
    } else {
        base_spec
    };

    hash_bytes(
        &mut hash,
        serde_json::to_string(&params)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    );
    hash_bytes(
        &mut hash,
        serde_json::to_string(&constraints)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    );
    hash_bytes(&mut hash, target.id.as_bytes());
    let dataset_hash = format!("fnv1a64:{hash:016x}");
    let manifest = RunManifest {
        run_id: format!("star-{}-{hash:016x}", spec.experiment_id),
        timestamp_utc: timestamp_utc(),
        code_revision: "star_backend adapter / star-core 0.6.2".into(),
        calibration_version: format!("earth-qc-p01-p99@{hash:016x}"),
        dataset_hash,
        random_seed: 42,
        is_synthetic_demo: cells
            .iter()
            .any(|cell| cell.data_origin == DataOrigin::Synthetic),
        validation_protocol: "v0.6.2 mission assessment; input labels are not inferred".into(),
    };

    discover_candidates_v4(DiscoveryInputs {
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
    })
    .map_err(|e| e.to_string())
}
