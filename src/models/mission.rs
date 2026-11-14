use super::terrain::TerrainFingerprint;
use super::thermal::ThermalResponseSignature;
use super::validation::ModelValidationError;
use super::validation::check_range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CelestialBody {
    Moon,
    Mars,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeoCoordinate {
    pub latitude_deg: f64,
    pub longitude_deg: f64,
}

impl GeoCoordinate {
    pub fn validate(&self) -> Result<(), ModelValidationError> {
        check_range(Some(self.latitude_deg), -90.0, 90.0, "latitude_deg")?;

        check_range(Some(self.longitude_deg), -180.0, 180.0, "longitude_deg")?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MissionProfile {
    pub id: String,
    pub name: String,
    pub target_body: CelestialBody,
    pub center: GeoCoordinate,
    pub terrain: TerrainFingerprint,
    pub thermal: ThermalResponseSignature,
}

impl MissionProfile {
    pub fn validate(&self) -> Result<(), ModelValidationError> {
        if self.id.trim().is_empty() {
            return Err(ModelValidationError::new(
                "id",
                "mission ID cannot be empty",
            ));
        }

        if self.name.trim().is_empty() {
            return Err(ModelValidationError::new(
                "name",
                "mission name cannot be empty",
            ));
        }

        self.center.validate()?;
        self.terrain.validate()?;
        self.thermal.validate()?;

        Ok(())
    }
}
