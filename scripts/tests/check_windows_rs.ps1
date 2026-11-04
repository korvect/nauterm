#requires -Version 7.0
[CmdletBinding()]
param([switch]$Launch)

$ErrorActionPreference = "Stop"
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
  throw "Run this check on Windows x64 from an x64 Visual Studio Developer PowerShell."
}
if (-not [Environment]::Is64BitProcess -or $env:PROCESSOR_ARCHITECTURE -ne "AMD64") {
  throw "This check requires an x64 PowerShell process on Windows x64."
}

$project = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$target = "x86_64-pc-windows-msvc"
$toolchainText = Get-Content (Join-Path $project "rust-toolchain.toml") -Raw
if ($toolchainText -notmatch 'channel\s*=\s*"([^"]+)"') { throw "Missing pinned Rust toolchain." }
$toolchain = $Matches[1]
$report = Join-Path ([IO.Path]::GetTempPath()) ("Nauterm-RS-validation-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $report | Out-Null
$checks = [Collections.Generic.List[object]]::new()
$status = "failed"
$sourceRevision = $null
$names = @("OPENSSL_DIR", "OPENSSL_INCLUDE_DIR", "OPENSSL_LIB_DIR", "LIB", "PATH", "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER", "NAUTERM_RS_DATA_DIR")
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name, "Process") }

function Invoke-Check {
  param([string]$Name, [string]$Program, [string[]]$Arguments)
  Write-Host "`n== $Name =="
  $log = Join-Path $report "$Name.log"
  $oldPreference = $ErrorActionPreference
  try {
    # Cargo uses stderr for ordinary progress too. Record both streams and
    # inspect its exit code rather than treating progress as a failure.
    $ErrorActionPreference = "Continue"
    $PSNativeCommandUseErrorActionPreference = $false
    & $Program @Arguments 2>&1 | ForEach-Object { $_.ToString() } | Tee-Object -FilePath $log
    $code = $LASTEXITCODE
  } finally { $ErrorActionPreference = $oldPreference }
  $checks.Add([pscustomobject]@{ name = $Name; exitCode = $code; log = $log })
  if ($code -ne 0) { throw "$Name failed with exit code $code. See $log" }
}

Push-Location $project
try {
  foreach ($command in @("git", "cargo", "rustup", "python", "powershell.exe", "cl.exe", "rc.exe")) {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
      throw "$command was not found. See nauterm-rs/docs/windows-local-validation.md."
    }
  }
  $sourceRevision = (& git rev-parse HEAD | Out-String).Trim()
  if ($env:VSCMD_ARG_TGT_ARCH -and $env:VSCMD_ARG_TGT_ARCH -ne "x64") {
    throw "The Visual Studio environment targets $($env:VSCMD_ARG_TGT_ARCH); select the x64 target."
  }
  $compiler = Get-Command cl.exe -ErrorAction Stop
  $linker = Join-Path (Split-Path $compiler.Source) "link.exe"
  if (-not (Test-Path -LiteralPath $linker)) { throw "The MSVC linker was not found beside cl.exe." }
  $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = $linker

  $zig = if ($env:NAUTERM_ZIG) { $env:NAUTERM_ZIG } else { "zig" }
  if (-not (Get-Command $zig -ErrorAction SilentlyContinue)) { throw "Zig 0.16.0 was not found." }
  $zigVersion = (& $zig version | Out-String).Trim()
  if ($LASTEXITCODE -ne 0 -or $zigVersion -ne "0.16.0") { throw "Zig 0.16.0 is required; found $zigVersion." }

  $openssl = $env:OPENSSL_DIR
  if (-not $openssl) {
    $openssl = @("C:\OpenSSL-Win64", "C:\Program Files\OpenSSL-Win64") |
      Where-Object { Test-Path -LiteralPath (Join-Path $_ "include\openssl\ssl.h") } | Select-Object -First 1
  }
  if (-not $openssl) { throw "OpenSSL was not found. Run scripts/ci/setup_windows_openssl.ps1 -Arch x86_64 first." }
  $include = if ($env:OPENSSL_INCLUDE_DIR) { $env:OPENSSL_INCLUDE_DIR } else { Join-Path $openssl "include" }
  $lib = if ($env:OPENSSL_LIB_DIR) { $env:OPENSSL_LIB_DIR } else {
    @((Join-Path $openssl "lib\VC\x64\MD"), (Join-Path $openssl "lib")) |
      Where-Object { Test-Path -LiteralPath (Join-Path $_ "libcrypto.lib") } | Select-Object -First 1
  }
  if (-not $lib -or -not (Test-Path -LiteralPath (Join-Path $include "openssl\ssl.h"))) { throw "OpenSSL headers/import library were not found." }
  $bin = Join-Path $openssl "bin"
  if (-not (Get-ChildItem -LiteralPath $bin -Filter "libcrypto-*-x64.dll" -File -ErrorAction SilentlyContinue)) { throw "The x64 OpenSSL runtime DLL is missing." }
  $env:OPENSSL_DIR = $openssl
  $env:OPENSSL_INCLUDE_DIR = $include
  $env:OPENSSL_LIB_DIR = $lib
  $env:LIB = "$lib;$env:LIB"
  $env:PATH = "$bin;$env:PATH"

  Invoke-Check -Name "python" -Program "python" -Arguments @("-c", "import sys; assert sys.version_info >= (3, 10), 'Python 3.10+ is required'")
  Invoke-Check -Name "toolchain" -Program "rustup" -Arguments @("toolchain", "install", $toolchain, "--profile", "minimal", "--component", "clippy", "--component", "rustfmt")
  Invoke-Check -Name "target" -Program "rustup" -Arguments @("target", "add", $target, "--toolchain", $toolchain)
  Invoke-Check -Name "format" -Program "cargo" -Arguments @("+$toolchain", "fmt", "--all", "--check")
  Invoke-Check -Name "clippy" -Program "cargo" -Arguments @("+$toolchain", "clippy", "-p", "nauterm_core", "-p", "nauterm-rs", "-p", "nauterm_cmd_integration", "--all-targets", "--target", $target, "--locked", "--", "-D", "warnings")
  Invoke-Check -Name "rs-tests" -Program "cargo" -Arguments @("+$toolchain", "test", "-p", "nauterm-rs", "--target", $target, "--locked")
  Invoke-Check -Name "powershell-tests" -Program "cargo" -Arguments @("+$toolchain", "test", "-p", "nauterm_core", "--features", "terminal-ghostty", "--target", $target, "--locked", "pty::powershell")
  Invoke-Check -Name "cmd-protocol-tests" -Program "cargo" -Arguments @("+$toolchain", "test", "-p", "nauterm_cmd_integration", "--target", $target, "--locked")
  Invoke-Check -Name "build" -Program "cargo" -Arguments @("+$toolchain", "build", "-p", "nauterm-rs", "--target", $target, "--locked")

  $metadataText = & cargo "+$toolchain" metadata --format-version 1 --no-deps --locked
  if ($LASTEXITCODE -ne 0) { throw "Unable to read Cargo target directory." }
  $metadata = ($metadataText | Out-String) | ConvertFrom-Json
  $executable = Join-Path $metadata.target_directory "$target/debug/nauterm-rs.exe"
  Invoke-Check -Name "mosh-runtime" -Program "python" -Arguments @("scripts/stage_rs_mosh.py", "--project", $project, "--executable", $executable)
  $status = "build-and-tests-passed"
  if ($Launch) {
    $env:NAUTERM_RS_DATA_DIR = Join-Path $report "app-data"
    Write-Host "`nStarting RS with test data at $($env:NAUTERM_RS_DATA_DIR)"
    & $executable
    if ($LASTEXITCODE -ne 0) { $status = "runtime-exited-unsuccessfully"; throw "RS exited with code $LASTEXITCODE." }
    $status = "build-and-tests-passed-runtime-launched"
  }
} finally {
  try {
    [pscustomobject]@{
      status = $status
      target = $target
      toolchain = $toolchain
      os = [Environment]::OSVersion.VersionString
      source = $sourceRevision
      checks = $checks.ToArray()
    } | ConvertTo-Json -Depth 5 | Set-Content -Path (Join-Path $report "summary.json") -Encoding UTF8
  } finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $previous[$name], "Process") }
    Pop-Location
  }
  Write-Host "`nValidation report: $report"
}
