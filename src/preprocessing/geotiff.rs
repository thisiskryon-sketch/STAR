use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use tiff::decoder::{Decoder, DecodingResult};
use tiff::tags::Tag;

use crate::preprocessing::dem::RasterGrid;

#[derive(Debug, Clone)]
pub struct GeoTiffDem {
    pub grid: RasterGrid,
    pub path: PathBuf,
    pub epsg: u16,
    pub crs_label: String,
    pub origin_x: f64,
    pub origin_y: f64,
    pub pixel_size_m: f64,
    pub nodata: Option<f64>,
}

fn parse_geokeys(keys: &[u16]) -> Result<(u16, f64), String> {
    if keys.len() < 4 {
        return Err("Invalid GeoKeyDirectory".to_string());
    }

    let count = keys[3] as usize;

    if keys.len() < 4 + count * 4 {
        return Err("Incomplete GeoKeyDirectory".to_string());
    }

    let mut projected_epsg = None;
    let mut linear_units = None;

    for entry in keys[4..4 + count * 4].chunks_exact(4) {
        let key_id = entry[0];
        let location = entry[1];
        let count = entry[2];
        let value = entry[3];

        if location != 0 || count != 1 {
            continue;
        }

        match key_id {
            3072 => projected_epsg = Some(value),
            3076 => linear_units = Some(value),
            _ => {}
        }
    }

    let epsg = projected_epsg
        .ok_or_else(|| {
            "Projected CRS is required. Geographic coordinates in degrees are not yet supported for metric terrain analysis.".to_string()
        })?;

    let unit_factor = match linear_units {
        Some(9001) => 1.0,
        Some(9002) => 0.3048,
        _ => {
            return Err(
                "Unsupported or missing projected linear units. Expected metres or international feet.".to_string()
            );
        }
    };

    Ok((epsg, unit_factor))
}

fn decode_elevations(result: DecodingResult) -> Result<Vec<f64>, String> {
    let values = match result {
        DecodingResult::U8(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::U16(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::U32(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::U64(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::I8(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::I16(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::I32(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::I64(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::F16(v) => v.into_iter().map(|x| x.to_f32() as f64).collect(),
        DecodingResult::F32(v) => v.into_iter().map(|x| x as f64).collect(),
        DecodingResult::F64(v) => v,
    };

    Ok(values)
}

pub fn load_geotiff(path: impl AsRef<Path>) -> Result<GeoTiffDem, String> {
    let path = path.as_ref();

    let file = File::open(path).map_err(|e| format!("Cannot open file: {e}"))?;

    let mut decoder =
        Decoder::new(BufReader::new(file)).map_err(|e| format!("Invalid TIFF: {e}"))?;

    let (width, height) = decoder
        .dimensions()
        .map_err(|e| format!("Cannot read raster dimensions: {e}"))?;

    let width = width as usize;
    let height = height as usize;

    let total = width
        .checked_mul(height)
        .ok_or_else(|| "Raster dimensions overflow".to_string())?;

    if total > 100_000_000 {
        return Err("Raster exceeds the 100-million-cell safety limit".to_string());
    }

    match decoder
        .colortype()
        .map_err(|e| format!("Cannot read TIFF color type: {e}"))?
    {
        tiff::ColorType::Gray(_) => {}
        other => {
            return Err(format!(
                "Expected a single-band grayscale DEM, got {other:?}"
            ));
        }
    }

    let scale = decoder
        .get_tag_f64_vec(Tag::ModelPixelScaleTag)
        .map_err(|e| format!("Missing or invalid ModelPixelScaleTag: {e}"))?;

    let tiepoint = decoder
        .get_tag_f64_vec(Tag::ModelTiepointTag)
        .map_err(|e| format!("Missing or invalid ModelTiepointTag: {e}"))?;

    let geokeys = decoder
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .map_err(|e| format!("Missing or invalid GeoKeyDirectoryTag: {e}"))?;

    if scale.len() < 2 || tiepoint.len() < 6 {
        return Err("Incomplete GeoTIFF georeferencing metadata".to_string());
    }

    if scale.iter().take(2).any(|v| !v.is_finite() || *v <= 0.0) {
        return Err("Invalid pixel scale".to_string());
    }

    // This first implementation requires square, north-up pixels.
    let ratio = scale[0] / scale[1];

    if (ratio - 1.0).abs() > 0.01 {
        return Err(
            "Non-square pixels are not supported yet. Resample the DEM to square cells first."
                .to_string(),
        );
    }

    let (epsg, unit_factor) = parse_geokeys(&geokeys)?;

    let pixel_size_x = scale[0] * unit_factor;
    let pixel_size_y = scale[1] * unit_factor;
    let pixel_size_m = (pixel_size_x + pixel_size_y) / 2.0;

    // Pixel-to-model origin from the first tiepoint.
    let origin_x = tiepoint[3] - tiepoint[0] * scale[0];
    let origin_y = tiepoint[4] + tiepoint[1] * scale[1];

    if !origin_x.is_finite() || !origin_y.is_finite() {
        return Err("Invalid geospatial origin".to_string());
    }

    let nodata = decoder
        .get_tag_ascii_string(Tag::GdalNodata)
        .ok()
        .and_then(|s| s.trim_matches('\0').trim().parse::<f64>().ok());

    let raw = decoder
        .read_image()
        .map_err(|e| format!("Cannot decode raster pixels: {e}"))?;

    let mut elevation = decode_elevations(raw)?;

    if elevation.len() != total {
        return Err(format!(
            "Unexpected pixel count: expected {total}, got {}",
            elevation.len()
        ));
    }

    let valid_mask: Vec<bool> = elevation
        .iter()
        .map(|value| {
            if !value.is_finite() {
                return false;
            }

            match nodata {
                Some(n) if n.is_nan() => !value.is_nan(),
                Some(n) => *value != n,
                None => true,
            }
        })
        .collect();

    let grid = RasterGrid::new_masked(
        width,
        height,
        pixel_size_m,
        std::mem::take(&mut elevation),
        valid_mask,
    )?;

    if grid.valid_cells() == 0 {
        return Err("Raster contains no valid elevation cells".to_string());
    }

    let crs_label = if epsg == 32767 {
        "User-defined projected CRS".to_string()
    } else {
        format!("EPSG:{epsg}")
    };

    Ok(GeoTiffDem {
        grid,
        path: path.to_path_buf(),
        epsg,
        crs_label,
        origin_x,
        origin_y,
        pixel_size_m,
        nodata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_projected_metre_crs() {
        let keys = vec![1, 1, 0, 2, 3072, 0, 1, 32636, 3076, 0, 1, 9001];

        let (epsg, factor) = parse_geokeys(&keys).unwrap();

        assert_eq!(epsg, 32636);
        assert_eq!(factor, 1.0);
    }

    #[test]
    fn parses_projected_feet_crs() {
        let keys = vec![1, 1, 0, 2, 3072, 0, 1, 2277, 3076, 0, 1, 9002];

        let (_, factor) = parse_geokeys(&keys).unwrap();

        assert!((factor - 0.3048).abs() < 1e-10);
    }

    #[test]
    fn rejects_geographic_crs() {
        let keys = vec![1, 1, 0, 1, 2048, 0, 1, 4326];

        assert!(parse_geokeys(&keys).is_err());
    }

    #[test]
    fn rejects_incomplete_geokeys() {
        assert!(parse_geokeys(&[1, 1, 0, 2, 3072]).is_err());
    }
}
