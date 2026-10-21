# Test of the Windows package on a "clean" installation (block 13).
#
# Windows Sandbox is not available on the reference machine: the test
# runs in a blank temporary folder outside the repository, and the executable is
# launched with an environment reduced to what Windows strictly needs
# (SystemRoot, PATH = System32, TEMP, user profile), hence without
# TYRIAN_DATA, CARGO_*, RUST*, GODOT* or a development PATH.
#
# Steps:
#   1. extraction of the archive into <Temp>\ot-clean-<timestamp>\;
#   2. normal launch (title screen) for -Seconds s, capture of the
#      window by PrintWindow -> <Out>\windows-window.png, then shutdown.
#      If the session is locked (agent case), DWM does not composite the
#      OpenGL surface: the image shows the window (title bar) but a gray
#      render area. The proof of rendering is then step 2 bis;
#   2 bis. internal capture of the title screen by the game -> windows-menu.png;
#   3. launch with the game's capture tool on a golden master replay
#      (checks GDExtension DLL + data/ + simulation) -> <Out>\windows-combat.png
#      and, if the package contains upscaled\, <Out>\windows-combat-hd.png.
#
# Before these steps, two launches without display (--headless), only
# run with -HeadlessOnly (CI windows-latest: no GPU nor guaranteed
# interactive session):
#   1 bis. normal startup (title) stopped by --quit-after -> windows-headless-boot.log;
#   1 ter. strict replay of the golden master INSIDE the exported executable
#      (--replay-strict, tools/capture.gd): each tick compared to the full
#      C line (18 columns, seven state fingerprints) -> windows-replay-strict.log.
# Each launch fails on a non-zero return code or a "SCRIPT ERROR:" /
# "ERROR:" in its output (same rule as scripts/godot-checked.sh).
#
# Usage: powershell -ExecutionPolicy Bypass -File rewrite\scripts\test-package-windows.ps1 `
#           -Zip build\windows\OpenTyrian2000-Enhanced-v1.0-rc1-windows-x86_64.zip -Out docs\screenshots\export [-HeadlessOnly]
param(
    [Parameter(Mandatory = $true)][string]$Zip,
    [Parameter(Mandatory = $true)][string]$Out,
    [string]$Replay = '',
    [int]$Seconds = 10,
    [switch]$HeadlessOnly
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing, System.Windows.Forms

$Zip = (Resolve-Path $Zip).Path
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path $Out).Path
if (-not $Replay) { $Replay = Join-Path (Split-Path -Parent $PSScriptRoot) 'golden\e1-tyrian-autopilot.gm' }

$root = Join-Path ([System.IO.Path]::GetTempPath()) ("ot-clean-" + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $root | Out-Null
Expand-Archive -Path $Zip -DestinationPath $root
$game = Join-Path $root 'OpenTyrian2000-Enhanced'
$exe = Join-Path $game 'OpenTyrian2000-Enhanced.exe'
Copy-Item $Replay (Join-Path $root 'replay.gm')
Write-Host "package extracted to $game"

# Minimal environment: dedicated profile (Godot's user:// lives under APPDATA).
$profileDir = Join-Path $root 'profile'
New-Item -ItemType Directory -Force -Path (Join-Path $profileDir 'AppData\Roaming'), (Join-Path $profileDir 'AppData\Local'), (Join-Path $profileDir 'Temp') | Out-Null
function New-CleanStartInfo([string]$arguments) {
    # Player data (userdata/ next to the exe by default): in the dedicated profile.
    $userDir = (Join-Path $profileDir 'userdata') -replace '\\', '/'
    if ($arguments -match '(^| )--( |$)') { $arguments = $arguments -replace '(^| )--( |$)', "`$1-- --user-dir=$userDir`$2" }
    else { $arguments = ($arguments + " -- --user-dir=$userDir").Trim() }
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.Arguments = $arguments
    $psi.WorkingDirectory = $game
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.EnvironmentVariables.Clear()
    $psi.EnvironmentVariables['SystemRoot'] = $env:SystemRoot
    $psi.EnvironmentVariables['windir'] = $env:windir
    $psi.EnvironmentVariables['SystemDrive'] = $env:SystemDrive
    $psi.EnvironmentVariables['PATH'] = "$env:SystemRoot\System32;$env:SystemRoot"
    $psi.EnvironmentVariables['USERPROFILE'] = $profileDir
    $psi.EnvironmentVariables['APPDATA'] = Join-Path $profileDir 'AppData\Roaming'
    $psi.EnvironmentVariables['LOCALAPPDATA'] = Join-Path $profileDir 'AppData\Local'
    $psi.EnvironmentVariables['TEMP'] = Join-Path $profileDir 'Temp'
    $psi.EnvironmentVariables['TMP'] = Join-Path $profileDir 'Temp'
    $psi.EnvironmentVariables['COMPUTERNAME'] = $env:COMPUTERNAME
    $psi.EnvironmentVariables['USERNAME'] = $env:USERNAME
    return $psi
}

function Invoke-Game([string]$arguments, [string]$log, [int]$timeoutSec) {
    $p = [System.Diagnostics.Process]::Start((New-CleanStartInfo $arguments))
    $stdout = $p.StandardOutput.ReadToEndAsync()
    $stderr = $p.StandardError.ReadToEndAsync()
    if (-not $p.WaitForExit($timeoutSec * 1000)) { $p.Kill(); throw "timeout: $arguments" }
    Set-Content -Path $log -Value ($stdout.Result + $stderr.Result)
    return $p.ExitCode
}

# Godot may report SCRIPT ERROR and return zero (scripts/godot-checked.sh).
function Assert-CleanRun([int]$code, [string]$log, [string]$what) {
    $bad = Select-String -Path $log -Pattern '(SCRIPT ERROR:|(^|\s)ERROR:)' | Select-Object -First 5
    if ($code -ne 0 -or $bad) {
        Get-Content $log | Select-Object -Last 40 | Write-Host
        throw "$what failed (code $code) $($bad -join ' | ')"
    }
}

# 1 bis. Startup without display: title then shutdown after 300 frames.
$bootLog = Join-Path $Out 'windows-headless-boot.log'
$code = Invoke-Game '--headless --audio-driver Dummy --quit-after 300' $bootLog 120
Assert-CleanRun $code $bootLog '--headless startup'
Write-Host "--headless startup: code $code, no error"

# 1 ter. Strict replay of the golden master in the exported executable.
$replayGm = Join-Path $root 'replay.gm'
$strictLog = Join-Path $Out 'windows-replay-strict.log'
$strictPng = Join-Path $Out 'windows-replay-strict.png'
$code = Invoke-Game "--headless --audio-driver Dummy -- --capture=level:$($strictPng -replace '\\','/'):1000000 --replay=$($replayGm -replace '\\','/') --replay-strict" $strictLog 300
Assert-CleanRun $code $strictLog 'strict replay --headless'
$verdict = Select-String -Path $strictLog -Pattern 'strict replay: \d+ ticks identical'
if (-not $verdict) { throw "strict replay: verdict missing from $strictLog" }
Write-Host $verdict.Line
if ($HeadlessOnly) {
    Write-Host "OK (no display): Windows package launched from $game"
    exit 0
}

# 2. Normal launch + capture of the screen around the window.
$p = [System.Diagnostics.Process]::Start((New-CleanStartInfo ''))
$stdoutTask = $p.StandardOutput.ReadToEndAsync()
Start-Sleep -Seconds $Seconds
$p.Refresh()
if ($p.HasExited) { throw "the game stopped at launch (code $($p.ExitCode)): $($stdoutTask.Result)" }
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Win32Wnd {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
}
'@
[Win32Wnd]::SetProcessDPIAware() | Out-Null
$h = $p.MainWindowHandle
$titlePng = Join-Path $Out 'windows-window.png'
if ($h -ne [IntPtr]::Zero) {
    [Win32Wnd]::SetForegroundWindow($h) | Out-Null
    Start-Sleep -Milliseconds 800
    $r = New-Object Win32Wnd+RECT
    [Win32Wnd]::GetWindowRect($h, [ref]$r) | Out-Null
    $w = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
    $bmp = New-Object System.Drawing.Bitmap $w, $hh
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    # PrintWindow (PW_RENDERFULLCONTENT = 2): window content even if
    # it is covered or the session is locked (CopyFromScreen
    # would then capture the lock screen).
    $hdc = $g.GetHdc()
    $ok = [Win32Wnd]::PrintWindow($h, $hdc, 2)
    $g.ReleaseHdc($hdc)
    if (-not $ok) { $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size) }
    $bmp.Save($titlePng, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "window capture ($w x $hh) -> $titlePng"
} else {
    Write-Host "window not found (null MainWindowHandle): no screen capture"
}
$p.Kill()
$p.WaitForExit()
Set-Content -Path (Join-Path $Out 'windows-run.log') -Value $stdoutTask.Result

# 2 bis. Internal capture of the title screen (image rendered by the game itself).
$menu = Join-Path $Out 'windows-menu.png'
$code = Invoke-Game "-- --capture=menu:$($menu -replace '\\','/'):30" (Join-Path $Out 'windows-menu.log') 120
Assert-CleanRun $code (Join-Path $Out 'windows-menu.log') 'menu capture'
if (-not (Test-Path $menu)) { throw "menu capture missing" }

# 3. Replay of a golden master with the internal capture tool.
$combat = Join-Path $Out 'windows-combat.png'
$code = Invoke-Game "-- --capture=level:$($combat -replace '\\','/'):4221 --replay=$($replayGm -replace '\\','/')" (Join-Path $Out 'windows-combat.log') 300
Get-Content (Join-Path $Out 'windows-combat.log') | Select-String '^capture'
Assert-CleanRun $code (Join-Path $Out 'windows-combat.log') 'in-game capture'
if (-not (Test-Path $combat)) { throw "in-game capture missing" }
if (Test-Path (Join-Path $game 'upscaled')) {
    $hd = Join-Path $Out 'windows-combat-hd.png'
    $code = Invoke-Game "-- --capture=level:$($hd -replace '\\','/'):4221 --replay=$($replayGm -replace '\\','/') --hd-sprites" (Join-Path $Out 'windows-combat-hd.log') 300
    Get-Content (Join-Path $Out 'windows-combat-hd.log') | Select-String '^capture'
    Assert-CleanRun $code (Join-Path $Out 'windows-combat-hd.log') 'HD capture'
    if (-not (Test-Path $hd)) { throw "HD capture missing" }
}
Write-Host "OK: Windows package launched from $game"
