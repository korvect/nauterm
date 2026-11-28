param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("x86_64", "arm64")]
  [string]$Arch
)

$ErrorActionPreference = "Stop"

# Keep in sync with GHOSTTY_LIB_VERSION in native/nauterm_core/build.rs.
$version = "1.0.2"
if ($Arch -eq "arm64") {
  $target = "aarch64"
  $expectedSha256 = "463f092cd94540580b1e4d3b3e57661b60eceafbf9f74f53ecb2db5eb05cbdd9"
} else {
  $target = "x86_64"
  $expectedSha256 = "f16ec4c167631e7feb24b9793d1f4b8e9b922e959c900dee984b111bb94a8fa8"
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
