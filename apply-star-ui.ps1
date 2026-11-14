$ErrorActionPreference = "Stop"

$project = (Get-Location).Path
$source = Join-Path $project "src\main.rs"

if (-not (Test-Path (Join-Path $project "Cargo.toml"))) {
    throw "Cargo.toml не найден. Запусти скрипт из корня star-backend."
}

if (-not (Test-Path $source)) {
    throw "src\main.rs не найден."
}

$backupDir = Join-Path $project ("backups\ui_before_redesign_" + (Get-Date -Format "yyyyMMdd_HHmmss"))
New-Item -ItemType Directory -Force -Path $backupDir | Out-Null
Copy-Item $source (Join-Path $backupDir "main.rs") -Force

$zip = Join-Path $project "star-ui-redesign-package.zip"
if (-not (Test-Path $zip)) {
    throw "Положи star-ui-update.zip в корень проекта рядом с Cargo.toml."
}

$temp = Join-Path $env:TEMP ("star-ui-update_" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $temp | Out-Null
Expand-Archive -LiteralPath $zip -DestinationPath $temp -Force

$updatedMain = Join-Path $temp "src\main.rs"
if (-not (Test-Path $updatedMain)) {
    throw "В архиве отсутствует src\main.rs."
}

Copy-Item $updatedMain $source -Force
Remove-Item $temp -Recurse -Force

Write-Host "`n=== FORMAT ===" -ForegroundColor Cyan
cargo fmt
if ($LASTEXITCODE -ne 0) { throw "cargo fmt завершился с ошибкой." }

Write-Host "`n=== TESTS ===" -ForegroundColor Cyan
cargo test
if ($LASTEXITCODE -ne 0) { throw "cargo test завершился с ошибкой. Исходный main.rs сохранён в $backupDir" }

Write-Host "`n=== BUILD ===" -ForegroundColor Cyan
cargo build
if ($LASTEXITCODE -ne 0) { throw "cargo build завершился с ошибкой. Исходный main.rs сохранён в $backupDir" }

Write-Host "`nInstallation complete. Starting S.T.A.R..." -ForegroundColor Green
cargo run
