#requires -Version 5.1
[CmdletBinding()]
param(
  [ValidateSet('Build', 'Run')][string]$Mode = 'Build',
  [Parameter(ValueFromRemainingArguments = $true)][string[]]$ExtraArguments
)

# A 32-bit Windows Make resolves System32 to SysWOW64 and starts x86
# PowerShell, where ProgramFiles points at the wrong dependency directory.
# Re-enter through Sysnative before discovering toolchains or dependencies.
if ([Environment]::Is64BitOperatingSystem -and -not [Environment]::Is64BitProcess) {
  $nativePowerShell = Join-Path $env:SystemRoot 'Sysnative/WindowsPowerShell/v1.0/powershell.exe'
  & $nativePowerShell -NoProfile -ExecutionPolicy Bypass -File $PSCommandPath -Mode $Mode @ExtraArguments
  exit $LASTEXITCODE
}

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$encoding = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $encoding
$OutputEncoding = $encoding
$project = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
. (Join-Path $project 'scripts/ci/windows_runtime.ps1')

function Write-ProgressMessage([string]$Message) {
  [Console]::Error.WriteLine($Message)
}

function Get-RuntimeHash([string]$Path) {
  # Do not depend on module autoloading: a PS7 parent can pass an incompatible
  # PSModulePath to Windows PowerShell, including its Get-FileHash module.
  $stream = [IO.File]::OpenRead($Path)
  $sha256 = [Security.Cryptography.SHA256]::Create()
  try {
    return [BitConverter]::ToString($sha256.ComputeHash($stream)).Replace('-', '')
  } finally {
    $sha256.Dispose()
    $stream.Dispose()
  }
}

function Initialize-BuildEnvironment([string]$Target) {
  $architecture = if ($Target -eq 'aarch64-pc-windows-msvc') { 'arm64' } else { 'x64' }
  $compiler = Get-Command cl.exe -ErrorAction SilentlyContinue
  $resourceCompiler = Get-Command rc.exe -ErrorAction SilentlyContinue
  if (-not $compiler -or -not $resourceCompiler -or $env:VSCMD_ARG_TGT_ARCH -ne $architecture) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere)) {
      throw 'Install Visual Studio Build Tools with Desktop development with C++ and a Windows SDK.'
    }
    $component = if ($architecture -eq 'arm64') {
      'Microsoft.VisualStudio.Component.VC.Tools.ARM64'
    } else { 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64' }
    $installation = & $vswhere -latest -products '*' -requires $component -property installationPath
    if (-not $installation) { throw "A Visual Studio C++ installation for $architecture was not found." }
    $developerShell = Join-Path $installation 'Common7/Tools/Launch-VsDevShell.ps1'
    if (-not (Test-Path -LiteralPath $developerShell)) {
      throw "The Visual Studio developer shell was not found at $developerShell. Repair the C++ Build Tools installation."
    }
    $hostArchitecture = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'amd64' }
    Write-ProgressMessage "Configuring Visual Studio ($architecture)..."
    & $developerShell -Arch $architecture -HostArch $hostArchitecture -SkipAutomaticLocation 1>$null 6>$null
  }
  $compiler = Get-Command cl.exe -ErrorAction Stop
  Get-Command rc.exe -ErrorAction Stop | Out-Null
  $linker = Join-Path (Split-Path $compiler.Source) 'link.exe'
  if (-not (Test-Path -LiteralPath $linker)) { throw 'The MSVC linker was not found beside cl.exe.' }
  # Git Bash also supplies link.exe; bind Cargo to the MSVC executable explicitly.
  $linkerName = 'CARGO_TARGET_' + $Target.ToUpperInvariant().Replace('-', '_') + '_LINKER'
  [Environment]::SetEnvironmentVariable($linkerName, $linker, 'Process')

  $roots = @($env:OPENSSL_DIR)
  if ($architecture -eq 'arm64') {
    $roots += @('C:/OpenSSL-Win-arm64', (Join-Path $env:ProgramFiles 'OpenSSL-Win-arm64'))
  } else {
    $roots += @('C:/OpenSSL-Win64', (Join-Path $env:ProgramFiles 'OpenSSL-Win64'))
  }
  $root = $null
  foreach ($candidate in $roots) {
    if ($candidate -and (Test-Path -LiteralPath (Join-Path $candidate 'include/openssl/ssl.h'))) {
      $root = $candidate
      break
    }
  }
  if (-not $root) {
    throw 'OpenSSL headers were not found. Install Windows OpenSSL (not the Light package), or set OPENSSL_DIR.'
  }
  $include = if ($env:OPENSSL_INCLUDE_DIR) { $env:OPENSSL_INCLUDE_DIR } else { Join-Path $root 'include' }
  $libraries = @($env:OPENSSL_LIB_DIR, (Join-Path $root "lib/VC/$architecture/MD"), (Join-Path $root 'lib'))
  $lib = $libraries | Where-Object {
    $_ -and (Test-Path -LiteralPath (Join-Path $_ 'libcrypto.lib'))
  } | Select-Object -First 1
  if (-not $lib -or -not (Test-Path -LiteralPath (Join-Path $include 'openssl/ssl.h'))) {
    throw "OpenSSL import libraries/headers were not found under $root. Configure OPENSSL_LIB_DIR and OPENSSL_INCLUDE_DIR."
  }
  $bin = Join-Path $root 'bin'
  $runtime = Get-ChildItem -LiteralPath $bin -Filter 'libcrypto-*.dll' -File |
    Where-Object { (Get-WindowsPeArchitecture -Path $_.FullName) -eq $architecture } |
    Select-Object -First 1
  if (-not $runtime) { throw "A matching $architecture OpenSSL runtime was not found in $bin." }
  $env:OPENSSL_DIR = $root
  $env:OPENSSL_INCLUDE_DIR = $include
  $env:OPENSSL_LIB_DIR = $lib
  $env:LIB = "$lib;$env:LIB"
  $env:PATH = "$bin;$env:PATH"
}

function Copy-Runtime([string]$Source, [string]$Destination, [string]$Architecture) {
  if ((Get-WindowsPeArchitecture -Path $Source) -ne $Architecture) {
    throw "The runtime architecture does not match the executable: $Source"
  }
  if (Test-Path -LiteralPath $Destination -PathType Leaf) {
    if ((Get-RuntimeHash $Source) -eq (Get-RuntimeHash $Destination)) { return }
  }
  Copy-Item -LiteralPath $Source -Destination $Destination -Force
}

function Stage-Runtimes([string]$Executable) {
  $architecture = Get-WindowsPeArchitecture -Path $Executable
  if ($architecture -notin @('x64', 'arm64')) { throw 'Cargo produced an unsupported Windows executable.' }
  $destination = Split-Path $Executable
  $moshArchitecture = if ($architecture -eq 'x64') { 'x86_64' } else { 'arm64' }
  $relativeMosh = "third_party/nauterm_mosh_ffi/windows-$moshArchitecture/nauterm_mosh_ffi.dll"
  $mosh = Join-Path $project $relativeMosh
  $expected = $null
  foreach ($line in (Get-Content -LiteralPath (Join-Path $project 'third_party/nauterm_mosh_ffi/SHA256SUMS'))) {
    if ($line -match '^([a-fA-F0-9]{64})\s+\*?(.+)$' -and $Matches[2] -eq $relativeMosh) {
      $expected = $Matches[1]
      break
    }
  }
  if (-not $expected -or -not (Test-Path -LiteralPath $mosh -PathType Leaf) -or
      ((Get-Item -LiteralPath $mosh).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or
      (Get-RuntimeHash $mosh) -ne $expected) {
    throw 'The pinned Mosh runtime is missing, invalid, or has a checksum mismatch.'
  }
  Copy-Runtime $mosh (Join-Path $destination 'nauterm_mosh_ffi.dll') $architecture
  foreach ($runtime in (Get-ChildItem -LiteralPath (Join-Path $env:OPENSSL_DIR 'bin') -Filter 'lib*.dll' -File)) {
    if ((Get-WindowsPeArchitecture -Path $runtime.FullName) -eq $architecture) {
      Copy-Runtime $runtime.FullName (Join-Path $destination $runtime.Name) $architecture
    }
  }
  $crtNames = @('vcruntime140.dll', 'msvcp140.dll')
  if ($architecture -eq 'x64') { $crtNames += 'vcruntime140_1.dll' }
  $crt = Find-WindowsVcRuntime -Architecture $architecture -DllNames $crtNames
  foreach ($name in $crtNames) {
    Copy-Runtime (Join-Path $crt $name) (Join-Path $destination $name) $architecture
  }
  if (-not (Test-Path -LiteralPath (Join-Path $destination 'ghostty-vt.dll'))) {
    throw 'The Ghostty runtime was not staged beside the executable by the native build.'
  }
}

Push-Location $project
try {
  $hostInfo = & rustc -vV
  if ($LASTEXITCODE -ne 0) { throw 'Rust was not found. Install the pinned toolchain with rustup.' }
  $target = ($hostInfo | Where-Object { $_ -match '^host: ' }) -replace '^host: ', ''
  if ($env:CARGO_BUILD_TARGET) { $target = $env:CARGO_BUILD_TARGET }
  if ($Mode -eq 'Build') {
    for ($index = 0; $index -lt $ExtraArguments.Count; $index++) {
      if ($ExtraArguments[$index] -eq '--target' -and $index + 1 -lt $ExtraArguments.Count) {
        $target = $ExtraArguments[$index + 1]
      } elseif ($ExtraArguments[$index] -like '--target=*') {
        $target = $ExtraArguments[$index].Substring(9)
      }
    }
  }
  if ($target -notin @('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')) {
    throw "Nauterm RS on Windows requires an MSVC Rust target; found $target."
  }
  Initialize-BuildEnvironment $target
  $buildArguments = @('build', '--manifest-path', (Join-Path $project 'Cargo.toml'), '-p', 'nauterm-rs', '--locked', '--message-format=json-render-diagnostics')
  if ($Mode -eq 'Build') { $buildArguments += $ExtraArguments }
  $lines = @(& cargo @buildArguments | ForEach-Object {
    # Cargo's rendered Rust diagnostics are carried inside JSON stdout. Keep
    # them visible while retaining the artifact messages for exact resolution.
    $event = $_ | ConvertFrom-Json
    if ($event.reason -eq 'compiler-message' -and $event.message.rendered) {
      [Console]::Error.Write($event.message.rendered)
    }
    $_
  })
  if ($LASTEXITCODE -ne 0) { throw "Cargo build failed with exit code $LASTEXITCODE." }
  if ($env:NAUTERM_RS_BUILD_LOG) { [IO.File]::WriteAllLines($env:NAUTERM_RS_BUILD_LOG, [string[]]$lines, $encoding) }
  $executables = @(
    foreach ($line in $lines) {
      $item = $line | ConvertFrom-Json
      if ($item.reason -eq 'compiler-artifact' -and $item.target.name -eq 'nauterm-rs' -and $item.executable) {
        $item.executable
      }
    }
  )
  if ($executables.Count -ne 1) { throw 'Cargo did not report exactly one Nauterm RS executable.' }
  $executable = $executables[0]
  Stage-Runtimes $executable
  if ($Mode -eq 'Run') {
    Write-ProgressMessage "Starting $executable"
    & $executable @ExtraArguments
    exit $LASTEXITCODE
  }
  Write-Output $executable
} finally {
  Pop-Location
}
