use super::mission::GeoCoordinate;
use super::terrain::TerrainFingerprint;
use super::thermal::ThermalResponseSignature;
use super::validation::{ModelValidationError, check_range};

#[derive(Debug, Clone)]
pub struct DataQuality {
    pub valid_observation_fraction: Option<f64>,
    pub source_ids: Vec<String>,
}

impl DataQuality {
    pub fn validate(&self) -> Result<(), ModelValidationError> {
        check_range(
            self.valid_observation_fraction,
            0.0,
            1.0,
            "valid_observation_fraction",
        )?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EarthCandidate {
    pub id: String,
    pub coordinates: GeoCoordinate,
    pub region_name: Option<String>,
    pub terrain: TerrainFingerprint,
    pub thermal: ThermalResponseSignature,
    pub data_quality: DataQuality,
}

impl EarthCandidate {
    pub fn validate(&self) -> Result<(), ModelValidationError> {
        if self.id.trim().is_empty() {
            return Err(ModelValidationError::new(
                "id",
                "candidate ID cannot be empty",
            ));
        }

        self.coordinates.validate()?;
        self.terrain.validate()?;
        self.thermal.validate()?;
        self.data_quality.validate()?;

        Ok(())
    }
}
