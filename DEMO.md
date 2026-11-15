---
marp: true
theme: default
paginate: true
---

# S.T.A.R.
## Spatio-Temporal & Physical Analog Discovery

**NASA Space Apps Challenge 2026**

A Rust-based terrain intelligence prototype for characterizing landscapes and building a foundation for planetary surface analog research.

[Project repository](https://github.com/thisiskryon-sketch/STAR)

---

# 1. The mission

Planetary analog research needs measurable surface descriptors — not visual resemblance alone.

**Our question:** How can we describe a landscape consistently enough to compare candidate environments?

S.T.A.R. starts with terrain: elevation data becomes quantitative morphology features.

> Current scope: terrain-data ingestion, analysis and fingerprint extraction. A validated global Earth–Moon–Mars ranking system is not yet complete.

---

# 2. The workflow

1. **Load data** — select a GeoTIFF digital elevation model (DEM).
2. **Validate** — check raster geometry and projected coordinate reference system (CRS).
3. **Analyze** — extract terrain metrics in batch.
4. **Inspect** — explore elevation and derived terrain layers.
5. **Export** — save terrain-cell fingerprints as structured JSON.

**Input requirement:** a projected CRS with linear units (typically metres). Geographic coordinates in degrees are not suitable for current metric calculations.

---

# 3. Terrain analysis

The desktop application provides selectable visualizations derived from the loaded elevation raster:

- **Elevation model** — relative height across the raster.
- **Hillshade** — shaded relief for terrain-form inspection.
- **Slope** — gradient magnitude expressed in degrees.
- **Roughness** — local surface variability.
- **Curvature proxy** — a local Laplacian-based proxy, not a universal curvature measurement.

The interface also supports elevation profiles and elevation-distribution views.

---

# 4. Multiscale terrain fingerprints

For each accepted raster tile, the multiscale module computes:

| Scale | Interpretation | Features |
|---|---|---|
| 1× | Fine local texture | RMS slope + RMS roughness |
| 3× | Intermediate form | RMS slope + RMS roughness |
| 9× | Broader terrain pattern | RMS slope + RMS roughness |

**Quality filter:** Q = valid cells / total tile cells. Tiles below the configured minimum Q can be excluded before feature extraction.

The resulting records preserve scale-specific metrics, tile coordinates, pixel size and source metadata.

---

# 5. Reproducible data export

S.T.A.R. exports validated cell records as JSON (schema: `star-temp-0`).

Each record can include:
- Source body, region, role and DEM name
- EPSG code and cell-centre coordinates
- Tile size, cell size and quality score Q
- Slope and roughness at 1× / 3× / 9×
- Reserved shadow fields for future integration

Validation checks metadata, finite values, metric ranges and duplicate cell identifiers. **Shadow metrics are placeholders (null) in the current exporter.**

---

# 6. Current status & next steps

**Implemented in the repository**
- Rust desktop application (eframe / egui)
- GeoTIFF DEM ingestion and CRS validation
- Terrain layers and batch feature extraction
- Multiscale tile fingerprints and JSON export

**Next development milestones**
- Planetary DEM ingestion for the Moon and Mars
- Thermal and temporal feature integration
- Earth–Moon–Mars analog scoring and validation
- Benchmark datasets and scientific evaluation

**S.T.A.R. — building the terrain-analysis foundation for planetary analog discovery.**

[Explore the source code](https://github.com/thisiskryon-sketch/STAR)
