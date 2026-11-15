use super::validation::{ModelValidationError, check_non_negative, check_positive};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThermalResponseSignature {
    pub heating_time_hours: Option<f64>,
    pub cooling_time_hours: Option<f64>,
    pub diurnal_temperature_range_k: Option<f64>,
    pub peak_temperature_lag_hours: Option<f64>,
    pub heating_rate_k_per_hour: Option<f64>,
    pub cooling_rate_k_per_hour: Option<f64>,
    pub seasonal_amplitude_k: Option<f64>,
}

impl ThermalResponseSignature {
    pub fn validate(&self) -> Result<(), ModelValidationError> {
        check_positive(self.heating_time_hours, "heating_time_hours")?;
        check_positive(self.cooling_time_hours, "cooling_time_hours")?;

        check_non_negative(
            self.diurnal_temperature_range_k,
            "diurnal_temperature_range_k",
        )?;

        check_non_negative(self.heating_rate_k_per_hour, "heating_rate_k_per_hour")?;

        check_non_negative(self.cooling_rate_k_per_hour, "cooling_rate_k_per_hour")?;

        check_non_negative(self.seasonal_amplitude_k, "seasonal_amplitude_k")?;

        super::validation::check_finite(
            self.peak_temperature_lag_hours,
            "peak_temperature_lag_hours",
        )?;

        Ok(())
    }
}
