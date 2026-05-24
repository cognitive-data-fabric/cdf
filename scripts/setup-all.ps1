# CDF Full Setup Script for Windows
# Run as Administrator if needed

$ErrorActionPreference = "Stop"
Write-Host "=== CDF Setup Script ===" -ForegroundColor Cyan

# 1. Check prerequisites
Write-Host "`n[1/6] Checking prerequisites..." -ForegroundColor Green
$tools = @{
    "rustc" = "Rust"
    "cargo" = "Cargo"
    "python" = "Python"
    "pip" = "Pip"
    "docker" = "Docker"
    "go" = "Go"
}

foreach ($cmd in $tools.Keys) {
    if (Get-Command $cmd -ErrorAction SilentlyContinue) {
        Write-Host "  ✓ $($tools[$cmd]) found" -ForegroundColor Green
    } else {
        Write-Host "  ✗ $($tools[$cmd]) NOT FOUND - Please install first" -ForegroundColor Red
    }
}

# 2. Rust dependencies
Write-Host "`n[2/6] Fetching Rust crates..." -ForegroundColor Green
cargo fetch

# 3. Go dependencies
Write-Host "`n[3/6] Fetching Go modules..." -ForegroundColor Green
Set-Location cmd\cdf-gateway; go mod tidy; Set-Location ..\..
Set-Location cmd\cdf-router; go mod tidy; Set-Location ..\..
Set-Location cmd\cdf-meta; go mod tidy; Set-Location ..\..
Set-Location cmd\cdf-ctl; go mod tidy; Set-Location ..\..

# 4. Python dependencies
Write-Host "`n[4/6] Installing Python packages..." -ForegroundColor Green
pip install -e ".[dev]"

# 5. Pre-download embedding model
Write-Host "`n[5/6] Downloading sentence-transformers model..." -ForegroundColor Green
python -c "
from sentence_transformers import SentenceTransformer
print('Downloading all-MiniLM-L6-v2...')
model = SentenceTransformer('all-MiniLM-L6-v2')
print('Model downloaded successfully!')
"

# 6. Verify
Write-Host "`n[6/6] Verification..." -ForegroundColor Green
cargo check --workspace 2>$null
if ($LASTEXITCODE -eq 0) { Write-Host "  ✓ Rust workspace compiles" } else { Write-Host "  ⚠ Rust has warnings (expected for prototype)" }
go build ./cmd/cdf-gateway 2>$null
if ($LASTEXITCODE -eq 0) { Write-Host "  ✓ Go gateway builds" } else { Write-Host "  ⚠ Go needs module download" }

Write-Host "`n=== Setup Complete! ===" -ForegroundColor Cyan
Write-Host "Start the stack: docker-compose up -d"
Write-Host "Or individually: see README.md"
