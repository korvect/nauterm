$ErrorActionPreference = 'Stop'

# Ghostty's MSVC build needs the Visual Studio and Windows SDK environment
# variables that Cargo's linker setup does not export to Zig. Import the same
# environment used by the Visual Studio developer prompt into this job.
$vswhereCandidates = @(
    (Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'),
    (Join-Path $env:ProgramFiles 'Microsoft Visual Studio\Installer\vswhere.exe')
)
$vswhere = $vswhereCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $vswhere) {
    throw 'Visual Studio vswhere.exe was not found on the Windows runner.'
}

$installationPath = & $vswhere -latest -products * -property installationPath |
    Select-Object -First 1
if (-not $installationPath) {
    throw 'No Visual Studio installation was found on the Windows runner.'
}

$developerCommand = Join-Path $installationPath 'Common7\Tools\VsDevCmd.bat'
if (-not (Test-Path $developerCommand)) {
    throw "Visual Studio developer command not found: $developerCommand"
}

$targetArch = if ($env:NAUTERM_WINDOWS_ARCH -eq 'arm64') { 'arm64' } else { 'x64' }
$hostArch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
$command = '"{0}" -arch={1} -host_arch={2} >nul && set' -f $developerCommand, $targetArch, $hostArch
$environmentDump = & cmd.exe /d /s /c $command
if ($LASTEXITCODE -ne 0) {
    throw "Failed to initialize the Visual Studio developer environment for $targetArch."
}

$exportedNames = @(
    'INCLUDE',
    'LIB',
    'LIBPATH',
    'PATH',
    'VCINSTALLDIR',
    'VCToolsInstallDir',
    'VCToolsVersion',
    'WindowsSdkDir',
    'WindowsSdkVerBinPath',
    'UCRTVersion',
    'UniversalCRTSdkDir',
    'VSINSTALLDIR',
    'VisualStudioVersion',
    'VSCMD_ARG_app_plat',
    'VSCMD_ARG_HOST_ARCH',
    'VSCMD_ARG_TGT_ARCH',
    'VSCMD_VER'
)

$values = @{}
foreach ($line in $environmentDump) {
    if ($line -match '^([^=]+)=(.*)$') {
        $name = $Matches[1]
        if ($exportedNames -contains $name) {
            $values[$name] = $Matches[2]
        }
    }
}

foreach ($name in $exportedNames) {
    if ($values.ContainsKey($name)) {
        [Environment]::SetEnvironmentVariable($name, $values[$name], 'Process')
        Add-Content -Path $env:GITHUB_ENV -Value "$name=$($values[$name])"
    }
}

Write-Host "Configured Visual Studio MSVC environment for $targetArch (host: $hostArch)."
Write-Host "WindowsSdkDir=$env:WindowsSdkDir"
Write-Host "VCToolsInstallDir=$env:VCToolsInstallDir"
