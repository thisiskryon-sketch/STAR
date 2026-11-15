# Lunar DEM source

The project uses six South Pole site surface DEMs from NASA PGDA LOLA Product 78: `Site01`, `Site04`, `Site07`, `Site11`, `Site20`, and `Site23`. Each original GeoTIFF is 5 m/pixel in south-polar stereographic metres (`MOON_ME`, DE421); the desktop exporter creates a 30 m average-resampled copy for the app.

- Source: [NASA PGDA South Pole Landing Site LOLA DEMs](https://pgda.gsfc.nasa.gov/data/LOLA_5mpp/)
- Original filenames: `SiteXX_final_adj_5mpp_surf.tif`
- Download and preparation commands: [data/README.md](../README.md)
- Processed outputs: `processed/SiteXX_30m.tif`

The source and processed rasters are excluded from Git by the repository's `*.tif` rule. Download them with `tools/download_project_dems.ps1` before preparing a fresh checkout. Source uncertainty values must be verified against the product's total-Z-uncertainty rasters before interpreting scientific results.
