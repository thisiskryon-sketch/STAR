#[derive(Debug, Clone)]
pub struct RasterGrid {
    pub width: usize,
    pub height: usize,
    pub cell_size_m: f64,
    pub elevation: Vec<f64>,
    valid_mask: Vec<bool>,
}

impl RasterGrid {
    pub fn new(
        width: usize,
        height: usize,
        cell_size_m: f64,
        elevation: Vec<f64>,
    ) -> Result<Self, String> {
        let valid_mask = vec![true; elevation.len()];

        Self::new_masked(width, height, cell_size_m, elevation, valid_mask)
    }

    pub fn new_masked(
        width: usize,
        height: usize,
        cell_size_m: f64,
        mut elevation: Vec<f64>,
        valid_mask: Vec<bool>,
    ) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("Raster dimensions cannot be zero".to_string());
        }

        if !cell_size_m.is_finite() || cell_size_m <= 0.0 {
            return Err("Cell size must be positive and finite".to_string());
        }

        let expected = width
            .checked_mul(height)
            .ok_or_else(|| "Raster dimensions overflow".to_string())?;

        if elevation.len() != expected {
            return Err("Elevation data size does not match dimensions".to_string());
        }

        if valid_mask.len() != expected {
            return Err("Validity mask size does not match dimensions".to_string());
        }

        for (value, valid) in elevation.iter_mut().zip(valid_mask.iter()) {
            if *valid && !value.is_finite() {
                return Err("Valid elevation contains non-finite values".to_string());
            }

            if !*valid {
                *value = 0.0;
            }
        }

        Ok(Self {
            width,
            height,
            cell_size_m,
            elevation,
            valid_mask,
        })
    }

    pub fn get(&self, x: usize, y: usize) -> Option<f64> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let index = y * self.width + x;

        if !self.valid_mask[index] {
            return None;
        }

        Some(self.elevation[index])
    }

    pub fn is_valid(&self, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }

        self.valid_mask[y * self.width + x]
    }

    pub fn valid_cells(&self) -> usize {
        self.valid_mask.iter().filter(|&&v| v).count()
    }

    pub fn min_max(&self) -> Option<(f64, f64)> {
        let mut values = self
            .elevation
            .iter()
            .zip(self.valid_mask.iter())
            .filter_map(|(&value, &valid)| valid.then_some(value));

        let first = values.next()?;
        let mut min = first;
        let mut max = first;

        for value in values {
            min = min.min(value);
            max = max.max(value);
        }

        Some((min, max))
    }
}

/// Aggregate square pixels to a coarser square grid using valid-pixel averages.
/// Incomplete output blocks remain NoData when their valid coverage is below the
/// supplied threshold; they are never filled with an invented zero elevation.
pub fn downsample_average(
    grid: &RasterGrid,
    factor: usize,
    min_valid_ratio: f64,
) -> Result<RasterGrid, String> {
    if factor <= 1 {
        return Err("Downsample factor must be greater than one".into());
    }
    if !(min_valid_ratio.is_finite() && (0.0..=1.0).contains(&min_valid_ratio)) {
        return Err("Minimum valid-pixel ratio must be within 0..=1".into());
    }
    let width = grid.width / factor;
    let height = grid.height / factor;
    if width == 0 || height == 0 {
        return Err("Downsample factor is larger than the raster".into());
    }
    let block_size = factor
        .checked_mul(factor)
        .ok_or("Downsample block size overflow")?;
    let mut values = Vec::with_capacity(width * height);
    let mut mask = Vec::with_capacity(width * height);
    for out_y in 0..height {
        for out_x in 0..width {
            let mut sum = 0.0;
            let mut valid = 0usize;
            for dy in 0..factor {
                for dx in 0..factor {
                    if let Some(value) = grid.get(out_x * factor + dx, out_y * factor + dy) {
                        sum += value;
                        valid += 1;
                    }
                }
            }
            let is_valid = valid > 0 && valid as f64 / block_size as f64 >= min_valid_ratio;
            values.push(if is_valid { sum / valid as f64 } else { 0.0 });
            mask.push(is_valid);
        }
    }
    RasterGrid::new_masked(
        width,
        height,
        grid.cell_size_m * factor as f64,
        values,
        mask,
    )
}
