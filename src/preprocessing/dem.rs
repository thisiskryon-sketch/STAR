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
