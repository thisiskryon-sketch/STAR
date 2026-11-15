<div align="center">

# S.T.A.R.

### Spatio-Temporal & Physical Analog Discovery

**A Rust-based scientific desktop application for terrain analysis and planetary surface analog research.**

[![Rust](https://img.shields.io/badge/Rust-2024-orange?logo=rust)](https://www.rust-lang.org/)
[![Desktop](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-informational)](#)
[![Status](https://img.shields.io/badge/Status-Active%20Development-yellow)](#project-status)

*Developed in the context of NASA Space Apps Challenge 2026.*

</div>

---

## Overview

S.T.A.R. (Spatio-Temporal & Physical Analog Discovery) is a native desktop research prototype designed to explore how terrestrial landscapes can be characterized and compared with candidate locations on the Moon and Mars.

The current implementation focuses on **digital elevation model (DEM) ingestion and terrain-feature extraction**. It provides a foundation for a broader workflow in which terrain morphology and, as the project evolves, thermal and environmental signatures can support the search for planetary surface analogs.

> **Project status:** S.T.A.R. is under active development. The current terrain-analysis workflow is implemented as a research prototype. Full Earth–Moon–Mars analog ranking and validated planetary datasets should not be considered complete unless explicitly demonstrated by the current code and included datasets.

## Current capabilities

- Native desktop interface built with Rust, `eframe` and `egui`.
- DEM loading from GeoTIFF.
- Raster metadata handling, including dimensions, cell size and coordinate reference system (CRS).
- Validation of projected coordinate systems for metric terrain calculations.
- Terrain fingerprint extraction, including:
  - elevation;
  - slope;
  - local relief;
  - surface roughness;
  - curvature proxy.
- Batch processing of raster cells and coverage reporting.
- Terrain visualizations, including an elevation map, elevation profile and elevation distribution.
- Synthetic DEM generation for testing the workflow without external datasets.

## Technology

| Component | Technology |
|---|---|
| Language | Rust (Edition 2024) |
| Desktop UI | `eframe` / `egui` |
| Raster input | GeoTIFF (`tiff` crate) |
| File selection | `rfd` |
| Build system | Cargo |

## Getting started

### Requirements

- Rust toolchain with Cargo.
- A supported desktop environment.
- A projected, single-band GeoTIFF DEM for metric terrain analysis.

Install Rust from [rustup.rs](https://rustup.rs/).

### Build and run

Clone the repository:

```bash
git clone https://github.com/thisiskryon-sketch/STAR.git
cd STAR
```

Build and test:

```bash
cargo fmt --check
cargo test --workspace
cargo build --release
```

Run the application:

```bash
cargo run --release
```

On Windows, these commands can be executed from PowerShell in the project directory.

### Mission discovery with real DEM data

The v0.6.2 mission-discovery engine is included in `algorithm/star-core` and is called directly by the desktop app's **Analog Discovery** page. It is kept separate from the existing terrain-analysis library. See [the integration guide](algorithm/INTEGRATION.md) for the NASA DEM workflow, input bundle contract, and import command. The files in `algorithm/demo_data` are synthetic logic-demo inputs, not scientific results.

## Working with elevation data

1. Launch S.T.A.R.
2. Open **Terrain Analysis**.
3. Use **Load GeoTIFF** to select a DEM file.
4. Inspect the raster metadata and elevation visualization.
5. Set the analysis neighborhood radius.
6. Run terrain analysis and inspect the resulting terrain fingerprints and coverage statistics.

### GeoTIFF requirements

The current metric terrain-analysis pipeline expects:

- a single-band elevation raster;
- a **projected CRS** with linear units (normally metres);
- north-up raster orientation;
- square pixels.

Geographic coordinate systems such as **EPSG:4326** store coordinates in degrees and are not suitable for the current metric calculations. Reproject such rasters before loading them.

For example, a DEM covering the Alps can be reprojected to WGS 84 / UTM zone 32N:

```bash
gdalwarp \
  -t_srs EPSG:32632 \
  -tr 30 30 \
  -r bilinear \
  input_dem.tif \
  alps_dem_utm.tif
```

The output CRS must be appropriate for the geographic area being analyzed. Do not use EPSG:32632 for locations outside UTM zone 32N.

## Project structure

```text
STAR/
├── algorithm/
│   ├── star-core/       # bundled v0.6.2 discovery algorithm
│   ├── star-cli/        # input importer and CLI runner
│   └── real_data_template/
├── data/
│   ├── earth/
│   ├── known_analogs/
│   ├── mars/
│   ├── moon/
│   └── targets/
├── docs/
├── src/
│   ├── app/
│   ├── discovery/
│   ├── features/
│   ├── models/
│   ├── preprocessing/
│   ├── validation/
│   ├── lib.rs
│   └── main.rs
├── tests/
├── Cargo.toml
└── README.md
```

The exact contents may evolve as development progresses.

## Scientific scope

Terrain fingerprints are intended to provide quantitative descriptors for comparing landforms. The current curvature value is a local Laplacian-based proxy; it should not be interpreted as a universally defined mean or Gaussian curvature measurement.

The quality and scientific validity of any analog result depend on the source DEM, its resolution, projection, NoData handling, preprocessing choices and the validation methodology. Results from synthetic DEMs are for software testing only and are not scientific observations.

## Data sources

Use authoritative datasets and retain their original attribution and licensing information. Potential sources include:

- [Copernicus DEM](https://dataspace.copernicus.eu/)
- [OpenTopography](https://opentopography.org/)
- [USGS 3D Elevation Program](https://www.usgs.gov/3d-elevation-program)
- [NASA Planetary Data System](https://pds.nasa.gov/)

Large raster datasets are intentionally not stored in the Git repository. Keep source data, derived rasters and credentials out of commits unless there is a clear licensing and size justification.

## Roadmap

- [x] Native Rust desktop application foundation
- [x] DEM ingestion and raster validation
- [x] Terrain fingerprint calculations
- [x] Batch terrain extraction and coverage reporting
- [x] Initial terrain visualization and analysis workflow
- [ ] Expanded raster format and CRS support
- [ ] Advanced terrain layers (slope, hillshade and contours)
- [ ] Interactive 3D terrain visualization
- [ ] Thermal and temporal feature integration
- [ ] Planetary DEM ingestion for lunar and Martian regions
- [ ] Earth–Moon–Mars analog scoring and validation
- [ ] Reproducible benchmark datasets and scientific evaluation

## Contributing

Issues, suggestions and technical discussions are welcome. For substantial changes, open an issue first to discuss the proposed approach.

## License

No license has been declared yet. Until a license is added to this repository, all rights are reserved by the copyright holder. Do not assume that the code is available for reuse, redistribution or modification.

---

<div align="center">

**S.T.A.R. — exploring terrestrial analogs for planetary exploration.**

</div>
