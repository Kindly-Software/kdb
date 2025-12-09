# MSIX Package Validation Script
# Validates MSIX package before Microsoft Store submission
# PowerShell 7.0+ required

#Requires -Version 7.0

param(
    [string]$PackagePath = "",
    [switch]$Verbose = $false
)

$ErrorActionPreference = "Stop"

# Color output helpers
function Write-Check { param($Message) Write-Host "✓ $Message" -ForegroundColor Green }
function Write-Fail { param($Message) Write-Host "✗ $Message" -ForegroundColor Red }
function Write-Warn { param($Message) Write-Host "⚠ $Message" -ForegroundColor Yellow }
function Write-Info { param($Message) Write-Host "ℹ $Message" -ForegroundColor Cyan }

$ValidationErrors = 0
$ValidationWarnings = 0

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  kindly-av1 MSIX Package Validation" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host ""

# Auto-detect package if not specified
if ([string]::IsNullOrWhiteSpace($PackagePath)) {
    $OutputDir = Join-Path $PSScriptRoot "output"
    $Packages = Get-ChildItem -Path $OutputDir -Filter "*.msix" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending

    if ($Packages.Count -eq 0) {
        Write-Fail "No MSIX packages found in output/ directory"
        Write-Info "Run .\build-msix.ps1 first to create a package"
        exit 1
    }

    $PackagePath = $Packages[0].FullName
    Write-Info "Auto-detected package: $($Packages[0].Name)"
}

if (-not (Test-Path $PackagePath)) {
    Write-Fail "Package not found: $PackagePath"
    exit 1
}

Write-Host ""
Write-Host "Package: $(Split-Path -Leaf $PackagePath)" -ForegroundColor White
Write-Host "Size: $([math]::Round((Get-Item $PackagePath).Length / 1MB, 2)) MB" -ForegroundColor White
Write-Host ""

# Validation 1: Check file size
Write-Info "Validating package size..."
$PackageSize = (Get-Item $PackagePath).Length
$MaxSize = 100MB

if ($PackageSize -gt $MaxSize) {
    Write-Fail "Package too large: $([math]::Round($PackageSize / 1MB, 2)) MB (max 100 MB recommended)"
    $ValidationErrors++
} else {
    Write-Check "Package size OK: $([math]::Round($PackageSize / 1MB, 2)) MB"
}

# Validation 2: Check assets exist
Write-Info "Validating assets..."
$AssetsDir = Join-Path $PSScriptRoot "Assets"
$RequiredAssets = @(
    "StoreLogo.png",
    "Square44x44Logo.png",
    "Square150x150Logo.png",
    "Wide310x150Logo.png",
    "LargeTile.png"
)

$AssetErrors = 0
foreach ($Asset in $RequiredAssets) {
    $AssetPath = Join-Path $AssetsDir $Asset
    if (Test-Path $AssetPath) {
        $AssetSize = (Get-Item $AssetPath).Length
        if ($AssetSize -lt 1KB) {
            Write-Warn "$Asset is too small ($AssetSize bytes) - may be placeholder"
            $ValidationWarnings++
        } else {
            Write-Check "$Asset exists ($([math]::Round($AssetSize / 1KB, 2)) KB)"
        }
    } else {
        Write-Fail "$Asset missing"
        $AssetErrors++
    }
}

if ($AssetErrors -gt 0) {
    $ValidationErrors += $AssetErrors
}

# Validation 3: Check manifest
Write-Info "Validating manifest..."
$ManifestPath = Join-Path $PSScriptRoot "Package.appxmanifest"

if (Test-Path $ManifestPath) {
    try {
        [xml]$Manifest = Get-Content $ManifestPath
        $Identity = $Manifest.Package.Identity

        Write-Check "Manifest syntax valid"

        # Check version format
        if ($Identity.Version -match '^\d+\.\d+\.\d+\.\d+$') {
            Write-Check "Version format valid: $($Identity.Version)"
        } else {
            Write-Fail "Invalid version format: $($Identity.Version) (expected X.Y.Z.W)"
            $ValidationErrors++
        }

        # Check publisher
        if ($Identity.Publisher -like "CN=*") {
            Write-Check "Publisher format valid: $($Identity.Publisher)"
        } else {
            Write-Fail "Invalid publisher format: $($Identity.Publisher) (expected CN=...)"
            $ValidationErrors++
        }

        # Check display name
        $DisplayName = $Manifest.Package.Properties.DisplayName
        if ($DisplayName -eq "kindly-av1") {
            Write-Check "Display name correct: $DisplayName"
        } else {
            Write-Warn "Display name may be incorrect: $DisplayName (expected 'kindly-av1')"
            $ValidationWarnings++
        }

    } catch {
        Write-Fail "Manifest XML syntax error: $_"
        $ValidationErrors++
    }
} else {
    Write-Fail "Package.appxmanifest not found"
    $ValidationErrors++
}

# Validation 4: Check if package is signed
Write-Info "Validating package signature..."

$SignCheckOutput = & signtool.exe verify /pa $PackagePath 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Check "Package is signed"
} else {
    Write-Warn "Package is not signed or signature invalid"
    Write-Info "For Microsoft Store submission, package must be signed with Partner Center certificate"
    $ValidationWarnings++
}

# Validation 5: Windows App Certification Kit (if available)
Write-Info "Checking for Windows App Certification Kit (WACK)..."
$WackPath = "C:\Program Files (x86)\Windows Kits\10\App Certification Kit\appcert.exe"

if (Test-Path $WackPath) {
    Write-Check "WACK available: $WackPath"
    Write-Info "Run WACK validation manually:"
    Write-Host "  & `"$WackPath`" -appxpackagepath `"$PackagePath`" -reportoutputpath validation-report.xml" -ForegroundColor Gray
} else {
    Write-Warn "WACK not found. Install Windows SDK for full validation."
    $ValidationWarnings++
}

# Validation 6: Check for common issues
Write-Info "Checking for common issues..."

# Check if binary exists in local build
$BinaryPath = Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "target\x86_64-pc-windows-msvc\release\kindly-av1.exe"
if (Test-Path $BinaryPath) {
    $BinarySize = (Get-Item $BinaryPath).Length / 1MB
    Write-Check "Binary found: $([math]::Round($BinarySize, 2)) MB"
} else {
    Write-Warn "Binary not found at expected path: $BinaryPath"
    Write-Info "This is OK if you built on a different machine"
    $ValidationWarnings++
}

# Summary
Write-Host ""
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  Validation Summary" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host ""

if ($ValidationErrors -eq 0 -and $ValidationWarnings -eq 0) {
    Write-Check "All validations passed!"
    Write-Host ""
    Write-Host "✓ Ready for Microsoft Store submission" -ForegroundColor Green
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor White
    Write-Host "  1. Upload to Partner Center (https://partner.microsoft.com/dashboard)" -ForegroundColor Gray
    Write-Host "  2. Complete store listing (description, screenshots, pricing)" -ForegroundColor Gray
    Write-Host "  3. Submit for certification" -ForegroundColor Gray
    Write-Host "  4. Wait 1-3 business days for review" -ForegroundColor Gray
    Write-Host ""
    exit 0
} elseif ($ValidationErrors -eq 0) {
    Write-Warn "$ValidationWarnings warning(s) found"
    Write-Host ""
    Write-Host "⚠ Package may be ready, but review warnings above" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Recommendations:" -ForegroundColor White
    Write-Host "  - Replace placeholder assets with branded designs" -ForegroundColor Gray
    Write-Host "  - Sign package with production certificate" -ForegroundColor Gray
    Write-Host "  - Run WACK validation for full compliance check" -ForegroundColor Gray
    Write-Host ""
    exit 0
} else {
    Write-Fail "$ValidationErrors error(s), $ValidationWarnings warning(s) found"
    Write-Host ""
    Write-Host "✗ Package NOT ready for submission" -ForegroundColor Red
    Write-Host ""
    Write-Host "Fix the errors above and re-run validation" -ForegroundColor White
    Write-Host ""
    exit 1
}
