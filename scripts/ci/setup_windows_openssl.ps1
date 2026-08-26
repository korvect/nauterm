$ErrorActionPreference = "Stop"

choco install openssl --version 4.0.1 -y --no-progress

$opensslCandidates = @(
  (Join-Path $env:ProgramFiles "OpenSSL-Win64"),
  (Join-Path $env:ProgramFiles "OpenSSL")
)
$opensslDir = $opensslCandidates | Where-Object {
  Test-Path (Join-Path $_ "include\openssl\ssl.h")
} | Select-Object -First 1
if (-not $opensslDir) {
  throw "OpenSSL installation was not found. Checked: $($opensslCandidates -join ', ')"
}

$opensslLibCandidates = @(
  (Join-Path $opensslDir "lib"),
  (Join-Path $opensslDir "lib\VC\x64\MD")
)
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

# The runtime DLLs (libcrypto-3-*.dll / libssl-3-*.dll) live in OpenSSL's bin
# directory. nauterm_ffi links OpenSSL (via rusqlite's bundled-sqlcipher), so the
# test executable needs them on PATH at runtime, otherwise it fails with
# STATUS_DLL_NOT_FOUND (0xc0000135).
$opensslBinDir = Join-Path $opensslDir "bin"
if (-not (Test-Path (Join-Path $opensslBinDir "libcrypto-3-x64.dll"))) {
  throw "OpenSSL runtime DLLs were not found at $opensslBinDir"
}
Add-Content -Path $env:GITHUB_PATH -Value $opensslBinDir
