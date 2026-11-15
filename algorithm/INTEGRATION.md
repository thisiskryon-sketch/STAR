# Integrating the v0.6.2 discovery engine

The algorithm source is stored in `star-core` unchanged from the supplied archive. The desktop app depends on this crate directly; `src/discovery/integration.rs` only loads its documented JSON inputs and builds a reproducibility manifest.

## Real-data path

The app's **Terrain Analysis → Discovery Export** step reads a projected DEM, averages 5 m / 10 m / 15 m pixels to the common 30 m scale, extracts the six multiscale terrain features, and calls the bundled `star-core` low-sun raycaster for obstruction at 1°, 2°, and 3°. A 3 km padded area is required. Tiles with insufficient valid rays keep shadow values missing, so the importer reports and skips them instead of treating missing data as zero.

1. Obtain and cite real DEMs. NASA's [PGDA Site01 LOLA product](https://pgda.gsfc.nasa.gov/data/LOLA_5mpp/) supplies a 5 m/pixel GeoTIFF in south-polar stereographic metres; the app averages integer-resolution blocks to the required common 30 m scale. For Earth, follow the NASADEM / ASTER choices in `DATA_PLAN.md`; NASA LP DAAC downloads may require the team's Earthdata Login.
2. Prepare the source rasters and keep the source/version and vertical uncertainty for each one. The UI exports one `star-temp-0` JSON per DEM; assign each a source entry in an `import_manifest.json` based on `real_data_template/import_manifest.json`.
3. In **Analog Discovery**, click **Import source manifest**, select the manifest, and choose a new output folder. This calls the bundled importer in-process and refuses to overwrite an existing bundle. The equivalent CLI command is:

   ```powershell
   cargo run --release -p star-cli -- --import .\real\import_manifest.json --out-dir .\real\bundle
   ```

4. The app selects the imported folder automatically. Set a target ID if the mission spec does not select one, run the assessment, then save the JSON report.

The app expects `cells.json`, `targets.json`, and `mission_spec.json`; `catalog.json`, `evidence.json`, `labs.json`, `site_windows.json`, `params.json`, and `campaign_constraints.json` are optional. The CLI writes the complete import report and records every skipped source record.

## Data status

The NASA Site01 source raster is available locally at `data/moon/Site01_final_adj_5mpp_surf.tif` and is ignored by Git due to its size. The repository does not yet contain the matching real Earth DEMs or the remaining lunar targets, so there is not yet a complete Earth-to-Moon campaign bundle. A result becomes a real-data result only after the team supplies the cited source rasters, uncertainty values, and processing metadata. Do not use `algorithm/demo_data` as scientific evidence.
