//! Produces the low-sun features required by the bundled v0.6.2 data contract.
//! The ray-casting definition itself is delegated to `star-core` unchanged.

use crate::features::multiscale::TileFeatures;
use crate::preprocessing::dem::RasterGrid;
use star_core::shadows::compute_low_sun_obstruction;

pub const DEFAULT_MAX_RADIUS_M: f64 = 3_000.0;
pub const DEFAULT_CLEAN_RAY_RATIO_THRESHOLD: f64 = 0.85;

/// Compute obstruction ratios for each exported terrain tile.
/// `None` means the tile lacks the padded, valid rays required for an honest value.
pub fn obstruction_by_tile(
    grid: &RasterGrid,
    tiles: &[TileFeatures],
    max_radius_m: f64,
    clean_ray_ratio_threshold: f64,
) -> Result<Vec<Option<[f64; 3]>>, String> {
    if !max_radius_m.is_finite() || max_radius_m <= 0.0 {
        return Err("Low-sun ray radius must be positive and finite".into());
    }
    let cell_count = grid
        .width
        .checked_mul(grid.height)
        .ok_or_else(|| "DEM dimensions overflow".to_owned())?;
    if cell_count > 2_000_000 {
        return Err(
            "DEM is too large for the in-app low-sun raycaster; prepare a padded 30 m region first"
                .into(),
        );
    }

    let dem: Vec<f64> = (0..cell_count)
        .map(|index| {
            let x = index % grid.width;
            let y = index / grid.width;
            grid.get(x, y).unwrap_or(f64::NAN)
        })
        .collect();
    let raycast = compute_low_sun_obstruction(
        &dem,
        grid.height,
        grid.width,
        grid.cell_size_m,
        max_radius_m,
        clean_ray_ratio_threshold,
    );
    let pad = (max_radius_m / grid.cell_size_m).ceil() as usize;
    let mut output = Vec::with_capacity(tiles.len());

    for tile in tiles {
        let right = tile.x0.saturating_add(tile.width);
        let bottom = tile.y0.saturating_add(tile.height);
        if tile.x0 < pad
            || tile.y0 < pad
            || right.saturating_add(pad) > grid.width
            || bottom.saturating_add(pad) > grid.height
            || tile.x0 == 0
            || tile.y0 == 0
            || right >= grid.width
            || bottom >= grid.height
        {
            output.push(None);
            continue;
        }

        let mut valid_rays = 0usize;
        let mut total_rays = 0usize;
        let mut blocked = [0usize; 3];
        for y in tile.y0..bottom {
            for x in tile.x0..right {
                if !grid.is_valid(x, y) {
                    total_rays += 8;
                    continue;
                }
                let ray_index = (y - 1) * (grid.width - 2) + (x - 1);
                let Some(horizons) = raycast.azimuth_max_horizon_deg.get(ray_index) else {
                    continue;
                };
                for horizon in horizons {
                    total_rays += 1;
                    if let Some(angle) = horizon {
                        valid_rays += 1;
                        if *angle > 1.0 {
                            blocked[0] += 1;
                        }
                        if *angle > 2.0 {
                            blocked[1] += 1;
                        }
                        if *angle > 3.0 {
                            blocked[2] += 1;
                        }
                    }
                }
            }
        }
        let clean_ratio = if total_rays == 0 {
            0.0
        } else {
            valid_rays as f64 / total_rays as f64
        };
        if valid_rays == 0 || clean_ratio < clean_ray_ratio_threshold {
            output.push(None);
            continue;
        }
        output.push(Some(blocked.map(|count| count as f64 / valid_rays as f64)));
    }
    Ok(output)
}
