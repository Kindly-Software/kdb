# kindly-av1 MSIX Package Build Script
# PowerShell 7.0+ required
# Builds signed MSIX package for Microsoft Store submission

#Requires -Version 7.0

param(
    [string]$Configuration = "Release",
    [string]$Version = "1.0.0.0",
    [string]$Publisher = "CN=Kindly",
    [string]$CertificateThumbprint = "",
    [switch]$SkipSign = $false,
    [switch]$Verbose = $false
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

# Color output helpers
function Write-Step { param($Message) Write-Host "==> $Message" -ForegroundColor Cyan }
function Write-Success { param($Message) Write-Host "✓ $Message" -ForegroundColor Green }
function Write-Error { param($Message) Write-Host "✗ $Message" -ForegroundColor Red }
function Write-Warning { param($Message) Write-Host "⚠ $Message" -ForegroundColor Yellow }

# Paths
$RootDir = Split-Path -Parent $PSScriptRoot
$PackagingDir = $PSScriptRoot
$AssetsDir = Join-Path $PackagingDir "Assets"
$TargetDir = Join-Path $RootDir "target" "x86_64-pc-windows-msvc" $Configuration.ToLower()
$StagingDir = Join-Path $PackagingDir "staging"
$OutputDir = Join-Path $PackagingDir "output"

Write-Step "kindly-av1 MSIX Build Pipeline"
Write-Host "  Version: $Version"
Write-Host "  Configuration: $Configuration"
Write-Host "  Publisher: $Publisher"
Write-Host ""

# Step 1: Validate Prerequisites
Write-Step "Validating prerequisites..."

# Check for Windows SDK tools
$MakeAppxPath = Get-Command "makeappx.exe" -ErrorAction SilentlyContinue
if (-not $MakeAppxPath) {
    Write-Error "makeappx.exe not found. Install Windows SDK 10.0.19041.0 or later."
    Write-Host "Download from: https://developer.microsoft.com/windows/downloads/windows-sdk/"
    exit 1
}

$MakePriPath = Get-Command "makepri.exe" -ErrorAction SilentlyContinue
if (-not $MakePriPath) {
    Write-Warning "makepri.exe not found. Resource indexing will be skipped."
    $SkipPRI = $true
} else {
    $SkipPRI = $false
}

$SignToolPath = Get-Command "signtool.exe" -ErrorAction SilentlyContinue
if (-not $SignToolPath -and -not $SkipSign) {
    Write-Warning "signtool.exe not found. Package will not be signed."
    $SkipSign = $true
}

Write-Success "Prerequisites validated"

# Step 2: Build Binary
Write-Step "Building kindly-av1 binary..."

Push-Location $RootDir
try {
    $BuildArgs = @(
        "build",
        "--bin", "kindly-av1",
        "--release",
        "--target", "x86_64-pc-windows-msvc"
    )

    if ($Verbose) {
        $BuildArgs += "--verbose"
    }

    & cargo @BuildArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo build failed with exit code $LASTEXITCODE"
    }
    Write-Success "Binary built successfully"
} finally {
    Pop-Location
}

# Step 3: Verify Binary Exists
$BinaryPath = Join-Path $TargetDir "kindly-av1.exe"
if (-not (Test-Path $BinaryPath)) {
    Write-Error "Binary not found at: $BinaryPath"
    exit 1
}

$BinarySize = (Get-Item $BinaryPath).Length / 1MB
Write-Host "  Binary size: $($BinarySize.ToString('F2')) MB"

# Step 4: Clean and Create Staging Directory
Write-Step "Preparing staging directory..."

if (Test-Path $StagingDir) {
    Remove-Item -Path $StagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $StagingDir | Out-Null

# Step 5: Copy Files to Staging
Write-Step "Staging package files..."

# Copy binary
Copy-Item -Path $BinaryPath -Destination $StagingDir
Write-Host "  ✓ kindly-av1.exe"

# Copy manifest
Copy-Item -Path (Join-Path $PackagingDir "Package.appxmanifest") -Destination $StagingDir
Write-Host "  ✓ Package.appxmanifest"

# Copy assets
$AssetsStagingDir = Join-Path $StagingDir "Assets"
New-Item -ItemType Directory -Path $AssetsStagingDir | Out-Null
Get-ChildItem -Path $AssetsDir -Filter "*.png" | ForEach-Object {
    Copy-Item -Path $_.FullName -Destination $AssetsStagingDir
    Write-Host "  ✓ Assets/$($_.Name)"
}

Write-Success "Staging complete"

# Step 6: Create Package Resource Index (PRI)
if (-not $SkipPRI) {
    Write-Step "Creating package resource index..."

    $PriConfigPath = Join-Path $PackagingDir "priconfig.xml"
    $PriOutputPath = Join-Path $StagingDir "resources.pri"

    try {
        & makepri.exe createconfig /cf $PriConfigPath /dq en-US /o
        & makepri.exe new /pr $StagingDir /cf $PriConfigPath /of $PriOutputPath /o
        Write-Success "Resource index created"
    } catch {
        Write-Warning "Failed to create PRI file: $_"
    }
}

# Step 7: Create MSIX Package
Write-Step "Creating MSIX package..."

if (Test-Path $OutputDir) {
    Remove-Item -Path $OutputDir -Recurse -Force
}
New-Item -ItemType Directory -Path $OutputDir | Out-Null

$PackageName = "kindly-av1-$Version.msix"
$PackagePath = Join-Path $OutputDir $PackageName

$MakeAppxArgs = @(
    "pack",
    "/d", $StagingDir,
    "/p", $PackagePath,
    "/o"
)

if ($Verbose) {
    $MakeAppxArgs += "/v"
}

& makeappx.exe @MakeAppxArgs
if ($LASTEXITCODE -ne 0) {
    Write-Error "makeappx.exe failed with exit code $LASTEXITCODE"
    exit 1
}

$PackageSize = (Get-Item $PackagePath).Length / 1MB
Write-Success "Package created: $PackageName ($($PackageSize.ToString('F2')) MB)"

# Step 8: Sign Package
if (-not $SkipSign) {
    Write-Step "Signing package..."

    if ([string]::IsNullOrWhiteSpace($CertificateThumbprint)) {
        Write-Warning "No certificate thumbprint provided. Using test certificate."
        Write-Host ""
        Write-Host "For Microsoft Store submission, you need a production certificate:"
        Write-Host "  1. Enroll in Partner Center: https://partner.microsoft.com/dashboard"
        Write-Host "  2. Reserve app name 'kindly-av1'"
        Write-Host "  3. Download certificate from Partner Center > Certificates"
        Write-Host "  4. Install certificate and provide thumbprint to this script"
        Write-Host ""
        Write-Host "Example with production cert:"
        Write-Host "  .\build-msix.ps1 -CertificateThumbprint '1234567890ABCDEF...'"
        Write-Host ""

        # Create test certificate if it doesn't exist
        $TestCertPath = Join-Path $OutputDir "kindly-av1-test.pfx"
        $TestCertPassword = ConvertTo-SecureString -String "test123" -AsPlainText -Force

        if (-not (Test-Path $TestCertPath)) {
            Write-Host "Creating test certificate..."
            & powershell -Command "New-SelfSignedCertificate -Type Custom -Subject 'CN=Kindly' -KeyUsage DigitalSignature -FriendlyName 'Kindly Test Certificate' -CertStoreLocation 'Cert:\CurrentUser\My' -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')"
            $TestCert = Get-ChildItem -Path Cert:\CurrentUser\My | Where-Object {$_.Subject -eq "CN=Kindly"} | Select-Object -First 1
            Export-PfxCertificate -Cert $TestCert -FilePath $TestCertPath -Password $TestCertPassword | Out-Null
        }

        & signtool.exe sign /fd SHA256 /a /f $TestCertPath /p "test123" $PackagePath
    } else {
        & signtool.exe sign /fd SHA256 /sha1 $CertificateThumbprint $PackagePath
    }

    if ($LASTEXITCODE -eq 0) {
        Write-Success "Package signed successfully"
    } else {
        Write-Warning "Signing failed. Package is unsigned."
    }
} else {
    Write-Warning "Package signing skipped"
}

# Step 9: Summary
Write-Host ""
Write-Step "Build Summary"
Write-Host "  Package: $PackagePath"
Write-Host "  Size: $($PackageSize.ToString('F2')) MB"
Write-Host "  Signed: $(-not $SkipSign)"
Write-Host ""

if ($SkipSign -or [string]::IsNullOrWhiteSpace($CertificateThumbprint)) {
    Write-Warning "This package requires signing before Microsoft Store submission"
    Write-Host "  See MICROSOFT_STORE_SETUP.md for certificate acquisition instructions"
}

Write-Success "Build complete!"
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Test package locally: Add-AppxPackage -Path '$PackagePath'"
Write-Host "  2. Submit to Microsoft Store via Partner Center"
Write-Host "  3. See MICROSOFT_STORE_SETUP.md for detailed submission guide"
