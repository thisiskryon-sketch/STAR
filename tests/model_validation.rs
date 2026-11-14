use star_backend::models::candidate::{DataQuality, EarthCandidate};
use star_backend::models::mission::{CelestialBody, GeoCoordinate, MissionProfile};
use star_backend::models::terrain::TerrainFingerprint;
use star_backend::models::thermal::ThermalResponseSignature;
use star_backend::preprocessing::dem::RasterGrid;

fn valid_coordinate() -> GeoCoordinate {
    GeoCoordinate {
        latitude_deg: 45.0,
        longitude_deg: 30.0,
    }
}

fn valid_terrain() -> TerrainFingerprint {
    TerrainFingerprint {
        elevation_m: Some(1500.0),
        slope_deg: Some(25.0),
        local_relief_m: Some(120.0),
        roughness_m: Some(4.5),
        curvature: Some(0.02),
    }
}

fn valid_thermal() -> ThermalResponseSignature {
    ThermalResponseSignature {
        heating_time_hours: Some(3.0),
        cooling_time_hours: Some(5.0),
        diurnal_temperature_range_k: Some(80.0),
        peak_temperature_lag_hours: Some(1.5),
        heating_rate_k_per_hour: Some(12.0),
        cooling_rate_k_per_hour: Some(8.0),
        seasonal_amplitude_k: Some(30.0),
    }
}

fn valid_candidate() -> EarthCandidate {
    EarthCandidate {
        id: "earth-001".to_string(),
        coordinates: valid_coordinate(),
        region_name: Some("Atacama".to_string()),
        terrain: valid_terrain(),
        thermal: valid_thermal(),
        data_quality: DataQuality {
            valid_observation_fraction: Some(0.95),
            source_ids: vec!["ERA5-Land".to_string()],
        },
    }
}

#[test]
fn valid_coordinates_are_accepted() {
    assert!(valid_coordinate().validate().is_ok());
}

#[test]
fn invalid_latitude_is_rejected() {
    let coordinate = GeoCoordinate {
        latitude_deg: 95.0,
        longitude_deg: 30.0,
    };

    assert!(coordinate.validate().is_err());
}

#[test]
fn nan_coordinates_are_rejected() {
    let coordinate = GeoCoordinate {
        latitude_deg: f64::NAN,
        longitude_deg: 30.0,
    };

    assert!(coordinate.validate().is_err());
}

#[test]
fn negative_slope_is_rejected() {
    let terrain = TerrainFingerprint {
        slope_deg: Some(-10.0),
        ..Default::default()
    };

    assert!(terrain.validate().is_err());
}

#[test]
fn negative_heating_time_is_rejected() {
    let thermal = ThermalResponseSignature {
        heating_time_hours: Some(-2.0),
        ..Default::default()
    };

    assert!(thermal.validate().is_err());
}

#[test]
fn incomplete_measurements_are_allowed() {
    let terrain = TerrainFingerprint::default();
    let thermal = ThermalResponseSignature::default();

    assert!(terrain.validate().is_ok());
    assert!(thermal.validate().is_ok());
}

#[test]
fn invalid_data_quality_is_rejected() {
    let quality = DataQuality {
        valid_observation_fraction: Some(1.5),
        source_ids: vec![],
    };

    assert!(quality.validate().is_err());
}

#[test]
fn valid_earth_candidate_is_accepted() {
    assert!(valid_candidate().validate().is_ok());
}

#[test]
fn invalid_earth_candidate_is_rejected() {
    let mut candidate = valid_candidate();
    candidate.coordinates.latitude_deg = 100.0;

    assert!(candidate.validate().is_err());
}

#[test]
fn valid_mission_profile_is_accepted() {
    let mission = MissionProfile {
        id: "moon-south-pole-001".to_string(),
        name: "Lunar South Pole".to_string(),
        target_body: CelestialBody::Moon,
        center: valid_coordinate(),
        terrain: valid_terrain(),
        thermal: valid_thermal(),
    };

    assert!(mission.validate().is_ok());
}

use star_backend::features::terrain_fingerprint::calculate_slope;

#[test]
fn flat_surface_has_zero_slope() {
    let grid = RasterGrid::new(
        3,
        3,
        1.0,
        vec![
            100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0,
        ],
    )
    .unwrap();

    let result = calculate_slope(&grid, 1, 1).unwrap();

    assert!(result.slope_deg.abs() < 1e-10);
}

#[test]
fn slope_of_45_degrees_is_calculated() {
    let grid = RasterGrid::new(
        3,
        3,
        1.0,
        vec![99.0, 100.0, 101.0, 99.0, 100.0, 101.0, 99.0, 100.0, 101.0],
    )
    .unwrap();

    let result = calculate_slope(&grid, 1, 1).unwrap();

    assert!((result.slope_deg - 45.0).abs() < 1e-10);
}

#[test]
fn slope_is_unavailable_at_grid_boundary() {
    let grid = RasterGrid::new(3, 3, 1.0, vec![100.0; 9]).unwrap();

    assert!(calculate_slope(&grid, 0, 0).is_none());
}
use star_backend::features::terrain_fingerprint::calculate_local_relief;

#[test]
fn flat_terrain_has_zero_local_relief() {
    let grid = RasterGrid::new(3, 3, 1.0, vec![100.0; 9]).unwrap();

    let result = calculate_local_relief(&grid, 1, 1, 1);

    assert_eq!(result, Some(0.0));
}

#[test]
fn local_relief_is_calculated_correctly() {
    let grid = RasterGrid::new(
        3,
        3,
        1.0,
        vec![
            100.0, 105.0, 110.0, 102.0, 107.0, 112.0, 104.0, 109.0, 114.0,
        ],
    )
    .unwrap();

    let result = calculate_local_relief(&grid, 1, 1, 1);

    assert_eq!(result, Some(14.0));
}

#[test]
fn local_relief_is_unavailable_at_boundary() {
    let grid = RasterGrid::new(3, 3, 1.0, vec![100.0; 9]).unwrap();

    assert_eq!(calculate_local_relief(&grid, 0, 0, 1), None);
}
use star_backend::features::terrain_fingerprint::calculate_roughness;

#[test]
fn flat_terrain_has_zero_roughness() {
    let grid = RasterGrid::new(3, 3, 1.0, vec![100.0; 9]).unwrap();

    let result = calculate_roughness(&grid, 1, 1, 1);

    assert_eq!(result, Some(0.0));
}

#[test]
fn roughness_is_calculated_correctly() {
    let grid =
        RasterGrid::new(3, 3, 1.0, vec![1.0, 1.0, 1.0, 1.0, 9.0, 1.0, 1.0, 1.0, 1.0]).unwrap();

    let result = calculate_roughness(&grid, 1, 1, 1).unwrap();

    let expected = (512.0_f64 / 81.0).sqrt();

    assert!((result - expected).abs() < 1e-10);
}

#[test]
fn roughness_is_unavailable_at_boundary() {
    let grid = RasterGrid::new(3, 3, 1.0, vec![100.0; 9]).unwrap();

    assert_eq!(calculate_roughness(&grid, 0, 0, 1), None);
}

#[test]
fn flat_surface_has_zero_curvature() {
    let grid = RasterGrid::new(3, 3, 1.0, vec![100.0; 9]).unwrap();

    let result =
        star_backend::features::terrain_fingerprint::calculate_curvature(&grid, 1, 1).unwrap();

    assert!(result.abs() < 1e-10);
}

#[test]
fn concave_surface_has_positive_curvature() {
    let grid =
        RasterGrid::new(3, 3, 1.0, vec![1.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();

    let result =
        star_backend::features::terrain_fingerprint::calculate_curvature(&grid, 1, 1).unwrap();

    assert!((result - 4.0).abs() < 1e-10);
}

#[test]
fn curvature_is_unavailable_at_boundary() {
    let grid = RasterGrid::new(3, 3, 1.0, vec![100.0; 9]).unwrap();

    let result = star_backend::features::terrain_fingerprint::calculate_curvature(&grid, 0, 0);

    assert!(result.is_none());
}

#[test]
fn aggregate_fingerprint_is_complete() {
    let grid = RasterGrid::new(5, 5, 1.0, vec![10.0; 25]).unwrap();

    let fingerprint =
        star_backend::features::terrain_fingerprint::calculate_terrain_fingerprint(&grid, 2, 2, 1)
            .unwrap();

    assert_eq!(fingerprint.elevation_m, Some(10.0));
    assert_eq!(fingerprint.slope_deg, Some(0.0));
    assert_eq!(fingerprint.local_relief_m, Some(0.0));
    assert_eq!(fingerprint.roughness_m, Some(0.0));
    assert_eq!(fingerprint.curvature, Some(0.0));
}

#[test]
fn aggregate_fingerprint_is_unavailable_at_boundary() {
    let grid = RasterGrid::new(5, 5, 1.0, vec![10.0; 25]).unwrap();

    let fingerprint =
        star_backend::features::terrain_fingerprint::calculate_terrain_fingerprint(&grid, 0, 0, 1);

    assert!(fingerprint.is_none());
}

#[test]
fn aggregate_fingerprint_rejects_zero_radius() {
    let grid = RasterGrid::new(5, 5, 1.0, vec![10.0; 25]).unwrap();

    let fingerprint =
        star_backend::features::terrain_fingerprint::calculate_terrain_fingerprint(&grid, 2, 2, 0);

    assert!(fingerprint.is_none());
}

#[test]
fn dem_batch_processes_all_interior_cells() {
    use star_backend::preprocessing::dem_batch::extract_terrain_batch;

    let grid = RasterGrid::new(5, 5, 1.0, vec![100.0; 25]).unwrap();
    let report = extract_terrain_batch(&grid, 1).unwrap();

    assert_eq!(report.total_cells(), 25);
    assert_eq!(report.processed_cells(), 9);
    assert_eq!(report.skipped_cells, 16);
    assert!((report.coverage_fraction() - 0.36).abs() < 1e-10);
}

#[test]
fn dem_batch_calculates_mean_elevation() {
    use star_backend::preprocessing::dem_batch::extract_terrain_batch;

    let grid = RasterGrid::new(5, 5, 1.0, vec![42.0; 25]).unwrap();
    let report = extract_terrain_batch(&grid, 1).unwrap();

    assert_eq!(report.mean_elevation_m(), Some(42.0));
}

#[test]
fn dem_batch_rejects_zero_radius() {
    use star_backend::preprocessing::dem_batch::extract_terrain_batch;

    let grid = RasterGrid::new(5, 5, 1.0, vec![10.0; 25]).unwrap();

    assert!(extract_terrain_batch(&grid, 0).is_err());
}

#[test]
fn dem_batch_handles_raster_smaller_than_window() {
    use star_backend::preprocessing::dem_batch::extract_terrain_batch;

    let grid = RasterGrid::new(2, 2, 1.0, vec![10.0; 4]).unwrap();
    let report = extract_terrain_batch(&grid, 1).unwrap();

    assert_eq!(report.processed_cells(), 0);
    assert_eq!(report.skipped_cells, 4);
    assert_eq!(report.mean_elevation_m(), None);
}

#[test]
fn dem_batch_preserves_cell_coordinates() {
    use star_backend::preprocessing::dem_batch::extract_terrain_batch;

    let grid = RasterGrid::new(5, 5, 1.0, vec![7.0; 25]).unwrap();
    let report = extract_terrain_batch(&grid, 1).unwrap();

    assert!(
        report
            .samples
            .iter()
            .all(|sample| { sample.x >= 1 && sample.x < 4 && sample.y >= 1 && sample.y < 4 })
    );
}

#[test]
fn raster_mask_hides_nodata_cells() {
    let grid = RasterGrid::new_masked(
        3,
        3,
        1.0,
        vec![1.0, 2.0, 3.0, 4.0, 0.0, 6.0, 7.0, 8.0, 9.0],
        vec![true, true, true, true, false, true, true, true, true],
    )
    .unwrap();

    assert_eq!(grid.get(1, 1), None);
    assert_eq!(grid.get(0, 0), Some(1.0));
    assert_eq!(grid.valid_cells(), 8);
}

#[test]
fn raster_mask_rejects_wrong_length() {
    let result = RasterGrid::new_masked(3, 3, 1.0, vec![1.0; 9], vec![true; 8]);

    assert!(result.is_err());
}

#[test]
fn raster_mask_rejects_nonfinite_valid_values() {
    let result = RasterGrid::new_masked(2, 2, 1.0, vec![1.0, 2.0, f64::NAN, 4.0], vec![true; 4]);

    assert!(result.is_err());
}
