# Lunar DEM source

`Site01_final_adj_5mpp_surf.tif` is the NASA PGDA Site01 LOLA surface DEM for the Connecting Ridge. The original 5 m/pixel GeoTIFF is in south-polar stereographic metres (`MOON_ME`, DE421). The desktop exporter averages it to the project's common 30 m scale before feature extraction.

- Source: [NASA PGDA South Pole Landing Site LOLA DEMs](https://pgda.gsfc.nasa.gov/data/LOLA_5mpp/)
- Product: `Site01_final_adj_5mpp_surf.tif`
- Original size: 40,980,810 bytes; 3,200 × 3,200 pixels
- SHA-256: `3ba7b97cb00a2bcf21189c3aeb535f65afc21207154ab9f0d43c5bdc1f7e747e`
- Source vertical uncertainty for the mission import manifest: use the value documented by the product. The real-data template currently proposes 0.5 m; verify it against the product documentation before reporting results.

The raster is excluded from Git by the repository's `*.tif` rule. Download it from the source above when preparing a fresh checkout.
