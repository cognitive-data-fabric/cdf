# Development helper script for PowerShell
param(
    [Parameter()]
    [ValidateSet("start", "stop", "build", "test", "lint", "clean")]
    [string]$Action = "start"
)

switch ($Action) {
    "start" {
        Write-Host "Starting development environment..." -ForegroundColor Green
        docker-compose up -d
        Write-Host "App running at http://localhost:3000" -ForegroundColor Cyan
    }
    "stop" {
        Write-Host "Stopping development environment..." -ForegroundColor Yellow
        docker-compose down
    }
    "build" {
        Write-Host "Building production image..." -ForegroundColor Green
        docker build --target prod -t myapp:latest .
    }
    "test" {
        Write-Host "Running tests..." -ForegroundColor Green
        docker-compose exec app npm test
    }
    "lint" {
        Write-Host "Running linter..." -ForegroundColor Green
        docker-compose exec app npm run lint
    }
    "clean" {
        Write-Host "Cleaning up containers and volumes..." -ForegroundColor Red
        docker-compose down -v --remove-orphans
        docker system prune -f
    }
}
