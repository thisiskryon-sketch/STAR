# S.T.A.R. — validate, stage and publish the project to GitHub
# Run from the repository root in PowerShell.
$ErrorActionPreference = "Stop"

$root = (Get-Location).Path
$remote = "https://github.com/thisiskryon-sketch/STAR.git"

Write-Host "`n=== S.T.A.R. GitHub publishing ===" -ForegroundColor Cyan
Write-Host "Repository: $root"

if (-not (Test-Path ".\Cargo.toml")) {
    throw "Cargo.toml was not found. Run this script from the STAR project root."
}
if (-not (Test-Path ".\.git")) {
    throw "This folder is not a Git repository."
}

# Install the prepared README.
$readmeSource = Join-Path $PSScriptRoot "README.md"
if (-not (Test-Path $readmeSource)) {
    throw "README.md from the release kit is missing."
}
if ((Resolve-Path $readmeSource).Path -ne (Join-Path $root "README.md")) { Copy-Item $readmeSource ".\README.md" -Force }

# Append safe exclusions without replacing the existing .gitignore.
$ignorePath = ".\.gitignore"
if (-not (Test-Path $ignorePath)) {
    New-Item -ItemType File -Path $ignorePath | Out-Null
}
$ignoreText = @'

# --- S.T.A.R. local/generated files ---
/target/
/backups/
/_star_ui_package/
*.zip
*.7z
*.rar
*.tif
*.tiff
*.vrt
*.aux.xml
*.ovr
*.gpkg
*.sqlite
*.db
.env
.env.*
!.env.example
.DS_Store
Thumbs.db
'@
Add-Content -Path $ignorePath -Value $ignoreText -Encoding UTF8

Write-Host "`n[1/5] Formatting..." -ForegroundColor Yellow
cargo fmt --all
if ($LASTEXITCODE -ne 0) { throw "cargo fmt failed." }

Write-Host "`n[2/5] Running tests..." -ForegroundColor Yellow
cargo test
if ($LASTEXITCODE -ne 0) { throw "cargo test failed. Nothing has been pushed." }

Write-Host "`n[3/5] Building release..." -ForegroundColor Yellow
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed. Nothing has been pushed." }

Write-Host "`n[4/5] Checking files before staging..." -ForegroundColor Yellow
git remote -v
git status --short

# GitHub rejects individual files larger than 100 MB. Stop before staging them.
$largeFiles = Get-ChildItem -Path . -Recurse -File -Force |
    Where-Object {
        $_.FullName -notmatch '\\(\.git|target|backups|_star_ui_package)\\' -and
        $_.Length -gt 90MB
    }
if ($largeFiles) {
    $largeFiles | Select-Object FullName, Length | Format-Table -AutoSize
    throw "Files larger than 90 MB found. Keep large datasets/build artifacts out of Git or use Git LFS."
}

# Guard against accidentally committing common secrets.
$secretFiles = @(".env", ".env.local", "id_rsa", "id_ed25519")
foreach ($file in $secretFiles) {
    if (Test-Path $file) {
        throw "Potential secret file found: $file. Remove it or keep it out of Git before continuing."
    }
}

Write-Host "`nReview the status above. The following command stages all non-ignored project files." -ForegroundColor Cyan
git add -A
if ($LASTEXITCODE -ne 0) { throw "git add failed." }

Write-Host "`nStaged changes:" -ForegroundColor Green
git status --short

$confirm = Read-Host "`nType PUSH to commit and push these files to GitHub"
if ($confirm -cne "PUSH") {
    Write-Host "Cancelled. Your changes remain staged locally; nothing was pushed." -ForegroundColor Yellow
    exit 0
}

git commit -m "Build S.T.A.R. terrain analysis application and documentation"
if ($LASTEXITCODE -ne 0) {
    throw "git commit failed. If Git says there is nothing to commit, check git status."
}

# Ensure origin points to the intended repository.
$origin = git remote get-url origin 2>$null
if (-not $origin) {
    git remote add origin $remote
} elseif ($origin -ne $remote -and $origin -ne "git@github.com:thisiskryon-sketch/STAR.git") {
    throw "The origin remote is '$origin', not the expected STAR repository. Review it before pushing."
}

$branch = (git branch --show-current).Trim()
if (-not $branch) { throw "Could not determine the current Git branch." }

git push -u origin $branch
if ($LASTEXITCODE -ne 0) {
    throw "Push failed. Check GitHub authentication and the current branch. Your local commit is preserved."
}

Write-Host "`nPublished successfully: https://github.com/thisiskryon-sketch/STAR" -ForegroundColor Green
