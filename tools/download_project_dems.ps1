$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$moonDir = Join-Path $repoRoot 'data/moon'
$earthRawDir = Join-Path $repoRoot 'data/earth/raw'
New-Item -ItemType Directory -Path $moonDir, $earthRawDir -Force | Out-Null

function Get-ProjectData([string]$Uri, [string]$Destination) {
    if (Test-Path -LiteralPath $Destination) {
        $existing = Get-Item -LiteralPath $Destination
        if ($existing.Length -gt 0) {
            Write-Host "Already present: $($existing.Name)"
            return
        }
    }

    Write-Host "Downloading $([IO.Path]::GetFileName($Destination))"
    Invoke-WebRequest -Uri $Uri -OutFile $Destination -UseBasicParsing
    if ((Get-Item -LiteralPath $Destination).Length -eq 0) {
        throw "Downloaded file is empty: $Destination"
    }
}

foreach ($site in @('Site01', 'Site04', 'Site07', 'Site11', 'Site20', 'Site23')) {
    $file = "${site}_final_adj_5mpp_surf.tif"
    $uri = "https://pgda.gsfc.nasa.gov/data/LOLA_5mpp/$site/$file"
    Get-ProjectData $uri (Join-Path $moonDir $file)
}

$earthTileNames = @(
    'n35w112', 'n35w111', 'n34w112', 'n34w111',
    'n19w156', 'n19w155', 's25w070', 'n55e160',
    's03w061', 's03w060', 's04w061', 's04w060'
)
foreach ($tile in $earthTileNames) {
    $file = "NASADEM_HGT_${tile}.tif"
    $uri = "https://opentopography.s3.sdsc.edu/raster/NASADEM/NASADEM_be/$file"
    Get-ProjectData $uri (Join-Path $earthRawDir $file)
}

$copernicusFile = 'Copernicus_DSM_COG_10_N75_00_W090_00_DEM.tif'
$copernicusUri = "https://copernicus-dem-30m.s3.amazonaws.com/$($copernicusFile.Replace('.tif', ''))/$copernicusFile"
Get-ProjectData $copernicusUri (Join-Path $earthRawDir $copernicusFile)

Write-Host 'Source DEM download complete. Run tools/prepare_project_dems.ps1 to create the app-ready 30 m rasters.'
