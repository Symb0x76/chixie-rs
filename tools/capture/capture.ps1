<#
.SYNOPSIS
  Capture the exact G-code ChiXie3 streams for a text, without hardware.

.DESCRIPTION
  1. Starts `sast-writer-cli fake-grbl` on 127.0.0.1:23 (a fake Grbl that records every line).
  2. Optionally patches ChiXie3's config.ini (backed up and restored afterwards).
  3. Starts a fresh ChiXie3 un-elevated (__COMPAT_LAYER=RunAsInvoker, no UAC prompt).
  4. Connects ChiXie3's wireless device "[自定义] guangku" (127.0.0.1:23, add it once by hand:
     设备管理 → 无线 → + → IP 127.0.0.1, port 23).
  5. Pastes -Text into the first cell of the current template, presses ▶, waits for the end
     marker `G90G0 X0Y0`, then quits ChiXie3 (forcefully if its Telnet thread hangs).

  Requirements: ChiXie3 logged in (auto login), maximized on the primary screen, 书写 tab, device
  pane closed. Click positions are for a 3840×2160 screen at 200% (same layout as 1920×1080 at
  100%); re-measure with -Shots if the layout differs.

  Input goes only to ChiXie3: every click/keystroke checks that a ChiXie3 window is in front.
  Keep the mouse and keyboard still while it runs (~40 s). ChiXie3 stalls on long Telnet jobs, so
  keep texts short (< ~200 G-code lines, a dozen characters).

.EXAMPLE
  ./tools/capture/capture.ps1 -Text "你好A1" -Out captures/nihao.gcode
  ./tools/capture/capture.ps1 -Text "你好" -Out captures/d7.gcode -Config @{ 'CharacterDeformation.EnableCharacterDeformation' = 'true'; 'CharacterDeformation.shape_deform_degree' = '7' }
#>
param(
    [Parameter(Mandatory)] [string]$Text,
    [Parameter(Mandatory)] [string]$Out,
    # 'Section.key' = 'value' overrides for %USERPROFILE%\ChiXie3\conf\config.ini (restored afterwards)
    [hashtable]$Config = @{},
    [int]$TimeoutSec = 90,
    # Save a screenshot after each UI step into <Out>.shots/ (for re-measuring click positions)
    [switch]$Shots
)
$ErrorActionPreference = 'Stop'
$repo = Resolve-Path "$PSScriptRoot\..\.."
$cli = Join-Path $repo 'target\debug\sast-writer-cli.exe'
$chixie = Join-Path $env:LOCALAPPDATA 'Programs\ChiXie3\start.exe'
$ini = Join-Path $env:USERPROFILE 'ChiXie3\conf\config.ini'
$Out = [IO.Path]::GetFullPath((Join-Path (Get-Location) $Out))
$shotDir = "$Out.shots"
if (-not (Test-Path $cli)) { throw "build first: cargo build -p sast-writer-cli" }
if (-not (Test-Path $chixie)) { throw "ChiXie3 not found at $chixie" }
if (Get-Process start -ErrorAction SilentlyContinue) { throw "close ChiXie3 first" }
if ($Shots) { New-Item -ItemType Directory -Force $shotDir | Out-Null }

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class CapUi {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
}
"@
[CapUi]::SetProcessDPIAware() | Out-Null
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$scale = $screen.Width / 1600.0   # click positions below are in a 1600-wide frame of the screen

function Assert-ChiXieFront {
    $fg = [CapUi]::GetForegroundWindow(); $fpid = 0
    [CapUi]::GetWindowThreadProcessId($fg, [ref]$fpid) | Out-Null
    if (-not (@(Get-Process start -ErrorAction SilentlyContinue).Id -contains [int]$fpid)) {
        $main = Get-Process start | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
        [CapUi]::SetForegroundWindow($main.MainWindowHandle) | Out-Null
        Start-Sleep -Milliseconds 400
        $fg = [CapUi]::GetForegroundWindow(); [CapUi]::GetWindowThreadProcessId($fg, [ref]$fpid) | Out-Null
        if (-not (@(Get-Process start).Id -contains [int]$fpid)) { throw "ChiXie3 is not in front; refusing input" }
    }
}
function Shot([string]$name) {
    if (-not $Shots) { return }
    $bmp = New-Object System.Drawing.Bitmap $screen.Width, $screen.Height
    [System.Drawing.Graphics]::FromImage($bmp).CopyFromScreen($screen.Left, $screen.Top, 0, 0, $bmp.Size)
    $bmp.Save("$shotDir\$name.png", [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
}
function Click([string]$name, [double]$x, [double]$y, [int]$wait = 1000) {
    Assert-ChiXieFront
    [CapUi]::SetCursorPos([int]($screen.Left + $x * $scale), [int]($screen.Top + $y * $scale)) | Out-Null
    Start-Sleep -Milliseconds 150
    [CapUi]::mouse_event(2, 0, 0, 0, [IntPtr]::Zero); [CapUi]::mouse_event(4, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds $wait
    Shot $name
}
function Keys([string]$name, [string]$keys, [int]$wait = 1000) {
    Assert-ChiXieFront
    [System.Windows.Forms.SendKeys]::SendWait($keys)
    Start-Sleep -Milliseconds $wait
    Shot $name
}
function Set-IniValues([string]$path, [hashtable]$values) {
    $lines = [System.Collections.Generic.List[string]](Get-Content -LiteralPath $path -Encoding UTF8)
    foreach ($k in $values.Keys) {
        $sec, $key = $k.Split('.', 2)
        $start = $lines.IndexOf("[$sec]")
        if ($start -lt 0) { $lines.Add("[$sec]"); $lines.Add("$key=$($values[$k])"); continue }
        $end = $start + 1
        while ($end -lt $lines.Count -and -not $lines[$end].StartsWith('[')) { $end++ }
        $hit = -1
        for ($i = $start + 1; $i -lt $end; $i++) { if ($lines[$i] -match "^$([regex]::Escape($key))=") { $hit = $i } }
        if ($hit -ge 0) { $lines[$hit] = "$key=$($values[$k])" } else { $lines.Insert($end, "$key=$($values[$k])") }
    }
    Set-Content -LiteralPath $path -Value $lines -Encoding UTF8
}

$backup = $null
$fake = $null
$errLog = "$Out.fake.log"
try {
    if ($Config.Count) {
        $backup = "$ini.capture-backup"
        Copy-Item -LiteralPath $ini -Destination $backup -Force
        Set-IniValues $ini $Config
    }
    $fake = Start-Process -FilePath $cli -ArgumentList 'fake-grbl', "`"$Out`"", '--idle', "$TimeoutSec" -PassThru -NoNewWindow -RedirectStandardError $errLog
    Start-Sleep -Milliseconds 800
    if ($fake.HasExited) { throw "fake-grbl failed: $(Get-Content $errLog -Raw)" }

    $env:__COMPAT_LAYER = 'RunAsInvoker'
    Start-Process -FilePath $chixie -WorkingDirectory (Split-Path $chixie)
    Remove-Item Env:__COMPAT_LAYER
    for ($i = 0; $i -lt 90 -and -not (Get-Process start -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -like '炽写(alpha*' }); $i++) { Start-Sleep 1 }
    Start-Sleep 10   # login, fonts and templates load after the window appears
    Shot '00-start'

    # Connect the wireless fake board.
    Click '01-device-pane' 40 860 2500
    Click '02-wireless-tab' 65 321 1500
    Click '03-select' 83 201 800
    Click '04-connect' 170 292 1000
    for ($i = 0; $i -lt 20 -and -not ((Get-Content $errLog -Raw) -match 'client'); $i++) { Start-Sleep -Milliseconds 500 }
    if (-not ((Get-Content $errLog -Raw) -match 'client')) { throw "ChiXie3 did not connect to the fake board" }

    # Type the text and start the job.
    Set-Clipboard -Value $Text
    Click '05-first-cell' 593 265 800
    Keys '06-paste' '^v' 2500
    Click '07-start' 897 60 1000

    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while (-not $fake.HasExited -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
    Shot '08-end'
    if (-not $fake.HasExited) { throw "no end marker within $TimeoutSec s (ChiXie3 Telnet stall?)" }
    if ($fake.ExitCode -ne 0) { throw "capture incomplete: $(Get-Content $errLog -Raw)" }
    $n = (Get-Content -LiteralPath $Out).Count
    Write-Output "captured $n lines -> $Out"
}
finally {
    if ($fake -and -not $fake.HasExited) { Stop-Process -Id $fake.Id -Force }
    $main = Get-Process start -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -like '炽写(alpha*' }
    if ($main) {
        $main.CloseMainWindow() | Out-Null
        Start-Sleep 3
        try { Click '09-quit-confirm' 774 432 4000 } catch { }   # 关闭确认 → 退出
    }
    # A wedged Telnet thread keeps ChiXie3 alive; it has nothing unsaved worth keeping here.
    Get-Process start -ErrorAction SilentlyContinue | Stop-Process -Force
    if ($backup) { Move-Item -LiteralPath $backup -Destination $ini -Force }
}
