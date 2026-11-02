param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("x86_64", "arm64")]
  [string]$Arch
)

$ErrorActionPreference = "Stop"

# Keep in sync with GHOSTTY_LIB_VERSION in native/nauterm_core/build.rs.
$version = "1.0.1"
if ($Arch -eq "arm64") {
  $target = "aarch64"
  $expectedSha256 = "cc61da5f968ea0b08639bdf019c6b7da46bad1d985774fd8a2de57af32862892"
} else {
  $target = "x86_64"
  $expectedSha256 = "8e2eb9e1822e85d786b456c8beb86f49d09c318c91f0c2913f91e7ffa12cda29"
}
$package = "ghostty-vt-$version-$target-windows-gnu"
$archive = Join-Path $env:RUNNER_TEMP "$package.zip"
$url = "https://release.files.ghostty.org/libghostty-vt/$version/$package.zip"
Invoke-WebRequest -Uri $url -OutFile $archive -UseBasicParsing

$actualSha256 = (Get-FileHash -Path $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualSha256 -ne $expectedSha256) {
  throw "libghostty-vt archive checksum mismatch: expected $expectedSha256, got $actualSha256"
}

Expand-Archive -Path $archive -DestinationPath $env:RUNNER_TEMP -Force
$packageDir = Join-Path $env:RUNNER_TEMP $package
$dllPath = Join-Path $packageDir "bin\ghostty-vt.dll"
if (-not (Test-Path $dllPath -PathType Leaf)) {
  throw "libghostty-vt DLL not found: $dllPath"
}
$libDir = Join-Path $packageDir "lib"
Add-Content -Path $env:GITHUB_ENV -Value "NAUTERM_GHOSTTY_VT_LIB_DIR=$libDir"
