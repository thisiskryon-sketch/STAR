use crate::preprocessing::dem::RasterGrid;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlopeResult {
    pub slope_deg: f64,
    pub gradient_x: f64,
    pub gradient_y: f64,
}

pub fn calculate_slope(grid: &RasterGrid, x: usize, y: usize) -> Option<SlopeResult> {
    if x == 0 || y == 0 {
        return None;
    }

    if x + 1 >= grid.width || y + 1 >= grid.height {
        return None;
    }

    let z_left = grid.get(x - 1, y)?;
    let z_right = grid.get(x + 1, y)?;
    let z_top = grid.get(x, y - 1)?;
    let z_bottom = grid.get(x, y + 1)?;

    let resolution = grid.cell_size_m;

    let gradient_x = (z_right - z_left) / (2.0 * resolution);
    let gradient_y = (z_bottom - z_top) / (2.0 * resolution);

    let gradient_magnitude = gradient_x.hypot(gradient_y);

    let slope_deg = gradient_magnitude.atan().to_degrees();

    Some(SlopeResult {
        slope_deg,
        gradient_x,
        gradient_y,
    })
}

pub fn calculate_local_relief(
    grid: &RasterGrid,
    x: usize,
    y: usize,
    radius_cells: usize,
) -> Option<f64> {
    calculate_local_metrics(grid, x, y, radius_cells).map(|(relief, _)| relief)
}

fn calculate_local_metrics(
    grid: &RasterGrid,
    x: usize,
    y: usize,
    radius_cells: usize,
) -> Option<(f64, f64)> {
    if radius_cells == 0 {
        return None;
    }

    let x_start = x.checked_sub(radius_cells)?;
    let y_start = y.checked_sub(radius_cells)?;
    let x_end = x.checked_add(radius_cells)?;
    let y_end = y.checked_add(radius_cells)?;

    if x_end >= grid.width || y_end >= grid.height {
        return None;
    }

    let mut min_elevation = f64::INFINITY;
    let mut max_elevation = f64::NEG_INFINITY;
    let mut count = 0.0;
    let mut mean = 0.0;
    let mut sum_squared_differences = 0.0;

    for row in y_start..=y_end {
        for col in x_start..=x_end {
            let elevation = grid.get(col, row)?;
            min_elevation = min_elevation.min(elevation);
            max_elevation = max_elevation.max(elevation);

            count += 1.0;
            let delta = elevation - mean;
            mean += delta / count;
            sum_squared_differences += delta * (elevation - mean);
        }
    }

    let roughness = (sum_squared_differences / count).max(0.0).sqrt();
    Some((max_elevation - min_elevation, roughness))
}

pub fn calculate_roughness(
    grid: &RasterGrid,
    x: usize,
    y: usize,
    radius_cells: usize,
) -> Option<f64> {
    calculate_local_metrics(grid, x, y, radius_cells).map(|(_, roughness)| roughness)
}

pub fn calculate_curvature(grid: &RasterGrid, x: usize, y: usize) -> Option<f64> {
    if x == 0 || y == 0 {
        return None;
    }

    let x_next = x.checked_add(1)?;
    let y_next = y.checked_add(1)?;

    if x_next >= grid.width || y_next >= grid.height {
        return None;
    }

    let center = grid.get(x, y)?;
    let left = grid.get(x - 1, y)?;
    let right = grid.get(x_next, y)?;
    let top = grid.get(x, y - 1)?;
    let bottom = grid.get(x, y_next)?;

    let cell_size_squared = grid.cell_size_m.powi(2);

    let d2x = (right - 2.0 * center + left) / cell_size_squared;
    let d2y = (bottom - 2.0 * center + top) / cell_size_squared;

    Some(d2x + d2y)
}

pub fn calculate_terrain_fingerprint(
    grid: &RasterGrid,
    x: usize,
    y: usize,
    radius_cells: usize,
) -> Option<crate::models::terrain::TerrainFingerprint> {
    let elevation = grid.get(x, y)?;
    let slope = calculate_slope(grid, x, y)?;
    let (local_relief, roughness) = calculate_local_metrics(grid, x, y, radius_cells)?;
    let curvature = calculate_curvature(grid, x, y)?;

    Some(crate::models::terrain::TerrainFingerprint {
        elevation_m: Some(elevation),
        slope_deg: Some(slope.slope_deg),
        local_relief_m: Some(local_relief),
        roughness_m: Some(roughness),
        curvature: Some(curvature),
    })
}
