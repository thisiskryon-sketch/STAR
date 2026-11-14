use super::validation::{ModelValidationError, check_non_negative, check_range};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TerrainFingerprint {
    pub elevation_m: Option<f64>,
    pub slope_deg: Option<f64>,
    pub local_relief_m: Option<f64>,
    pub roughness_m: Option<f64>,
    pub curvature: Option<f64>,
}

impl TerrainFingerprint {
    pub fn validate(&self) -> Result<(), ModelValidationError> {
        check_range(self.slope_deg, 0.0, 90.0, "slope_deg")?;

        check_non_negative(self.local_relief_m, "local_relief_m")?;
        check_non_negative(self.roughness_m, "roughness_m")?;

        check_range(self.elevation_m, -f64::MAX, f64::MAX, "elevation_m")?;

        check_range(self.curvature, -f64::MAX, f64::MAX, "curvature")?;

        Ok(())
    }
}
