use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::features::multiscale::TileFeatures;

pub const SCHEMA_VERSION: &str = "star-temp-0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMeta {
    pub body: String,
    pub region: String,
    pub role: String,
    pub dem: String,
    pub epsg: u16,
    pub origin_x: f64,
    pub origin_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellRecord {
    pub id: String,
    pub body: String,
    pub region: String,
    pub role: String,
    pub dem: String,
    pub epsg: u16,

    pub center_x: f64,
    pub center_y: f64,

    pub tile_px: usize,
    pub cell_size_m: f64,
    pub quality_q: f64,

    pub slope_deg_1x: Option<f64>,
    pub slope_deg_3x: Option<f64>,
    pub slope_deg_9x: Option<f64>,

    pub rough_m_1x: Option<f64>,
    pub rough_m_3x: Option<f64>,
    pub rough_m_9x: Option<f64>,

    pub shadow_1deg: Option<f64>,
    pub shadow_2deg: Option<f64>,
    pub shadow_3deg: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellsFile {
    pub schema_version: String,
    pub cells: Vec<CellRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSummary {
    pub exported_cells: usize,
    pub output_path: PathBuf,
    pub schema_version: String,
}

fn pick(t: &TileFeatures, k: usize) -> (Option<f64>, Option<f64>) {
    t.scales
        .iter()
        .find(|s| s.scale == k)
        .map(|s| (s.rms_slope_deg, s.rms_roughness_m))
        .unwrap_or((None, None))
}

fn validate_optional_metric(value: Option<f64>, name: &str, index: usize) -> Result<(), String> {
    if let Some(v) = value {
        if !v.is_finite() {
            return Err(format!("Cell {index}: metric '{name}' is not finite"));
        }
    }

    Ok(())
}

pub fn validate_source_meta(meta: &SourceMeta) -> Result<(), String> {
    if meta.body.trim().is_empty() {
        return Err("Source body cannot be empty".into());
    }

    if meta.region.trim().is_empty() {
        return Err("Source region cannot be empty".into());
    }

    if meta.role.trim().is_empty() {
        return Err("Source role cannot be empty".into());
    }

    if meta.dem.trim().is_empty() {
        return Err("DEM name cannot be empty".into());
    }

    if !meta.origin_x.is_finite() || !meta.origin_y.is_finite() {
        return Err("Source origin coordinates must be finite".into());
    }

    if meta.epsg == 0 {
        return Err("EPSG code must be greater than zero".into());
    }

    Ok(())
}

fn validate_cell(cell: &CellRecord, index: usize) -> Result<(), String> {
    if cell.id.trim().is_empty() {
        return Err(format!("Cell {index}: empty ID"));
    }

    if cell.body.trim().is_empty()
        || cell.region.trim().is_empty()
        || cell.role.trim().is_empty()
        || cell.dem.trim().is_empty()
    {
        return Err(format!("Cell {index}: empty metadata field"));
    }

    if cell.epsg == 0 {
        return Err(format!("Cell {index}: invalid EPSG code"));
    }

    if !cell.center_x.is_finite() || !cell.center_y.is_finite() {
        return Err(format!("Cell {index}: invalid center coordinates"));
    }

    if cell.tile_px == 0 {
        return Err(format!("Cell {index}: tile size cannot be zero"));
    }

    if !cell.cell_size_m.is_finite() || cell.cell_size_m <= 0.0 {
        return Err(format!("Cell {index}: invalid pixel size"));
    }

    if !cell.quality_q.is_finite() || !(0.0..=1.0).contains(&cell.quality_q) {
        return Err(format!("Cell {index}: Q must be in range 0..=1"));
    }

    for (name, value) in [
        ("slope_deg_1x", cell.slope_deg_1x),
        ("slope_deg_3x", cell.slope_deg_3x),
        ("slope_deg_9x", cell.slope_deg_9x),
        ("rough_m_1x", cell.rough_m_1x),
        ("rough_m_3x", cell.rough_m_3x),
        ("rough_m_9x", cell.rough_m_9x),
        ("shadow_1deg", cell.shadow_1deg),
        ("shadow_2deg", cell.shadow_2deg),
        ("shadow_3deg", cell.shadow_3deg),
    ] {
        validate_optional_metric(value, name, index)?;
    }

    for (name, value) in [
        ("slope_deg_1x", cell.slope_deg_1x),
        ("slope_deg_3x", cell.slope_deg_3x),
        ("slope_deg_9x", cell.slope_deg_9x),
    ] {
        if let Some(v) = value {
            if !(0.0..=90.0).contains(&v) {
                return Err(format!(
                    "Cell {index}: '{name}' must be between 0 and 90 degrees"
                ));
            }
        }
    }

    for (name, value) in [
        ("shadow_1deg", cell.shadow_1deg),
        ("shadow_2deg", cell.shadow_2deg),
        ("shadow_3deg", cell.shadow_3deg),
    ] {
        if let Some(v) = value {
            if !(0.0..=1.0).contains(&v) {
                return Err(format!("Cell {index}: '{name}' must be between 0 and 1"));
            }
        }
    }

    Ok(())
}

pub fn validate_cells(cells: &[CellRecord]) -> Result<(), String> {
    let mut ids = HashSet::with_capacity(cells.len());

    for (index, cell) in cells.iter().enumerate() {
        validate_cell(cell, index)?;

        if !ids.insert(cell.id.as_str()) {
            return Err(format!("Duplicate cell ID '{}' at index {index}", cell.id));
        }
    }

    Ok(())
}

pub fn records_from_tiles(
    tiles: &[TileFeatures],
    meta: &SourceMeta,
) -> Result<Vec<CellRecord>, String> {
    validate_source_meta(meta)?;

    let mut records = Vec::with_capacity(tiles.len());

    for (index, t) in tiles.iter().enumerate() {
        if t.width == 0 || t.height == 0 {
            return Err(format!("Tile {index}: dimensions cannot be zero"));
        }

        if !t.cell_size_m.is_finite() || t.cell_size_m <= 0.0 {
            return Err(format!("Tile {index}: invalid cell size"));
        }

        if !t.quality_q.is_finite() || !(0.0..=1.0).contains(&t.quality_q) {
            return Err(format!("Tile {index}: invalid quality Q"));
        }

        let (s1, r1) = pick(t, 1);
        let (s3, r3) = pick(t, 3);
        let (s9, r9) = pick(t, 9);

        let cs = t.cell_size_m;

        let center_x = meta.origin_x + (t.x0 as f64 + t.width as f64 / 2.0) * cs;

        let center_y = meta.origin_y - (t.y0 as f64 + t.height as f64 / 2.0) * cs;

        let cell = CellRecord {
            id: format!("{}:{}:{}_{}", meta.body, meta.region, t.x0, t.y0),
            body: meta.body.clone(),
            region: meta.region.clone(),
            role: meta.role.clone(),
            dem: meta.dem.clone(),
            epsg: meta.epsg,
            center_x,
            center_y,
            tile_px: t.width.max(t.height),
            cell_size_m: cs,
            quality_q: t.quality_q,
            slope_deg_1x: s1,
            slope_deg_3x: s3,
            slope_deg_9x: s9,
            rough_m_1x: r1,
            rough_m_3x: r3,
            rough_m_9x: r9,
            shadow_1deg: None,
            shadow_2deg: None,
            shadow_3deg: None,
        };

        validate_cell(&cell, index)?;
        records.push(cell);
    }

    validate_cells(&records)?;
    Ok(records)
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();

    name.push(".tmp");

    path.with_file_name(name)
}

pub fn write_cells(path: &Path, cells: Vec<CellRecord>) -> Result<(), String> {
    validate_cells(&cells)?;

    let file = CellsFile {
        schema_version: SCHEMA_VERSION.to_string(),
        cells,
    };

    let json =
        serde_json::to_vec_pretty(&file).map_err(|e| format!("JSON serialization failed: {e}"))?;

    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir).map_err(|e| format!("Cannot create output directory: {e}"))?;
        }
    }

    let temp = temporary_path(path);

    fs::write(&temp, json).map_err(|e| format!("Cannot write temporary export: {e}"))?;

    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(format!(
            "Cannot move temporary export to destination: {error}"
        ));
    }

    Ok(())
}

pub fn export_cells(
    path: &Path,
    tiles: &[TileFeatures],
    meta: &SourceMeta,
) -> Result<ExportSummary, String> {
    let cells = records_from_tiles(tiles, meta)?;
    let count = cells.len();

    write_cells(path, cells)?;

    Ok(ExportSummary {
        exported_cells: count,
        output_path: path.to_path_buf(),
        schema_version: SCHEMA_VERSION.to_string(),
    })
}

pub fn read_cells(path: &Path) -> Result<CellsFile, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("Cannot read cells file: {e}"))?;

    let file: CellsFile =
        serde_json::from_str(&text).map_err(|e| format!("Invalid cells JSON: {e}"))?;

    if file.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "Unsupported schema version '{}'; expected '{}'",
            file.schema_version, SCHEMA_VERSION
        ));
    }

    validate_cells(&file.cells)?;

    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::multiscale::extract_all_tiles;
    use crate::preprocessing::dem::RasterGrid;

    fn test_meta() -> SourceMeta {
        SourceMeta {
            body: "earth".into(),
            region: "Test".into(),
            role: "candidate".into(),
            dem: "SRTM".into(),
            epsg: 32632,
            origin_x: 500_000.0,
            origin_y: 4_000_000.0,
        }
    }

    #[test]
    fn roundtrip_cells_json() {
        let grid = RasterGrid::new(40, 40, 30.0, vec![5.0; 1600]).unwrap();

        let tiles = extract_all_tiles(&grid, 20, 0.9);
        let records = records_from_tiles(&tiles, &test_meta()).unwrap();

        assert_eq!(records.len(), 4);
        assert_eq!(records[0].center_x, 500_000.0 + 10.0 * 30.0);
        assert_eq!(records[0].center_y, 4_000_000.0 - 10.0 * 30.0);

        let path = std::env::temp_dir().join("star_cells_test.json");

        write_cells(&path, records.clone()).unwrap();

        let back = read_cells(&path).unwrap();

        assert_eq!(back.schema_version, SCHEMA_VERSION);
        assert_eq!(back.cells.len(), 4);
        assert_eq!(back.cells[0].slope_deg_9x, records[0].slope_deg_9x);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_duplicate_ids() {
        let grid = RasterGrid::new(20, 20, 30.0, vec![5.0; 400]).unwrap();

        let tiles = extract_all_tiles(&grid, 20, 0.9);
        let mut records = records_from_tiles(&tiles, &test_meta()).unwrap();

        records.push(records[0].clone());

        assert!(validate_cells(&records).is_err());
    }

    #[test]
    fn rejects_invalid_quality() {
        let grid = RasterGrid::new(20, 20, 30.0, vec![5.0; 400]).unwrap();

        let tiles = extract_all_tiles(&grid, 20, 0.9);
        let mut records = records_from_tiles(&tiles, &test_meta()).unwrap();

        records[0].quality_q = 1.5;

        assert!(validate_cells(&records).is_err());
    }

    #[test]
    fn rejects_invalid_metadata() {
        let mut meta = test_meta();
        meta.region.clear();

        assert!(validate_source_meta(&meta).is_err());
    }
}
