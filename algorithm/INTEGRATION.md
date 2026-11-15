# Integrating the v0.6.2 discovery engine

The algorithm source is stored in `star-core` unchanged from the supplied archive. The desktop app depends on this crate directly; `src/discovery/integration.rs` only loads its documented JSON inputs and builds a reproducibility manifest.

## Real-data path

The app's **Terrain Analysis → Discovery Export** step reads a projected DEM, averages 5 m / 10 m / 15 m pixels to the common 30 m scale, extracts the six multiscale terrain features, and calls the bundled `star-core` low-sun raycaster for obstruction at 1°, 2°, and 3°. A 3 km padded area is required. Tiles with insufficient valid rays keep shadow values missing, so the importer reports and skips them instead of treating missing data as zero.

1. Obtain and cite real DEMs. NASA's [PGDA LOLA products](https://pgda.gsfc.nasa.gov/data/LOLA_5mpp/) provide the six selected 5 m/pixel lunar site GeoTIFFs in south-polar stereographic metres. For Earth, the five sites within NASADEM coverage use NASADEM; high-latitude Haughton uses Copernicus GLO-30. The supplied `tools/download_project_dems.ps1` and `tools/prepare_dem.py` scripts download and project the source DEMs described in `data/README.md`.
2. Prepare the source rasters and keep the source/version and vertical uncertainty for each one. The UI exports one `star-temp-0` JSON per DEM; assign each a source entry in an `import_manifest.json` based on `real_data_template/import_manifest.json`.
3. In **Analog Discovery**, click **Import source manifest**, select the manifest, and choose a new output folder. This calls the bundled importer in-process and refuses to overwrite an existing bundle. The equivalent CLI command is:

   ```powershell
   cargo run --release -p star-cli -- --import .\real\import_manifest.json --out-dir .\real\bundle
   ```

4. The app selects the imported folder automatically. Set a target ID if the mission spec does not select one, run the assessment, then save the JSON report.

The app expects `cells.json`, `targets.json`, and `mission_spec.json`; `catalog.json`, `evidence.json`, `labs.json`, `site_windows.json`, `params.json`, and `campaign_constraints.json` are optional. The CLI writes the complete import report and records every skipped source record.

## Data status

The six selected lunar source rasters and six Earth-region source rasters have been downloaded locally and projected 30 m GeoTIFFs are in `data/moon/processed/` and `data/earth/processed/`. All TIFF files are ignored by Git because of their size; `data/README.md` documents the reproducible download and preparation steps. Haughton uses Copernicus GLO-30 rather than the originally proposed ASTER GDEM because the latter requires an Earthdata login in this environment. Product-level uncertainty values are not site-specific error estimates. Do not use `algorithm/demo_data` as scientific evidence, and do not claim validated analog transfer without independent evidence.
