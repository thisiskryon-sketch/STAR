use crate::preprocessing::dem::RasterGrid;

pub const SCALES: [usize; 3] = [1, 3, 9];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleFeatures {
    pub scale: usize,
    pub base_m: f64,
    pub rms_slope_deg: Option<f64>,
    pub rms_roughness_m: Option<f64>,
    pub slope_cells: usize,
    pub roughness_cells: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileFeatures {
    pub x0: usize,
    pub y0: usize,
    pub width: usize,
    pub height: usize,
    pub cell_size_m: f64,
    pub quality_q: f64,
    pub scales: Vec<ScaleFeatures>,
}

fn clip_tile(
    grid: &RasterGrid,
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
) -> Option<(usize, usize, usize, usize)> {
    if x0 >= grid.width || y0 >= grid.height || w == 0 || h == 0 {
        return None;
    }
    let w = w.min(grid.width - x0);
    let h = h.min(grid.height - y0);
    Some((x0, y0, w, h))
}

pub fn tile_quality(grid: &RasterGrid, x0: usize, y0: usize, w: usize, h: usize) -> f64 {
    let Some((x0, y0, w, h)) = clip_tile(grid, x0, y0, w, h) else {
        return 0.0;
    };
    let mut valid = 0usize;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            if grid.is_valid(x, y) {
                valid += 1;
            }
        }
    }
    valid as f64 / (w * h) as f64
}

pub fn scale_features(
    grid: &RasterGrid,
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
    k: usize,
) -> ScaleFeatures {
    let cs = grid.cell_size_m;
    let mut slope_sq_sum = 0.0;
    let mut slope_n = 0usize;
    let mut rough_sq_sum = 0.0;
    let mut rough_n = 0usize;

    let (x0, y0, w, h) = match clip_tile(grid, x0, y0, w, h) {
        Some(t) => t,
        None => {
            return ScaleFeatures {
                scale: k,
                base_m: k as f64 * cs,
                rms_slope_deg: None,
                rms_roughness_m: None,
                slope_cells: 0,
                roughness_cells: 0,
            };
        }
    };

    for y in y0..y0 + h {
        for x in x0..x0 + w {
            if x >= k && y >= k {
                if let (Some(l), Some(r), Some(t), Some(b)) = (
                    grid.get(x - k, y),
                    grid.get(x + k, y),
                    grid.get(x, y - k),
                    grid.get(x, y + k),
                ) {
                    let gx = (r - l) / (2.0 * k as f64 * cs);
                    let gy = (b - t) / (2.0 * k as f64 * cs);
                    slope_sq_sum += gx * gx + gy * gy;
                    slope_n += 1;
                }
            }

            if x >= k && y >= k && x + k < grid.width && y + k < grid.height {
                if let Some(center) = grid.get(x, y) {
                    let mut sum = 0.0;
                    let mut n = 0usize;
                    let mut complete = true;
                    'win: for wy in (y - k)..=(y + k) {
                        for wx in (x - k)..=(x + k) {
                            match grid.get(wx, wy) {
                                Some(v) => {
                                    sum += v;
                                    n += 1;
                                }
                                None => {
                                    complete = false;
                                    break 'win;
                                }
                            }
                        }
                    }
                    if complete {
                        let d = center - sum / n as f64;
                        rough_sq_sum += d * d;
                        rough_n += 1;
                    }
                }
            }
        }
    }

    ScaleFeatures {
        scale: k,
        base_m: k as f64 * cs,
        rms_slope_deg: (slope_n > 0)
            .then(|| (slope_sq_sum / slope_n as f64).sqrt().atan().to_degrees()),
        rms_roughness_m: (rough_n > 0).then(|| (rough_sq_sum / rough_n as f64).sqrt()),
        slope_cells: slope_n,
        roughness_cells: rough_n,
    }
}

pub fn extract_tile_features(
    grid: &RasterGrid,
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
) -> Option<TileFeatures> {
    let (x0, y0, w, h) = clip_tile(grid, x0, y0, w, h)?;
    Some(TileFeatures {
        x0,
        y0,
        width: w,
        height: h,
        cell_size_m: grid.cell_size_m,
        quality_q: tile_quality(grid, x0, y0, w, h),
        scales: SCALES
            .iter()
            .map(|&k| scale_features(grid, x0, y0, w, h, k))
            .collect(),
    })
}

pub fn tile_heights(
    grid: &RasterGrid,
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
) -> Option<(usize, usize, Vec<f64>)> {
    let (x0, y0, w, h) = clip_tile(grid, x0, y0, w, h)?;
    let mut out = Vec::with_capacity(w * h);
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            out.push(grid.get(x, y).unwrap_or(f64::NAN));
        }
    }
    Some((w, h, out))
}

pub fn extract_all_tiles(grid: &RasterGrid, tile_px: usize, q_min: f64) -> Vec<TileFeatures> {
    let mut out = Vec::new();
    if tile_px == 0 {
        return out;
    }
    let mut y0 = 0;
    while y0 < grid.height {
        let mut x0 = 0;
        while x0 < grid.width {
            let q = tile_quality(grid, x0, y0, tile_px, tile_px);
            if q >= q_min {
                if let Some(t) = extract_tile_features(grid, x0, y0, tile_px, tile_px) {
                    out.push(t);
                }
            }
            x0 += tile_px;
        }
        y0 += tile_px;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(w: usize, h: usize, cs: f64, tan: f64) -> RasterGrid {
        let mut z = Vec::new();
        for _y in 0..h {
            for x in 0..w {
                z.push(tan * x as f64 * cs);
            }
        }
        RasterGrid::new(w, h, cs, z).unwrap()
    }

    #[test]
    fn plane_slope_is_same_at_all_scales() {
        let g = plane(60, 60, 30.0, 0.2);
        let t = extract_tile_features(&g, 0, 0, 60, 60).unwrap();
        let expected = 0.2f64.atan().to_degrees();
        for s in &t.scales {
            let got = s.rms_slope_deg.unwrap();
            assert!((got - expected).abs() < 1e-9, "k={} got {}", s.scale, got);
        }
        assert_eq!(t.quality_q, 1.0);
    }

    #[test]
    fn plane_has_zero_roughness() {
        let g = plane(60, 60, 30.0, 0.2);
        let t = extract_tile_features(&g, 0, 0, 60, 60).unwrap();
        for s in &t.scales {
            assert!(s.rms_roughness_m.unwrap() < 1e-9);
        }
    }

    #[test]
    fn flat_surface_is_zero() {
        let g = RasterGrid::new(40, 40, 90.0, vec![100.0; 1600]).unwrap();
        let t = extract_tile_features(&g, 0, 0, 40, 40).unwrap();
        for s in &t.scales {
            assert_eq!(s.rms_slope_deg.unwrap(), 0.0);
            assert_eq!(s.rms_roughness_m.unwrap(), 0.0);
        }
    }

    #[test]
    fn checkerboard_roughness_drops_with_scale_correctly() {
        let mut z = Vec::new();
        for y in 0..30 {
            for x in 0..30 {
                z.push(if (x + y) % 2 == 0 { 1.0 } else { -1.0 });
            }
        }
        let g = RasterGrid::new(30, 30, 30.0, z).unwrap();
        let t = extract_tile_features(&g, 0, 0, 30, 30).unwrap();
        assert!(t.scales[0].rms_roughness_m.unwrap() > 0.5);
    }

    #[test]
    fn nodata_lowers_q_and_is_not_used() {
        let w = 20;
        let mut z = vec![0.0; w * w];
        let mut mask = vec![true; w * w];
        for i in 0..(w * w / 4) {
            mask[i] = false;
            z[i] = f64::NAN;
        }
        let g = RasterGrid::new_masked(w, w, 30.0, z, mask).unwrap();
        let t = extract_tile_features(&g, 0, 0, w, w).unwrap();
        assert!((t.quality_q - 0.75).abs() < 1e-12);
        assert_eq!(t.scales[0].rms_slope_deg.unwrap(), 0.0);
    }

    #[test]
    fn low_q_tiles_are_dropped_before_features() {
        let w = 20;
        let mask: Vec<bool> = (0..w * w).map(|i| i % 2 == 0).collect();
        let z = vec![0.0; w * w];
        let g = RasterGrid::new_masked(w, w, 30.0, z, mask).unwrap();
        assert!(extract_all_tiles(&g, 10, 0.9).is_empty());
        assert!(!extract_all_tiles(&g, 10, 0.4).is_empty());
    }

    #[test]
    fn tile_heights_marks_invalid_as_nan() {
        let mask = vec![true, false, true, true];
        let g = RasterGrid::new_masked(2, 2, 30.0, vec![1.0, 2.0, 3.0, 4.0], mask).unwrap();
        let (w, h, v) = tile_heights(&g, 0, 0, 2, 2).unwrap();
        assert_eq!((w, h), (2, 2));
        assert!(v[1].is_nan() && v[0] == 1.0 && v[3] == 4.0);
    }
}
