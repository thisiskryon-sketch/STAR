use crate::features::terrain_fingerprint::calculate_terrain_fingerprint;
use crate::models::terrain::TerrainFingerprint;
use crate::preprocessing::dem::RasterGrid;

/// One processed DEM cell and its terrain fingerprint.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainSample {
    pub x: usize,
    pub y: usize,
    pub fingerprint: TerrainFingerprint,
}

/// Results of processing an entire DEM raster.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainExtractionReport {
    pub width: usize,
    pub height: usize,
    pub radius_cells: usize,
    pub samples: Vec<TerrainSample>,
    pub skipped_cells: usize,
}

impl TerrainExtractionReport {
    pub fn processed_cells(&self) -> usize {
        self.samples.len()
    }

    pub fn total_cells(&self) -> usize {
        self.width * self.height
    }

    pub fn coverage_fraction(&self) -> f64 {
        let total = self.total_cells();

        if total == 0 {
            return 0.0;
        }

        self.processed_cells() as f64 / total as f64
    }

    pub fn mean_elevation_m(&self) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }

        let sum: f64 = self
            .samples
            .iter()
            .filter_map(|sample| sample.fingerprint.elevation_m)
            .sum();

        let count = self
            .samples
            .iter()
            .filter(|sample| sample.fingerprint.elevation_m.is_some())
            .count();

        if count == 0 {
            return None;
        }

        Some(sum / count as f64)
    }
}

/// Extracts terrain fingerprints from every eligible DEM cell.
///
/// Cells without enough surrounding terrain are skipped.
pub fn extract_terrain_batch(
    grid: &RasterGrid,
    radius_cells: usize,
) -> Result<TerrainExtractionReport, String> {
    if radius_cells == 0 {
        return Err("Radius must be greater than zero".to_string());
    }

    let total_cells = grid
        .width
        .checked_mul(grid.height)
        .ok_or_else(|| "Raster dimensions overflow".to_string())?;

    let mut samples = Vec::new();

    let diameter = radius_cells
        .checked_mul(2)
        .ok_or_else(|| "Radius is too large".to_string())?;

    if diameter < grid.width && diameter < grid.height {
        for y in radius_cells..(grid.height - radius_cells) {
            for x in radius_cells..(grid.width - radius_cells) {
                if let Some(fingerprint) = calculate_terrain_fingerprint(grid, x, y, radius_cells) {
                    samples.push(TerrainSample { x, y, fingerprint });
                }
            }
        }
    }

    let skipped_cells = total_cells - samples.len();

    Ok(TerrainExtractionReport {
        width: grid.width,
        height: grid.height,
        radius_cells,
        samples,
        skipped_cells,
    })
}
