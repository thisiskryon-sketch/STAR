# Real-data run: one traceable NASA chain

Goal: NASA source → raw DEM → preprocessing → features → STAR → result. Every step is recorded in `import_manifest.json` and in the run manifest.

## 0. Prerequisites

- Merge the teammate patch `0001-Compute-low-sun-horizon-obstruction-per-tile-and-exp.patch` into the GeoTIFF app. Without it, `shadow_*deg` is always empty and no cell can be imported.
- GDAL installed (`gdalwarp`).
- A NASA Earthdata login (for LP DAAC downloads).

## 1. Common scale: 30 m

LOLA 5 m/pix south-pole DEMs are mostly interpolated, with an effective resolution of ~15–35 m. NASADEM and ASTER GDEM are ~30 m. All features are therefore computed at **30 m** for both bodies; `params.json` enforces this (`max_resolution_ratio` = 1.5).

Limitation to state on the slides: 30 m DEMs cannot resolve rover-scale (sub-metre) roughness. Roughness is used only at 9× scale and is marked `Exploratory`.

## 2. Moon targets: PGDA product 78 (LOLA 5 m/pix landing-site DEMs)

- Page: https://pgda.gsfc.nasa.gov/products/78
- Data: https://pgda.gsfc.nasa.gov/data/LOLA_5mpp/
- Files are GeoTIFFs in south polar stereographic metres, so the app accepts them directly.
- Sites: Site01 (Connecting ridge), Site04 (Shackleton rim); others per the README.
- Reported median RMS height error is 0.30–0.50 m → `vertical_uncertainty_m` = 0.5.

Resample to the common 30 m scale:

```
gdalwarp -tr 30 30 -r average Site01_LDEM.tif site01_30m.tif
```

Export from the app with body = `moon`, region = site name.

## 3. Earth regions

- **≤ 60° latitude:** NASADEM (NASA JPL).
  - OpenTopography, doi:10.5069/G93T9FD9, or LP DAAC.
- **> 60° latitude:** ASTER GDEM v003 (NASA/METI, LP DAAC, doi:10.5067/ASTER/ASTGTM.003).
  - Covers 83°N–83°S; NASADEM/SRTM stops at 60°N.

Vertical uncertainty values in `import_manifest.json`:
- NASADEM: 6 m — from ~10 m relative LE90 → σ ≈ 6 m. **Verify in the NASADEM user guide.**
- ASTER GDEM: 17 m — **replace with the value from the ASTER GDEM v3 validation report.**

`params.json` sets `max_vertical_uncertainty_m` = 20 so ASTER cells are not rejected by QC. State this choice on the slides.

Reproject to UTM at 30 m. Pad every region by at least 3 km: the shadow raycaster needs that radius, and border tiles otherwise get "unknown".

```
gdalwarp -t_srs EPSG:<UTM> -tr 30 30 -r bilinear in.tif region_utm30.tif
```

| Region (app `region`) | Role (app `role`) | Lat, lon | DEM | UTM EPSG |
|---|---|---|---|---|
| Haughton_Devon | known_analog | 75.4, −89.7 | ASTER GDEM | 32616 |
| Meteor_Crater | known_analog | 35.03, −111.02 | NASADEM | 32612 |
| Mauna_Kea | known_analog | 19.82, −155.47 | NASADEM | 32605 |
| Atacama | known_analog | −24.5, −69.25 | NASADEM | 32719 |
| Tolbachik_Kamchatka | candidate | 55.8, 160.3 | NASADEM | 32657 |
| Amazon | negative_control | −3.0, −60.0 | NASADEM | 32721 |

These roles come from the team's `catalog.json`. A `known_analog` label is generic: it is not proof that the site is an analog for *this* experiment, and the report says so.

## 4. Convert and add time windows

```
star-cli --import real/import_manifest.json --out-dir real
star-cli --lowsun-windows --region Haughton_Devon --mechanism sustained_low_sun_illumination \
         --lat 75.4 --year 2027 > real/site_windows.json
```

The import writes:
- `cells.json`, `targets.json` (median per lunar site), `labels.json`, `catalog.json` (median features of known-analog regions);
- `import_report.json`, which lists skipped records and the reason for each — never zero-filled.

The windows tool computes the days when the noon sun stays within 0–3° all day. At 75.4°N in 2027 these are about 11–19 Feb and 25 Oct – 3 Nov.

Copy `mission_spec.json`, `labs.json`, `params.json`, `campaign_constraints.json` and `evidence.json` from this template.

## 5. Run

```
star-cli --input-dir real --output real/report.json --all-targets --validate
```

`--all-targets` sets each lunar site's measured values as the target and keeps the mission tolerance widths.

## 6. Evidence

`evidence.json` is empty on purpose, so transfer requirements stay Unknown/Provisional. Add an entry only for a source you have read, with `valid_feature_bounds` covering the conditions it was obtained in. The labs (NASA Glenn SLOPE and VF-13) are real facilities described from public NASA pages. Their `transfer_evidence` is empty until a source shows that their results transfer for this experiment.
