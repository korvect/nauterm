param(
  [ValidateSet("x86_64", "arm64")]
  [string]$Arch = "x86_64"
)

$ErrorActionPreference = "Stop"

$manifestUrl = "https://raw.githubusercontent.com/slproweb/opensslhashes/master/win32_openssl_hashes.json"
$manifest = Invoke-RestMethod -Uri $manifestUrl -MaximumRetryCount 3 -RetryIntervalSec 5
$packageArchitecture = if ($Arch -eq "arm64") { "ARM" } else { "INTEL" }
$packagePrefix = if ($Arch -eq "arm64") { "Win64ARMOpenSSL-" } else { "Win64OpenSSL-" }
$package = $manifest.files.PSObject.Properties |
  ForEach-Object {
    [PSCustomObject]@{
      Name = $_.Name
      Version = $_.Value.basever
      Architecture = $_.Value.arch
      Bits = $_.Value.bits
      Light = $_.Value.light
      Installer = $_.Value.installer
      Url = $_.Value.url
      Sha256 = $_.Value.sha256
    }
  } |
  Where-Object {
    $_.Name.StartsWith($packagePrefix) -and
    $_.Architecture -eq $packageArchitecture -and
    $_.Bits -eq 64 -and
    -not $_.Light -and
    $_.Installer -eq "exe" -and
    $_.Version -like "4.0.*"
  } |
  Sort-Object { [version]$_.Version } -Descending |
  Select-Object -First 1
if ($null -eq $package) {
  throw "No $Arch OpenSSL 4.0 installer found in $manifestUrl"
}

$installer = Join-Path $env:TEMP $package.Name
Write-Host "Downloading $($package.Url)"
Invoke-WebRequest -Uri $package.Url -OutFile $installer -UseBasicParsing -MaximumRetryCount 3 -RetryIntervalSec 5
$actualSha256 = (Get-FileHash -Path $installer -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualSha256 -ne $package.Sha256.ToLowerInvariant()) {
  throw "SHA-256 mismatch for $($package.Name): expected $($package.Sha256), got $actualSha256"
}

$opensslDir = if ($Arch -eq "arm64") { "C:\OpenSSL-Win-arm64" } else { "C:\OpenSSL-Win64" }
Write-Host "Installing $Arch OpenSSL $($package.Version) to $opensslDir"
$installProcess = Start-Process -Wait -PassThru -FilePath $installer -ArgumentList "/VERYSILENT", "/DIR=$opensslDir"
if ($installProcess.ExitCode -ne 0) {
  throw "OpenSSL installer exited with code $($installProcess.ExitCode)"
}
if (-not (Test-Path (Join-Path $opensslDir "include\openssl\ssl.h"))) {
  throw "OpenSSL headers were not found at $opensslDir"
}

$opensslLibCandidates = if ($Arch -eq "arm64") {
  @(
    (Join-Path $opensslDir "lib\VC\arm64\MD"),
    (Join-Path $opensslDir "lib")
  )
} else {
  @(
    (Join-Path $opensslDir "lib\VC\x64\MD"),
    (Join-Path $opensslDir "lib")
  )
}
$opensslLibDir = $opensslLibCandidates | Where-Object {
  Test-Path (Join-Path $_ "libcrypto.lib")
} | Select-Object -First 1
if (-not $opensslLibDir) {
  throw "OpenSSL import library libcrypto.lib was not found. Checked: $($opensslLibCandidates -join ', ')"
}

if (-not $env:GITHUB_ENV) {
  throw "GITHUB_ENV is unavailable."
}

Write-Host "Using OpenSSL headers from $opensslDir and libraries from $opensslLibDir"
Add-Content -Path $env:GITHUB_ENV -Value "OPENSSL_DIR=$opensslDir"
Add-Content -Path $env:GITHUB_ENV -Value "OPENSSL_INCLUDE_DIR=$(Join-Path $opensslDir 'include')"
Add-Content -Path $env:GITHUB_ENV -Value "OPENSSL_LIB_DIR=$opensslLibDir"
Add-Content -Path $env:GITHUB_ENV -Value "LIB=$opensslLibDir;$env:LIB"

# nauterm_ffi links OpenSSL through rusqlite's bundled SQLCipher, so native
# tests and the packaged app need the matching runtime DLL on PATH.
$opensslBinDir = Join-Path $opensslDir "bin"
$runtimeArchitecture = if ($Arch -eq "arm64") { "arm64" } else { "x64" }
$runtimeDll = Get-ChildItem -Path $opensslBinDir -Filter "libcrypto-*-$runtimeArchitecture.dll" -File |
  Select-Object -First 1
if (-not $runtimeDll) {
  throw "OpenSSL runtime DLL was not found in $opensslBinDir"
}
Add-Content -Path $env:GITHUB_PATH -Value $opensslBinDir
