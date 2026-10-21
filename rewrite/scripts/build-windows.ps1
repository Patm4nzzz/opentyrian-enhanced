# Full build on the Windows side (PowerShell):
#   1. GDExtension tyrian-godot (cargo, release, MSVC target) -> godot\bin\tyrian_godot.dll
#   2. Resources: tyrian-import data\ -> godot\assets\imported\, then the music
#      (render.sh: music\ DOSBox for Classic, music_nuked\ Nuked OPL3 for Authentic)
#   3. Godot import (textures, sounds), then level scenes (editor\build_levels.gd)
#
# Prerequisites: bash, gcc, SDL2, ffmpeg and python3 in the PATH (music rendering, like
# build-linux.sh; otherwise go through WSL with rewrite/scripts/package.sh), rustup + toolchain stable-x86_64-pc-windows-msvc (Build Tools
# for Visual Studio, "Desktop development with C++" component), Godot 4.7 for Windows.
# Usage: powershell -ExecutionPolicy Bypass -File rewrite\scripts\build-windows.ps1 [-Godot C:\path\Godot_v4.7.2-stable_win64_console.exe] [-SkipImport]
param(
    [string]$Godot = $(if ($env:GODOT) { $env:GODOT } else { 'godot' }),
    [switch]$SkipImport
)
$ErrorActionPreference = 'Stop'

$rewrite = Split-Path -Parent $PSScriptRoot
$repo = Split-Path -Parent $rewrite
$godotDir = Join-Path $rewrite 'godot'
$target = 'x86_64-pc-windows-msvc'
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $rewrite 'target' }

function Invoke-Checked([string]$exe, [string[]]$arguments, [switch]$CheckGodot) {
    # Capture the native exit code immediately, even under Windows PowerShell 5.1.
    # Native stderr must not short-circuit its collection through ErrorActionPreference.
    $resolvedExe = (Get-Command $exe -ErrorAction Stop).Source
    $previous = $ErrorActionPreference
    $global:LASTEXITCODE = $null
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& $resolvedExe @arguments 2>&1)
        $exitCode = $LASTEXITCODE
    }
    finally { $ErrorActionPreference = $previous }
    $output | ForEach-Object { Write-Host "$_" }
    if ($exitCode -ne 0) { throw "$exe $($arguments -join ' '): code $exitCode" }
    if ($CheckGodot -and ($output -match 'SCRIPT ERROR:|(^|\s)ERROR:')) {
        throw "Godot: error detected despite a zero native exit code"
    }
}

Push-Location $rewrite
try {
    Write-Host '== 1. GDExtension (windows, MSVC)'
    Invoke-Checked 'cargo' @('build', '--release', '-p', 'tyrian-godot', '-p', 'tyrian-import', '--target', $target)
    New-Item -ItemType Directory -Force -Path (Join-Path $godotDir 'bin') | Out-Null
    Copy-Item (Join-Path $targetDir "$target\release\tyrian_godot.dll") (Join-Path $godotDir 'bin') -Force

    if (-not $SkipImport) {
        Write-Host '== 2. tyrian-import'
        Invoke-Checked (Join-Path $targetDir "$target\release\tyrian-import.exe") @('--data', (Join-Path $repo 'data'), '--out', (Join-Path $godotDir 'assets\imported'))

        # Music: without it the game is silent (Setup > Sound). Same rendering as build-linux.sh.
        $musicRender = Join-Path $rewrite 'tools\music-render\render.sh'
        $imported = Join-Path $godotDir 'assets\imported'
        $engines = @(@{ Dir = 'music'; Backend = 'dosbox' }, @{ Dir = 'music_nuked'; Backend = 'nuked' })
        foreach ($engine in $engines) {
            $json = Join-Path $imported "$($engine.Dir)\music.json"
            if (Test-Path $json) { continue }
            Write-Host "== 2b. music $($engine.Dir) (OPL rendering $($engine.Backend))"
            $env:MUSIC_BACKEND = $engine.Backend
            try { Invoke-Checked 'bash' @($musicRender, (Join-Path $repo 'data'), (Join-Path $imported $engine.Dir)) }
            finally { Remove-Item Env:\MUSIC_BACKEND -ErrorAction SilentlyContinue }
            if (-not (Test-Path $json)) { throw "music $($engine.Dir) missing after rendering: $json" }
        }
    }

    Write-Host '== 3. Godot import + level scenes'
    Invoke-Checked $Godot @('--headless', '--path', $godotDir, '--import') -CheckGodot
    Invoke-Checked $Godot @('--headless', '--path', $godotDir, 'res://editor/build_levels.tscn') -CheckGodot
    Invoke-Checked $Godot @('--headless', '--path', $godotDir, '--import') -CheckGodot

    Write-Host "OK: run `"$Godot --path $godotDir`""
}
finally {
    Pop-Location
}
