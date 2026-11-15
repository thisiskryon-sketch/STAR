# Project DEM data

The selected campaign has six lunar sites and six terrestrial regions. Source rasters and prepared 30 m GeoTIFFs are intentionally excluded from Git because the source set is over 200 MB. This keeps repository clones small; run the included scripts to recreate the local files.

## Download and prepare

From the repository root in PowerShell:

```powershell
.\tools\download_project_dems.ps1
python -m pip install numpy rasterio
.\tools\prepare_project_dems.ps1
```

The download script fetches six LOLA source DEMs, the NASADEM tiles needed for the five lower-latitude Earth sites, and a Copernicus GLO-30 tile for Haughton. The preparation script reprojects Earth data into local UTM, lunar data into south-polar stereographic metres, and writes a 30 m raster and `.meta.json` provenance file for each region. The application reads the prepared rasters from `data/moon/processed/` and `data/earth/processed/`.

## Sources and uncertainty notes

| Region | Product | Processing CRS | Product-level vertical uncertainty used | Source |
|---|---|---|---:|---|
| Site01, Site04, Site07, Site11, Site20, Site23 | NASA PGDA LOLA Product 78, 5 m/pixel | Lunar south-polar stereographic | 0.5 m (product total-Z RMS estimate; verify per-site uncertainty raster) | [NASA PGDA LOLA](https://pgda.gsfc.nasa.gov/data/LOLA_5mpp/) |
| Meteor Crater, Mauna Kea, Atacama, Tolbachik/Kamchatka, Amazon | NASADEM v001, 1 arc-second | Local UTM, 30 m | 6 m (approximate 1-sigma from the published 10 m 90% relative-height requirement) | [NASADEM product guide](https://lpdaac.usgs.gov/documents/2237/NASADEM_User_Guide_V13.pdf), [OpenTopography mirror](https://opentopography.s3.sdsc.edu/raster/NASADEM/) |
| Haughton/Devon | Copernicus DEM GLO-30, 1 arc-second DSM | EPSG:32616, 30 m | 4 m conservative product bound; published absolute vertical accuracy is <4 m at 90% linear error, not a site-specific sigma | [Copernicus DEM](https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM), [public tile](https://copernicus-dem-30m.s3.amazonaws.com/Copernicus_DSM_COG_10_N75_00_W090_00_DEM/Copernicus_DSM_COG_10_N75_00_W090_00_DEM.tif) |

The original plan selected ASTER GDEM for Haughton because NASADEM stops at high latitudes. ASTER GDEM V003 requires NASA Earthdata access; this prepared checkout uses Copernicus GLO-30 as an explicitly documented substitute. Copernicus GLO-30 is a surface model, so vegetation or structures can affect elevation. Do not silently label it ASTER or treat product-wide accuracy values as local ground-truth measurements.

All Earth output bounding boxes include surrounding terrain for the low-sun raycaster. `prepare_dem.py` writes source paths, resampling method, output CRS, resolution, valid-cell counts and GDAL/Rasterio versions beside each prepared raster. Missing data remains NoData; the workflow does not invent zero elevations.
