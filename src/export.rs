//! Временный формат выгрузки cells.json / targets.json.
//! Когда появится контракт star-core, менять нужно только структуры ниже.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::features::multiscale::TileFeatures;

pub const SCHEMA_VERSION: &str = "star-temp-0";

/// Метаданные источника, общие для всех тайлов одного DEM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMeta {
    /// "earth" или "moon"
    pub body: String,
    /// Регион/район: "Atacama", "Haworth", ...
    pub region: String,
    /// "known_analog", "negative_control", "candidate" (Земля) или "target" (Луна)
    pub role: String,
    /// Имя DEM: "SRTM", "ASTER GDEM", "COP30", "LOLA"
    pub dem: String,
    pub epsg: u16,
    /// Проективные координаты левого верхнего угла растра, м.
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
    /// Центр тайла в проективных координатах, м.
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
    /// Доля тени при высоте Солнца 1°, 2°, 3° (заполняет модуль теней).
    pub shadow_1deg: Option<f64>,
    pub shadow_2deg: Option<f64>,
    pub shadow_3deg: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellsFile {
    pub schema_version: String,
    pub cells: Vec<CellRecord>,
}

fn pick(t: &TileFeatures, k: usize) -> (Option<f64>, Option<f64>) {
    t.scales
        .iter()
        .find(|s| s.scale == k)
        .map(|s| (s.rms_slope_deg, s.rms_roughness_m))
        .unwrap_or((None, None))
}

/// Превращает тайлы одного растра в записи cells.json.
/// Тени остаются None, пока их не посчитает модуль теней.
pub fn records_from_tiles(tiles: &[TileFeatures], meta: &SourceMeta) -> Vec<CellRecord> {
    tiles
        .iter()
        .map(|t| {
            let (s1, r1) = pick(t, 1);
            let (s3, r3) = pick(t, 3);
            let (s9, r9) = pick(t, 9);
            let cs = t.cell_size_m;
            CellRecord {
                id: format!("{}:{}:{}_{}", meta.body, meta.region, t.x0, t.y0),
                body: meta.body.clone(),
                region: meta.region.clone(),
                role: meta.role.clone(),
                dem: meta.dem.clone(),
                epsg: meta.epsg,
                center_x: meta.origin_x + (t.x0 as f64 + t.width as f64 / 2.0) * cs,
                center_y: meta.origin_y - (t.y0 as f64 + t.height as f64 / 2.0) * cs,
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
            }
        })
        .collect()
}

pub fn write_cells(path: &Path, cells: Vec<CellRecord>) -> Result<(), String> {
    let file = CellsFile {
        schema_version: SCHEMA_VERSION.to_string(),
        cells,
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, json).map_err(|e| e.to_string())
}

pub fn read_cells(path: &Path) -> Result<CellsFile, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::multiscale::extract_all_tiles;
    use crate::preprocessing::dem::RasterGrid;

    #[test]
    fn roundtrip_cells_json() {
        let g = RasterGrid::new(40, 40, 30.0, vec![5.0; 1600]).unwrap();
        let tiles = extract_all_tiles(&g, 20, 0.9);
        let meta = SourceMeta {
            body: "earth".into(),
            region: "Test".into(),
            role: "candidate".into(),
            dem: "SRTM".into(),
            epsg: 32632,
            origin_x: 500000.0,
            origin_y: 4000000.0,
        };
        let recs = records_from_tiles(&tiles, &meta);
        assert_eq!(recs.len(), 4);
        assert_eq!(recs[0].center_x, 500000.0 + 10.0 * 30.0);
        assert_eq!(recs[0].center_y, 4000000.0 - 10.0 * 30.0);
        let p = std::env::temp_dir().join("star_cells_test.json");
        write_cells(&p, recs.clone()).unwrap();
        let back = read_cells(&p).unwrap();
        assert_eq!(back.schema_version, SCHEMA_VERSION);
        assert_eq!(back.cells.len(), 4);
        assert_eq!(back.cells[0].slope_deg_9x, recs[0].slope_deg_9x);
    }
}
