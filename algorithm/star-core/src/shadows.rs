// star-core/src/shadows.rs
use serde::{Deserialize, Serialize};

pub const AZIMUTH_BINS_DEG: [f64; 8] = [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShadowCoverageStatus {
    NominalCoverage,
    HighNoDataUncertainty,
    NoValidRays,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaycastResult {
    pub obstruction_1deg: Option<f64>,
    pub obstruction_2deg: Option<f64>,
    pub obstruction_3deg: Option<f64>,
    pub clean_ray_ratio: f64,
    pub status: ShadowCoverageStatus,
    pub azimuth_max_horizon_deg: Vec<[Option<f64>; 8]>,
}

/// Low-sun horizon obstruction on a north-up DEM grid.
///
/// Conventions: `dem` is row-major, row 0 = north edge, square pixels of `pixel_size_m`.
/// Azimuth is clockwise from north: 0° = -row (north), 90° = +col (east).
/// Rays step one pixel length along the azimuth and sample the nearest pixel.
/// A ray that leaves the raster before `max_radius_m` is truncated (counted as clean):
/// cells near the ROI edge therefore underestimate obstruction; pad the ROI by `max_radius_m`.
/// Body curvature is ignored (Moon: ~0.17° at 10 km), which matters only for long radii.
pub fn compute_low_sun_obstruction(
    dem: &[f64],
    rows: usize,
    cols: usize,
    pixel_size_m: f64,
    max_radius_m: f64,
    clean_ray_ratio_threshold: f64,
) -> RaycastResult {
    let dims_valid = rows >= 3 && cols >= 3 && dem.len() == rows * cols && pixel_size_m > 0.0;
    if !dims_valid {
        return RaycastResult {
            obstruction_1deg: None,
            obstruction_2deg: None,
            obstruction_3deg: None,
            clean_ray_ratio: 0.0,
            status: ShadowCoverageStatus::NoValidRays,
            azimuth_max_horizon_deg: Vec::new(),
        };
    }

    let max_steps = ((max_radius_m / pixel_size_m).ceil() as usize).max(1);
    let mut total_rays = 0usize;
    let mut clean_rays = 0usize;
    let mut obstructed_1 = 0usize;
    let mut obstructed_2 = 0usize;
    let mut obstructed_3 = 0usize;
    let mut azimuth_matrix = Vec::new();

    for r in 1..(rows - 1) {
        for c in 1..(cols - 1) {
            let z0 = dem[r * cols + c];
            if !z0.is_finite() {
                azimuth_matrix.push([None; 8]);
                total_rays += 8;
                continue;
            }

            let mut cell_horizons = [None; 8];
            for (az_idx, &az_deg) in AZIMUTH_BINS_DEG.iter().enumerate() {
                total_rays += 1;
                let az_rad = az_deg.to_radians();
                let dr = -az_rad.cos();
                let dc = az_rad.sin();

                let mut max_angle_deg = 0.0_f64;
                let mut hit_nodata = false;
                let mut steps_taken = 0usize;

                for step in 1..=max_steps {
                    let nr = (r as f64 + dr * (step as f64)).round() as isize;
                    let nc = (c as f64 + dc * (step as f64)).round() as isize;
                    let in_bounds =
                        nr >= 0 && nr < (rows as isize) && nc >= 0 && nc < (cols as isize);
                    if !in_bounds {
                        break;
                    }
                    let z = dem[(nr as usize) * cols + (nc as usize)];
                    if !z.is_finite() {
                        hit_nodata = true;
                        break;
                    }
                    steps_taken += 1;
                    let dist_m = (((nr - r as isize).pow(2) + (nc - c as isize).pow(2)) as f64)
                        .sqrt()
                        * pixel_size_m;
                    if dist_m > 0.0 {
                        let elev_deg = ((z - z0) / dist_m).atan().to_degrees();
                        if elev_deg > max_angle_deg {
                            max_angle_deg = elev_deg;
                        }
                    }
                }

                if !hit_nodata && steps_taken > 0 {
                    clean_rays += 1;
                    cell_horizons[az_idx] = Some(max_angle_deg);
                    if max_angle_deg > 1.0 {
                        obstructed_1 += 1;
                    }
                    if max_angle_deg > 2.0 {
                        obstructed_2 += 1;
                    }
                    if max_angle_deg > 3.0 {
                        obstructed_3 += 1;
                    }
                }
            }
            azimuth_matrix.push(cell_horizons);
        }
    }
    if clean_rays == 0 || total_rays == 0 {
        return RaycastResult {
            obstruction_1deg: None,
            obstruction_2deg: None,
            obstruction_3deg: None,
            clean_ray_ratio: 0.0,
            status: ShadowCoverageStatus::NoValidRays,
            azimuth_max_horizon_deg: azimuth_matrix,
        };
    }

    let clean_ratio = clean_rays as f64 / total_rays as f64;
    let status = if clean_ratio >= clean_ray_ratio_threshold {
        ShadowCoverageStatus::NominalCoverage
    } else {
        ShadowCoverageStatus::HighNoDataUncertainty
    };

    let denom = clean_rays as f64;
    RaycastResult {
        obstruction_1deg: Some(obstructed_1 as f64 / denom),
        obstruction_2deg: Some(obstructed_2 as f64 / denom),
        obstruction_3deg: Some(obstructed_3 as f64 / denom),
        clean_ray_ratio: clean_ratio,
        status,
        azimuth_max_horizon_deg: azimuth_matrix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flat_plane_zero_obstruction() {
        let dem = vec![10.0; 25];
        let res = compute_low_sun_obstruction(&dem, 5, 5, 10.0, 50.0, 0.85);
        assert_eq!(res.status, ShadowCoverageStatus::NominalCoverage);
        assert_eq!(res.obstruction_1deg, Some(0.0));
        assert_eq!(res.obstruction_2deg, Some(0.0));
        assert_eq!(res.obstruction_3deg, Some(0.0));
    }

    #[test]
    fn test_single_ridge_analytical_angle_and_monotonicity() {
        let mut dem = vec![0.0; 25];
        for c in 0..5 {
            dem[c] = 0.45;
        }
        let res = compute_low_sun_obstruction(&dem, 5, 5, 10.0, 50.0, 0.85);
        let o1 = res.obstruction_1deg.unwrap();
        let o2 = res.obstruction_2deg.unwrap();
        let o3 = res.obstruction_3deg.unwrap();
        assert!(
            o1 >= o2 && o2 >= o3,
            "Monotonicity violated: {} >= {} >= {}",
            o1,
            o2,
            o3
        );
        let center_north_deg = res.azimuth_max_horizon_deg[4][0].unwrap();
        let expected_deg = (0.45_f64 / 20.0_f64).atan().to_degrees();
        assert!((center_north_deg - expected_deg).abs() < 1e-6);
    }

    #[test]
    fn test_nodata_exclusion_triggers_high_uncertainty() {
        let mut dem = vec![0.0; 25];
        dem[0] = f64::NAN;
        dem[1] = f64::NAN;
        dem[2] = f64::NAN;
        dem[5] = f64::NAN;
        let res = compute_low_sun_obstruction(&dem, 5, 5, 10.0, 50.0, 0.90);
        assert_eq!(res.status, ShadowCoverageStatus::HighNoDataUncertainty);
        assert!(res.clean_ray_ratio < 0.90);
    }
}