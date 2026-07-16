[CmdletBinding()]
param(
  [switch]$ForceDownload
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $repoRoot 'src-tauri\dependencies.windows.lock.json'
$cacheRoot = Join-Path $repoRoot 'src-tauri\target\dependency-cache\windows'
$extractRoot = Join-Path $cacheRoot 'extracted'
$tauriConfigPath = Join-Path $repoRoot 'src-tauri\tauri.windows.full.conf.json'

function Get-ArtifactName {
  param([Parameter(Mandatory = $true)][string]$Url)

  $uri = [Uri]$Url
  return [Uri]::UnescapeDataString([IO.Path]::GetFileName($uri.AbsolutePath))
}

function Assert-Sha256 {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Expected
  )

  $stream = [IO.File]::OpenRead($Path)
  try {
    $sha256 = [Security.Cryptography.SHA256]::Create()
    try {
      $actual = ([BitConverter]::ToString($sha256.ComputeHash($stream))).Replace('-', '').ToLowerInvariant()
    }
    finally {
      $sha256.Dispose()
    }
  }
  finally {
    $stream.Dispose()
  }
  if ($actual -ne $Expected.ToLowerInvariant()) {
    throw "SHA-256 mismatch for '$Path'. Expected $Expected, got $actual."
  }
}

function Copy-DependencyFile {
  param(
    [Parameter(Mandatory = $true)]$Dependency,
    [Parameter(Mandatory = $true)][string]$ArtifactPath,
    [Parameter(Mandatory = $true)]$File,
    [string]$ExpandedPath
  )

  $targetPath = Join-Path $repoRoot ([string]$File.target)
  $targetDir = Split-Path -Parent $targetPath
  New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

  if ($Dependency.archive -eq 'file') {
    $sourcePath = $ArtifactPath
  }
  elseif ($Dependency.archive -eq 'zip') {
    $matches = @(Get-ChildItem -LiteralPath $ExpandedPath -Recurse -File -Filter ([string]$File.source))
    if ($matches.Count -ne 1) {
      throw "Expected one '$($File.source)' in $($Dependency.name), found $($matches.Count)."
    }
    $sourcePath = $matches[0].FullName
  }
  else {
    throw "Unsupported archive type '$($Dependency.archive)' for $($Dependency.name)."
  }

  Copy-Item -LiteralPath $sourcePath -Destination $targetPath -Force
  if (-not (Test-Path -LiteralPath $targetPath -PathType Leaf)) {
    throw "Dependency output was not created: $targetPath"
  }
  Write-Host "Prepared $($Dependency.name): $targetPath"
}

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
  throw "Dependency lock manifest not found: $manifestPath"
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.schemaVersion -ne 1 -or $manifest.platform -ne 'windows-x86_64') {
  throw "Unsupported dependency manifest schema/platform in $manifestPath"
}

New-Item -ItemType Directory -Force -Path $cacheRoot | Out-Null
New-Item -ItemType Directory -Force -Path $extractRoot | Out-Null

foreach ($dependency in $manifest.dependencies) {
  $artifactName = Get-ArtifactName -Url ([string]$dependency.url)
  $artifactPath = Join-Path $cacheRoot $artifactName
  $needsDownload = $ForceDownload -or -not (Test-Path -LiteralPath $artifactPath -PathType Leaf)

  if (-not $needsDownload) {
    try {
      Assert-Sha256 -Path $artifactPath -Expected ([string]$dependency.sha256)
    }
    catch {
      Remove-Item -LiteralPath $artifactPath -Force
      $needsDownload = $true
    }
  }

  if ($needsDownload) {
    Write-Host "Downloading $($dependency.name) $($dependency.version)..."
    $temporaryPath = "$artifactPath.download"
    Remove-Item -LiteralPath $temporaryPath -Force -ErrorAction SilentlyContinue
    Invoke-WebRequest -Uri ([string]$dependency.url) -OutFile $temporaryPath
    Assert-Sha256 -Path $temporaryPath -Expected ([string]$dependency.sha256)
    Move-Item -LiteralPath $temporaryPath -Destination $artifactPath -Force
  }

  Assert-Sha256 -Path $artifactPath -Expected ([string]$dependency.sha256)

  $expandedPath = $null
  if ($dependency.archive -eq 'zip') {
    $expandedPath = Join-Path $extractRoot ([string]$dependency.name)
    Remove-Item -LiteralPath $expandedPath -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $expandedPath | Out-Null
    Expand-Archive -LiteralPath $artifactPath -DestinationPath $expandedPath -Force
  }

  foreach ($file in $dependency.files) {
    Copy-DependencyFile -Dependency $dependency -ArtifactPath $artifactPath -File $file -ExpandedPath $expandedPath
  }
}

$version = (Get-Content -LiteralPath (Join-Path $repoRoot 'package.json') -Raw | ConvertFrom-Json).version
$versionMatch = [regex]::Match($version, '^(\d+)\.(\d+)\.(\d+)(?:-custom\.(\d+))?$')
if (-not $versionMatch.Success) {
  throw "Cannot derive an MSI version from app version '$version'."
}

$versionParts = @(
  [int]$versionMatch.Groups[1].Value
  [int]$versionMatch.Groups[2].Value
  [int]$versionMatch.Groups[3].Value
  $(if ($versionMatch.Groups[4].Success) { [int]$versionMatch.Groups[4].Value } else { 0 })
)
if ($versionParts[0] -gt 255 -or $versionParts[1] -gt 255 -or $versionParts[2] -gt 65535 -or $versionParts[3] -gt 65535) {
  throw "Derived MSI version fields exceed Windows Installer limits: $($versionParts -join '.')"
}

$resources = [ordered]@{
  'resources/dependencies/windows/ffmpeg.exe' = 'dependencies/ffmpeg.exe'
  'resources/dependencies/windows/ffprobe.exe' = 'dependencies/ffprobe.exe'
  'resources/dependencies/windows/deno.exe' = 'dependencies/deno.exe'
  'resources/dependencies/windows/gallery-dl.exe' = 'dependencies/gallery-dl.exe'
  'resources/dependencies/THIRD_PARTY_NOTICES.txt' = 'dependencies/THIRD_PARTY_NOTICES.txt'
  'dependencies.windows.lock.json' = 'dependencies/dependencies.windows.lock.json'
}

$fullConfig = [ordered]@{
  bundle = [ordered]@{
    resources = $resources
    windows = [ordered]@{
      wix = [ordered]@{
        version = ($versionParts -join '.')
      }
    }
  }
}

$configJson = $fullConfig | ConvertTo-Json -Depth 8
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText($tauriConfigPath, $configJson, $utf8NoBom)

$checks = @(
  @('src-tauri\bin\youwee-yt-dlp-x86_64-pc-windows-msvc.exe', '--version'),
  @('src-tauri\resources\dependencies\windows\ffmpeg.exe', '-version'),
  @('src-tauri\resources\dependencies\windows\ffprobe.exe', '-version'),
  @('src-tauri\resources\dependencies\windows\deno.exe', '--version'),
  @('src-tauri\resources\dependencies\windows\gallery-dl.exe', '--version')
)

foreach ($check in $checks) {
  $binaryPath = Join-Path $repoRoot $check[0]
  $output = & $binaryPath $check[1] 2>&1
  if ($LASTEXITCODE -ne 0) {
    throw "Dependency smoke check failed: $binaryPath $($check[1])"
  }
  $firstLine = @($output)[0]
  Write-Host "Verified $($check[0]): $firstLine"
}

Write-Host "Generated Tauri Full Installer config: $tauriConfigPath"
