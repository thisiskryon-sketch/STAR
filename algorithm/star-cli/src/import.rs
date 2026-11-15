// star-cli/src/import.rs
//! Adapter: teammate export schema `star-temp-0` -> S.T.A.R. `cells.json` / `targets.json`.
//!
//! Driven by an import manifest that records provenance for every source file:
//! {
//!   "sources": [
//!     { "file": "kamchatka_nasadem.json", "vertical_uncertainty_m": 3.0,
//!       "source_version": "NASADEM_HGT v001", "processing_version": "star_backend 8623762",
//!       "data_origin": "ExternalMeasured" }
//!   ]
//! }
//! Earth records become cells (lat/lon taken from the record, else inverse UTM for
//! EPSG 326xx/327xx). Moon/Mars records are aggregated per region into one TargetProfile
//! (median of each feature). Records missing any feature are skipped and reported,
//! never filled with 0. Existing output files are not overwritten unless --force.
use serde::{Deserialize, Serialize};
use star_core::assessment::{SiteWindow, TemporalAnalogWindow};
use star_core::novelty::haversine_km;
use star_core::schema::{
    CelestialBody, Cell, DataOrigin, DemFeatures, FeatureId, KnownAnalogSite, ProvenanceMetadata, TargetProfile,
    TestProfile,
};
use crate::synth::KnownAnalogLabels;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct StarTempFile {
    pub schema_version: String,
    pub cells: Vec<StarTempCell>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StarTempCell {
    pub id: String,
    pub body: String,
    pub region: String,
    pub role: String,
    pub dem: String,
    pub epsg: u32,
    pub center_x: f64,
    pub center_y: f64,
    #[allow(dead_code)]
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
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lon: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportSource {
    pub file: String,
    /// Relative (point-to-point) vertical accuracy of the DEM, 1-sigma, metres, from the product documentation.
    pub vertical_uncertainty_m: f64,
    pub source_version: String,
    pub processing_version: String,
    #[serde(default = "default_origin")]
    pub data_origin: DataOrigin,
}

fn default_origin() -> DataOrigin {
    DataOrigin::ExternalMeasured
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportManifest {
    pub sources: Vec<ImportSource>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkippedRecord {
    pub file: String,
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetSpread {
    pub target_id: String,
    pub cells_aggregated: usize,
    /// Interquartile range per feature across the aggregated lunar/martian cells.
    pub iqr: BTreeMap<FeatureId, f64>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ImportReport {
    pub earth_cells_written: usize,
    pub targets_written: usize,
    pub labels_written: bool,
    pub catalog_sites_written: usize,
    pub skipped: Vec<SkippedRecord>,
    pub target_spread: Vec<TargetSpread>,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// UTM (WGS84) <-> geographic, Krüger series (accurate to ~mm inside a zone)
// ---------------------------------------------------------------------------

const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_223_563;
const K0: f64 = 0.9996;

fn utm_consts() -> (f64, [f64; 3], [f64; 3]) {
    let n = F / (2.0 - F);
    let a_hat = A / (1.0 + n) * (1.0 + n * n / 4.0 + n.powi(4) / 64.0);
    let alpha = [n / 2.0 - 2.0 * n * n / 3.0 + 5.0 * n.powi(3) / 16.0, 13.0 * n * n / 48.0 - 3.0 * n.powi(3) / 5.0, 61.0 * n.powi(3) / 240.0];
    let beta = [n / 2.0 - 2.0 * n * n / 3.0 + 37.0 * n.powi(3) / 96.0, n * n / 48.0 + n.powi(3) / 15.0, 17.0 * n.powi(3) / 480.0];
    (a_hat, alpha, beta)
}

/// EPSG 32601-32660 (north) / 32701-32760 (south) -> (zone, north?)
pub fn utm_zone_from_epsg(epsg: u32) -> Option<(u32, bool)> {
    match epsg {
        32601..=32660 => Some((epsg - 32600, true)),
        32701..=32760 => Some((epsg - 32700, false)),
        _ => None,
    }
}

pub fn utm_to_latlon(easting: f64, northing: f64, zone: u32, north: bool) -> (f64, f64) {
    let (a_hat, _, beta) = utm_consts();
    let e2 = F * (2.0 - F);
    let e = e2.sqrt();
    let xi = (northing - if north { 0.0 } else { 10_000_000.0 }) / (K0 * a_hat);
    let eta = (easting - 500_000.0) / (K0 * a_hat);
    let mut xi_p = xi;
    let mut eta_p = eta;
    for (j, b) in beta.iter().enumerate() {
        let k = 2.0 * (j as f64 + 1.0);
        xi_p -= b * (k * xi).sin() * (k * eta).cosh();
        eta_p -= b * (k * xi).cos() * (k * eta).sinh();
    }
    let chi = (xi_p.sin() / eta_p.cosh()).asin();
    // Robust fixed-point on tan(phi) (Karney 2011, eq. 19-21).
    let tau_p = chi.tan();
    let mut tau = tau_p;
    for _ in 0..8 {
        let sigma = (e * (e * tau / (1.0 + tau * tau).sqrt()).atanh()).sinh();
        let tau_p_i = tau * (1.0 + sigma * sigma).sqrt() - sigma * (1.0 + tau * tau).sqrt();
        let dtau = (tau_p - tau_p_i) / (1.0 + tau_p_i * tau_p_i).sqrt() * (1.0 + (1.0 - e2) * tau * tau) / ((1.0 - e2) * (1.0 + tau * tau).sqrt());
        tau += dtau;
    }
    let lat = tau.atan().to_degrees();
    let lon0 = (zone as f64 - 1.0) * 6.0 - 180.0 + 3.0;
    let lon = lon0 + (eta_p.sinh() / xi_p.cos()).atan().to_degrees();
    (lat, lon)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn latlon_to_utm(lat: f64, lon: f64, zone: u32) -> (f64, f64) {
    let (a_hat, alpha, _) = utm_consts();
    let e = (F * (2.0 - F)).sqrt();
    let phi = lat.to_radians();
    let lam = (lon - ((zone as f64 - 1.0) * 6.0 - 180.0 + 3.0)).to_radians();
    let t = (phi.sin().atanh() - e * (e * phi.sin()).atanh()).sinh();
    let xi_p = t.atan2(lam.cos());
    let eta_p = (lam.sin() / (1.0 + t * t).sqrt()).atanh();
    let mut xi = xi_p;
    let mut eta = eta_p;
    for (j, a) in alpha.iter().enumerate() {
        let k = 2.0 * (j as f64 + 1.0);
        xi += a * (k * xi_p).sin() * (k * eta_p).cosh();
        eta += a * (k * xi_p).cos() * (k * eta_p).sinh();
    }
    let easting = 500_000.0 + K0 * a_hat * eta;
    let northing = K0 * a_hat * xi + if lat < 0.0 { 10_000_000.0 } else { 0.0 };
    (easting, northing)
}

// ---------------------------------------------------------------------------
// Low-sun windows from solar geometry (replaces hand-typed windows)
// ---------------------------------------------------------------------------

/// Solar declination (deg) for day-of-year, Spencer (1971) Fourier series (~0.03° accuracy).
pub fn solar_declination_deg(day_of_year: u32, days_in_year: u32) -> f64 {
    let g = 2.0 * std::f64::consts::PI * (day_of_year as f64 - 1.0) / days_in_year as f64;
    (0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2.0 * g).cos() + 0.000907 * (2.0 * g).sin()
        - 0.002697 * (3.0 * g).cos()
        + 0.00148 * (3.0 * g).sin())
    .to_degrees()
}

/// Geometric noon sun elevation (deg), no refraction: 90 - |lat - decl|.
pub fn noon_sun_elevation_deg(lat: f64, decl: f64) -> f64 {
    90.0 - (lat - decl).abs()
}

fn ymd(year: i32, doy: u32) -> String {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let months = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut d = doy;
    let mut m = 0;
    while d > months[m] {
        d -= months[m];
        m += 1;
    }
    format!("{year:04}-{:02}-{d:02}T00:00:00Z", m + 1)
}

/// Days on which the sun rises but its NOON elevation stays within (min_elev, max_elev]:
/// the sun skims the horizon all day, the closest Earth analog to lunar polar illumination
/// (where the sun stays within about ±1.5° of the horizon). Consecutive days form one window.
pub fn lowsun_windows(region: &str, mechanism_id: &str, lat: f64, year: i32, min_elev: f64, max_elev: f64) -> Vec<SiteWindow> {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let n = if leap { 366 } else { 365 };
    let ok: Vec<bool> = (1..=n)
        .map(|d| {
            let e = noon_sun_elevation_deg(lat, solar_declination_deg(d, n));
            e > min_elev && e <= max_elev
        })
        .collect();
    let mut out = Vec::new();
    let mut d = 0usize;
    while d < ok.len() {
        if ok[d] {
            let start = d;
            while d < ok.len() && ok[d] {
                d += 1;
            }
            out.push(SiteWindow {
                region: region.to_string(),
                mechanism_id: mechanism_id.to_string(),
                window: TemporalAnalogWindow {
                    window_id: format!("{}_{}_{}", sanitize(region), year, out.len() + 1),
                    start_utc: ymd(year, start as u32 + 1),
                    end_utc: ymd(year, d as u32 + 1),
                    season_or_solar_regime: format!(
                        "Noon sun elevation in ({min_elev}°, {max_elev}°] at lat {lat:.2}° (Spencer declination, no refraction, ±0.5°)"
                    ),
                    illumination_geometry_valid: true,
                },
            });
        } else {
            d += 1;
        }
    }
    out
}

fn features_of(r: &StarTempCell) -> Result<DemFeatures, String> {
    let vals = [
        ("slope_deg_1x", r.slope_deg_1x),
        ("slope_deg_3x", r.slope_deg_3x),
        ("slope_deg_9x", r.slope_deg_9x),
        ("rough_m_1x", r.rough_m_1x),
        ("rough_m_3x", r.rough_m_3x),
        ("rough_m_9x", r.rough_m_9x),
        ("shadow_1deg", r.shadow_1deg),
        ("shadow_2deg", r.shadow_2deg),
        ("shadow_3deg", r.shadow_3deg),
    ];
    let missing: Vec<&str> = vals.iter().filter(|(_, v)| v.is_none()).map(|(n, _)| *n).collect();
    if !missing.is_empty() {
        return Err(format!("missing features (not filled with 0): {}", missing.join(", ")));
    }
    let v = |i: usize| vals[i].1.unwrap();
    let f = DemFeatures {
        rms_slope_1x: v(0),
        rms_slope_3x: v(1),
        rms_slope_9x: v(2),
        roughness_1x: v(3),
        roughness_3x: v(4),
        roughness_9x: v(5),
        shadow_obstruction_1deg: v(6),
        shadow_obstruction_2deg: v(7),
        shadow_obstruction_3deg: v(8),
    };
    f.validate().map_err(|e| e.to_string())?;
    Ok(f)
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { 0.5 * (v[n / 2 - 1] + v[n / 2]) }
}

fn quantile(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    s[((s.len() - 1) as f64 * q).round() as usize]
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect()
}

pub fn run_import(manifest_path: &Path, out_dir: &Path, force: bool) -> Result<ImportReport, Box<dyn std::error::Error>> {
    let base = manifest_path.parent().unwrap_or(Path::new("."));
    let manifest: ImportManifest = serde_json::from_slice(&fs::read(manifest_path)?)?;
    for name in ["cells.json", "targets.json"] {
        if out_dir.join(name).exists() && !force {
            return Err(format!("{} exists; refusing to overwrite input data (use --force)", out_dir.join(name).display()).into());
        }
    }
    let mut report = ImportReport::default();
    let mut cells: Vec<Cell> = Vec::new();
    let mut seen = BTreeSet::new();
    let (mut positive, mut negative) = (BTreeSet::new(), BTreeSet::new());
    // (body, region) -> (features, resolution set, vertical uncertainty, provenance)
    let mut planetary: BTreeMap<(String, String), (Vec<DemFeatures>, BTreeSet<u64>, f64, String)> = BTreeMap::new();

    for src in &manifest.sources {
        if !(src.vertical_uncertainty_m.is_finite() && src.vertical_uncertainty_m >= 0.0) {
            return Err(format!("{}: invalid vertical_uncertainty_m", src.file).into());
        }
        let file: StarTempFile = serde_json::from_slice(&fs::read(base.join(&src.file))?)
            .map_err(|e| format!("{}: {e}", src.file))?;
        if file.schema_version != "star-temp-0" {
            return Err(format!("{}: unsupported schema '{}'", src.file, file.schema_version).into());
        }
        for r in &file.cells {
            let skip = |reason: String, rep: &mut ImportReport| rep.skipped.push(SkippedRecord { file: src.file.clone(), id: r.id.clone(), reason });
            let feats = match features_of(r) {
                Ok(f) => f,
                Err(e) => {
                    skip(e, &mut report);
                    continue;
                }
            };
            let mut origin = src.data_origin;
            if r.dem.to_lowercase().contains("synth") && origin != DataOrigin::Synthetic {
                origin = DataOrigin::Synthetic;
                let w = format!("{}: DEM '{}' looks synthetic; its records are marked Synthetic", src.file, r.dem);
                if !report.warnings.contains(&w) {
                    report.warnings.push(w);
                }
            }
            match r.body.to_lowercase().as_str() {
                "earth" => {
                    let (lat, lon) = match (r.lat, r.lon, utm_zone_from_epsg(r.epsg)) {
                        (Some(a), Some(b), _) => (a, b),
                        (_, _, Some((zone, north))) => utm_to_latlon(r.center_x, r.center_y, zone, north),
                        _ => {
                            skip(format!("EPSG {} is not UTM/WGS84 and the record has no lat/lon", r.epsg), &mut report);
                            continue;
                        }
                    };
                    match r.role.to_lowercase().as_str() {
                        "known_analog" => {
                            positive.insert(r.region.clone());
                        }
                        "negative_control" => {
                            negative.insert(r.region.clone());
                        }
                        _ => {}
                    }
                    if !seen.insert(r.id.clone()) {
                        return Err(format!("duplicate cell id '{}' across sources; make ids unique per region", r.id).into());
                    }
                    cells.push(Cell {
                        id: r.id.clone(),
                        region: r.region.clone(),
                        lat,
                        lon,
                        resolution_m: r.cell_size_m,
                        valid_pixel_ratio: r.quality_q,
                        vertical_uncertainty_m: src.vertical_uncertainty_m,
                        features: feats,
                        provenance: Some(ProvenanceMetadata {
                            source_id: r.dem.clone(),
                            source_version: src.source_version.clone(),
                            processing_version: src.processing_version.clone(),
                            observation_window_utc: None,
                        }),
                        feature_uncertainty: BTreeMap::new(),
                        data_origin: origin,
                    });
                }
                "moon" | "mars" => {
                    let e = planetary
                        .entry((r.body.to_lowercase(), r.region.clone()))
                        .or_insert_with(|| (Vec::new(), BTreeSet::new(), src.vertical_uncertainty_m, format!("{} {} / {}", r.dem, src.source_version, src.processing_version)));
                    e.0.push(feats);
                    e.1.insert(r.cell_size_m.to_bits());
                }
                other => skip(format!("unknown body '{other}'"), &mut report),
            }
        }
    }

    let mut targets: Vec<TargetProfile> = Vec::new();
    for ((body, region), (feats, res, unc, prov)) in planetary {
        if res.len() != 1 {
            return Err(format!("{body}/{region}: cells with different pixel sizes; resample to one resolution first").into());
        }
        let resolution_m = f64::from_bits(*res.iter().next().unwrap());
        let id = format!("{}_{}", sanitize(&body), sanitize(&region));
        let mut agg = feats[0].clone();
        let mut iqr = BTreeMap::new();
        for f in FeatureId::ALL {
            let v: Vec<f64> = feats.iter().map(|x| x.get(f)).collect();
            *agg.get_mut(f) = median(v.clone());
            iqr.insert(f, quantile(&v, 0.75) - quantile(&v, 0.25));
        }
        report.target_spread.push(TargetSpread { target_id: id.clone(), cells_aggregated: feats.len(), iqr });
        report.warnings.push(format!("{id}: target = median of {} cells from {prov}", feats.len()));
        targets.push(TargetProfile {
            id,
            body: if body == "moon" { CelestialBody::Moon } else { CelestialBody::Mars },
            region_name: region,
            test_profile: TestProfile::P3CombinedTerrainShadow,
            resolution_m,
            vertical_uncertainty_m: unc,
            features: agg,
        });
    }

    star_core::schema::validate_cells(&cells).map_err(|e| e.to_string())?;
    let resolutions: BTreeSet<u64> = cells.iter().map(|c| c.resolution_m.to_bits()).chain(targets.iter().map(|t| t.resolution_m.to_bits())).collect();
    if resolutions.len() > 1 {
        report.warnings.push("Earth and planetary cells have different pixel sizes: multi-scale features (1x/3x/9x) are only comparable at a common resolution; QC will reject ratios above max_resolution_ratio.".into());
    }
    fs::create_dir_all(out_dir)?;
    fs::write(out_dir.join("cells.json"), serde_json::to_string_pretty(&cells)?)?;
    fs::write(out_dir.join("targets.json"), serde_json::to_string_pretty(&targets)?)?;
    fs::create_dir_all(out_dir)?;
    if !positive.is_empty() || !negative.is_empty() {
        if out_dir.join("labels.json").exists() && !force {
            return Err("labels.json exists; refusing to overwrite (use --force)".into());
        }
        let labels = KnownAnalogLabels {
            target_id: "ALL".into(),
            positive_regions: positive.into_iter().collect(),
            negative_control_regions: negative.into_iter().collect(),
        };
        fs::write(out_dir.join("labels.json"), serde_json::to_string_pretty(&labels)?)?;
        report.labels_written = true;
        // Known-analog catalog: one entry per known-analog region (median features, centroid,
        // radius = farthest cell from the centroid). Built from measured cells, not typed in.
        let labels_pos: BTreeSet<String> = labels.positive_regions.iter().cloned().collect();
        let mut catalog: Vec<KnownAnalogSite> = Vec::new();
        for region in &labels_pos {
            let rc: Vec<&Cell> = cells.iter().filter(|c| &c.region == region).collect();
            if rc.is_empty() {
                continue;
            }
            let lat = rc.iter().map(|c| c.lat).sum::<f64>() / rc.len() as f64;
            let lon = rc.iter().map(|c| c.lon).sum::<f64>() / rc.len() as f64;
            let mut f = rc[0].features.clone();
            for id in FeatureId::ALL {
                *f.get_mut(id) = median(rc.iter().map(|c| c.features.get(id)).collect());
            }
            catalog.push(KnownAnalogSite {
                id: format!("CAT_{}", sanitize(region)),
                name: region.clone(),
                lat,
                lon,
                cluster_radius_km: rc.iter().map(|c| haversine_km(lat, lon, c.lat, c.lon)).fold(0.0, f64::max),
                features: f,
            });
        }
        if !catalog.is_empty() {
            if out_dir.join("catalog.json").exists() && !force {
                return Err("catalog.json exists; refusing to overwrite (use --force)".into());
            }
            fs::write(out_dir.join("catalog.json"), serde_json::to_string_pretty(&catalog)?)?;
            report.catalog_sites_written = catalog.len();
        }
        report.warnings.push("labels.json built from export roles: 'known_analog' is a generic label, not proof the site is an analog for THIS experiment.".into());
    }
    report.earth_cells_written = cells.len();
    report.targets_written = targets.len();
    fs::write(out_dir.join("import_report.json"), serde_json::to_string_pretty(&report)?)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowsun_windows_follow_solar_geometry() {
        // Equinox declination ~0°, solstices ~±23.4°.
        assert!(solar_declination_deg(80, 365).abs() < 1.0);
        assert!((solar_declination_deg(172, 365) - 23.44).abs() < 0.5);
        // At 75.4°N the noon sun is within (0°, 3°] only shortly before/after polar night.
        let w = lowsun_windows("Devon", "lowsun", 75.4, 2027, 0.0, 3.0);
        assert_eq!(w.len(), 2, "{w:?}");
        assert!(w[0].window.start_utc.starts_with("2027-01") || w[0].window.start_utc.starts_with("2027-02"));
        assert!(w[1].window.start_utc.starts_with("2027-10") || w[1].window.start_utc.starts_with("2027-11"));
        // Mid-latitudes never get an all-day low sun.
        assert!(lowsun_windows("Kamchatka", "lowsun", 55.8, 2027, 0.0, 3.0).is_empty());
    }

    #[test]
    fn utm_roundtrip_and_reference_points() {
        // Central meridian of zone 32 (9°E) maps to easting 500 000; equator to northing 0.
        let (e, n) = latlon_to_utm(0.0, 9.0, 32);
        assert!((e - 500_000.0).abs() < 1e-6 && n.abs() < 1e-6);
        for (lat, lon, zone) in [(42.0, 9.0, 32), (55.83, 160.33, 57), (-24.2, -67.6, 19), (75.4, -89.7, 16), (64.9, -16.9, 28)] {
            let (e, n) = latlon_to_utm(lat, lon, zone);
            let (la, lo) = utm_to_latlon(e, n, zone, lat >= 0.0);
            assert!((la - lat).abs() < 1e-7 && (lo - lon).abs() < 1e-7, "{lat},{lon} -> {la},{lo}");
        }
        // Independent check: 42°N 9°E -> northing 4 649 776.2 m (standard UTM table value).
        let (_, n) = latlon_to_utm(42.0, 9.0, 32);
        assert!((n - 4_649_776.22).abs() < 0.1, "northing {n}");
    }

    fn rec(id: &str, body: &str, region: &str, sh: Option<f64>) -> String {
        format!(
            r#"{{"id":"{id}","body":"{body}","region":"{region}","role":"candidate","dem":"NASADEM","epsg":32632,
            "center_x":500000.0,"center_y":4649776.22,"tile_px":33,"cell_size_m":30.0,"quality_q":0.97,
            "slope_deg_1x":10.0,"slope_deg_3x":8.5,"slope_deg_9x":6.0,"rough_m_1x":0.4,"rough_m_3x":0.5,"rough_m_9x":0.7,
            "shadow_1deg":{s},"shadow_2deg":{s},"shadow_3deg":{s}}}"#,
            s = sh.map(|v| v.to_string()).unwrap_or("null".into())
        )
    }

    #[test]
    fn import_converts_skips_and_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("star_import_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let earth = format!(r#"{{"schema_version":"star-temp-0","cells":[{},{}]}}"#, rec("E1", "earth", "Alps", Some(0.5)), rec("E2", "earth", "Alps", None));
        let moon = format!(r#"{{"schema_version":"star-temp-0","cells":[{},{},{}]}}"#,
            rec("M1", "moon", "Shackleton", Some(0.6)), rec("M2", "moon", "Shackleton", Some(0.7)), rec("M3", "moon", "Shackleton", Some(0.8)));
        fs::write(dir.join("earth.json"), earth).unwrap();
        fs::write(dir.join("moon.json"), moon).unwrap();
        fs::write(dir.join("manifest.json"), r#"{"sources":[
            {"file":"earth.json","vertical_uncertainty_m":3.0,"source_version":"NASADEM v001","processing_version":"t"},
            {"file":"moon.json","vertical_uncertainty_m":1.0,"source_version":"LOLA","processing_version":"t"}]}"#).unwrap();
        let out = dir.join("out");
        let rep = run_import(&dir.join("manifest.json"), &out, false).unwrap();
        assert_eq!(rep.earth_cells_written, 1);
        assert_eq!(rep.skipped.len(), 1, "record without shadows is skipped, not zero-filled");
        assert!(rep.skipped[0].reason.contains("shadow_1deg"));
        let cells: Vec<Cell> = serde_json::from_slice(&fs::read(out.join("cells.json")).unwrap()).unwrap();
        assert!((cells[0].lat - 42.0).abs() < 1e-6 && (cells[0].lon - 9.0).abs() < 1e-6, "inverse UTM");
        assert_eq!(cells[0].data_origin, DataOrigin::ExternalMeasured);
        let targets: Vec<TargetProfile> = serde_json::from_slice(&fs::read(out.join("targets.json")).unwrap()).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id, "MOON_SHACKLETON");
        assert!((targets[0].features.shadow_obstruction_1deg - 0.7).abs() < 1e-12, "median");
        assert!(run_import(&dir.join("manifest.json"), &out, false).is_err(), "must not overwrite");
        let _ = fs::remove_dir_all(&dir);
    }
}
