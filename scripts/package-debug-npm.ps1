param(
    [string]$Version,
    [string]$OutputDir,
    [string]$PackageName = "@local/cowodex",
    [string[]]$BinName = @("codex", "cowodex"),
    [string]$TargetTriple = "x86_64-pc-windows-msvc",
    [switch]$SkipBuild,
    [switch]$Install
)

$ErrorActionPreference = "Stop"

$scriptPath = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptPath "..")
$codexRs = Join-Path $repoRoot "codex-rs"
$codexCli = Join-Path $repoRoot "codex-cli"

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = "0.0.0-debug.$(Get-Date -Format 'yyyyMMddHHmmss')"
}

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = Join-Path $repoRoot "dist\npm-debug"
}

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) {
    $env:Path = "$cargoBin;$env:Path"
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo is required. Install Rust first, then rerun this script."
}

if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
    throw "pnpm is required. Install pnpm first, then rerun this script."
}

if ($BinName.Count -eq 0) {
    throw "At least one bin name is required."
}

if (-not $SkipBuild) {
    Push-Location $codexRs
    try {
        cargo build -p codex-cli --bin codex

        if ($TargetTriple -like "*windows*") {
            cargo build -p codex-windows-sandbox --bin codex-windows-sandbox-setup --bin codex-command-runner
        }
    } finally {
        Pop-Location
    }
}

$targetDebug = Join-Path $codexRs "target\debug"
$codexExe = Join-Path $targetDebug "codex.exe"
if (-not (Test-Path $codexExe)) {
    throw "Missing debug Codex binary: $codexExe"
}

$vendorRoot = Join-Path $codexCli "vendor\$TargetTriple"
$vendorCodex = Join-Path $vendorRoot "codex"
$vendorPath = Join-Path $vendorRoot "path"
New-Item -ItemType Directory -Force $vendorCodex, $vendorPath | Out-Null

Copy-Item $codexExe (Join-Path $vendorCodex "codex.exe") -Force

if ($TargetTriple -like "*windows*") {
    foreach ($helper in @("codex-windows-sandbox-setup.exe", "codex-command-runner.exe")) {
        $src = Join-Path $targetDebug $helper
        if (-not (Test-Path $src)) {
            throw "Missing debug helper binary: $src"
        }
        Copy-Item $src (Join-Path $vendorCodex $helper) -Force
    }
}

$rg = Get-Command rg -ErrorAction SilentlyContinue
if ($rg) {
    Copy-Item $rg.Source (Join-Path $vendorPath "rg.exe") -Force
} else {
    Write-Warning "rg.exe was not found on PATH; packaged Codex will still run, but search tools may be unavailable."
}

$stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) "codex-debug-npm"
$stagingDir = Join-Path $stagingRoot ([System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force $stagingDir | Out-Null

try {
    Copy-Item (Join-Path $codexCli "bin") (Join-Path $stagingDir "bin") -Recurse -Force
    Copy-Item (Join-Path $codexCli "vendor") (Join-Path $stagingDir "vendor") -Recurse -Force
    Copy-Item (Join-Path $codexCli "package.json") (Join-Path $stagingDir "package.json") -Force

    $readme = Join-Path $repoRoot "README.md"
    if (Test-Path $readme) {
        Copy-Item $readme (Join-Path $stagingDir "README.md") -Force
    }

    $packageJsonPath = Join-Path $stagingDir "package.json"
    $packageJson = Get-Content $packageJsonPath -Raw | ConvertFrom-Json
    $packageJson.name = $PackageName
    $packageJson.version = $Version
    $packageJson.files = @("bin", "vendor")
    $bin = [ordered]@{}
    foreach ($name in $BinName) {
        if ([string]::IsNullOrWhiteSpace($name)) {
            throw "Bin names cannot be empty."
        }
        $bin[$name] = "bin/codex.js"
    }
    $packageJson.bin = $bin
    if ($packageJson.PSObject.Properties.Name -contains "optionalDependencies") {
        $packageJson.PSObject.Properties.Remove("optionalDependencies")
    }
    $packageJson | ConvertTo-Json -Depth 20 | Set-Content $packageJsonPath -Encoding UTF8

    New-Item -ItemType Directory -Force $OutputDir | Out-Null

    Push-Location $stagingDir
    try {
        pnpm pack --pack-destination $OutputDir
    } finally {
        Pop-Location
    }

    $tarball = Get-ChildItem $OutputDir -Filter "*.tgz" |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if (-not $tarball) {
        throw "pnpm pack completed but no .tgz was found in $OutputDir"
    }

    Write-Host "Created debug npm package:"
    Write-Host $tarball.FullName

    if ($Install) {
        pnpm add -g $tarball.FullName --force
        Write-Host "Installed globally. Verify with: $($BinName[0]) --version"
    } else {
        Write-Host ""
        Write-Host "Install locally with:"
        Write-Host "pnpm add -g `"$($tarball.FullName)`" --force"
        Write-Host ""
        Write-Host "Then run:"
        foreach ($name in $BinName) {
            Write-Host "$name --version"
        }
    }
} finally {
    if (Test-Path $stagingDir) {
        Remove-Item $stagingDir -Recurse -Force
    }
}
