param([string]$PythonExe = 'python')

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$python = Get-Command $PythonExe -ErrorAction SilentlyContinue
if (-not $python) {
    throw "Python executable '$PythonExe' was not found. Install Python or pass -PythonExe with its path."
}

& $python.Source -c 'import numpy, rasterio' 2>$null
if ($LASTEXITCODE -ne 0) {
    throw 'Install the preparation dependencies with: python -m pip install numpy rasterio'
}

$prepareScript = Join-Path $PSScriptRoot 'prepare_dem.py'
$moonDir = Join-Path $repoRoot 'data/moon'
$earthRawDir = Join-Path $repoRoot 'data/earth/raw'
$moonOutputDir = Join-Path $moonDir 'processed'
$earthOutputDir = Join-Path $repoRoot 'data/earth/processed'
$env:PYTHONIOENCODING = 'utf-8'
Push-Location $repoRoot
try {

foreach ($site in @('Site01', 'Site04', 'Site07', 'Site11', 'Site20', 'Site23')) {
    $inputPath = Join-Path $moonDir "${site}_final_adj_5mpp_surf.tif"
    & $python.Source $prepareScript --body moon --region $site --input $inputPath --res 30 --resample average --out $moonOutputDir --dem-name "NASA PGDA LOLA Product 78 $site"
    if ($LASTEXITCODE -ne 0) { throw "Failed to prepare lunar site $site" }
}

$plans = @(
    @{ Name='Meteor_Crater'; Bbox=@('-111.12','34.95','-110.92','35.11'); Inputs=@('NASADEM_HGT_n35w112.tif','NASADEM_HGT_n35w111.tif','NASADEM_HGT_n34w112.tif','NASADEM_HGT_n34w111.tif'); Source='NASADEM v001, OpenTopography public mirror' },
    @{ Name='Mauna_Kea'; Bbox=@('-155.58','19.70','-155.36','19.94'); Inputs=@('NASADEM_HGT_n19w156.tif','NASADEM_HGT_n19w155.tif'); Source='NASADEM v001, OpenTopography public mirror' },
    @{ Name='Atacama'; Bbox=@('-69.34','-24.62','-69.16','-24.38'); Inputs=@('NASADEM_HGT_s25w070.tif'); Source='NASADEM v001, OpenTopography public mirror' },
    @{ Name='Tolbachik_Kamchatka'; Bbox=@('160.24','55.72','160.36','55.88'); Inputs=@('NASADEM_HGT_n55e160.tif'); Source='NASADEM v001, OpenTopography public mirror' },
    @{ Name='Amazon'; Bbox=@('-60.12','-3.12','-59.88','-2.88'); Inputs=@('NASADEM_HGT_s03w061.tif','NASADEM_HGT_s03w060.tif','NASADEM_HGT_s04w061.tif','NASADEM_HGT_s04w060.tif'); Source='NASADEM v001, OpenTopography public mirror' },
    @{ Name='Haughton_Devon'; Bbox=@('-89.86','75.25','-89.54','75.55'); Inputs=@('Copernicus_DSM_COG_10_N75_00_W090_00_DEM.tif'); Source='Copernicus DEM GLO-30' }
)

foreach ($plan in $plans) {
    $argumentList = [System.Collections.Generic.List[string]]::new()
    $argumentList.AddRange([string[]]@('--body','earth','--region',$plan.Name,'--input'))
    foreach ($file in $plan.Inputs) { $argumentList.Add((Join-Path $earthRawDir $file)) }
    foreach ($value in @('--bbox') + $plan.Bbox + @('--res','30','--resample','average','--out',$earthOutputDir,'--dem-name',$plan.Source)) { $argumentList.Add([string]$value) }
    & $python.Source $prepareScript @($argumentList.ToArray())
    if ($LASTEXITCODE -ne 0) { throw "Failed to prepare Earth region $($plan.Name)" }
}

    Write-Host 'All project DEMs are prepared at 30 m. See data/README.md for coverage and source limitations.'
}
finally {
    Pop-Location
}
