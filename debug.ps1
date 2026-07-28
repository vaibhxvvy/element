param(
    [switch]$Release = $false,
    [switch]$Build = $false,
    [switch]$Monitor = $true
)

$logPath = "$env:USERPROFILE\.element\debug.log"
$elementExe = if ($Release) { ".\target\release\element.exe" } else { ".\target\debug\element.exe" }
$configPath = "$env:USERPROFILE\.element\config.toml"

function Write-Header {
    Clear-Host
    Write-Host "╔════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║           Element Launcher — Debug Monitor v2            ║" -ForegroundColor Cyan
    Write-Host "╚════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""
}

# Build if requested
if ($Build) {
    Write-Header
    Write-Host "Building Element..." -ForegroundColor Green
    cargo build $(if ($Release) { "--release" }) --message-format short
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Build FAILED with exit code $LASTEXITCODE" -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Host "Build SUCCESS" -ForegroundColor Green
    Write-Host ""
}

# Kill any running element process
$existing = Get-Process -Name "element" -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "Killing existing Element process(es)..." -ForegroundColor Yellow
    $existing | Stop-Process -Force
    Start-Sleep -Seconds 2
    $stillRunning = Get-Process -Name "element" -ErrorAction SilentlyContinue
    if ($stillRunning) {
        Write-Host "Process still running, trying harder..." -ForegroundColor Red
        $stillRunning | Stop-Process -Force
        Start-Sleep -Seconds 1
    }
}

# Clear old log
if (Test-Path $logPath) {
    Remove-Item $logPath -Force
    Write-Host "Cleared old debug.log" -ForegroundColor Gray
}

if (-not (Test-Path $elementExe)) {
    Write-Host "ERROR: Element executable not found at: $elementExe" -ForegroundColor Red
    Write-Host "Run with -Build flag first, or specify -Release if using release build" -ForegroundColor Yellow
    exit 1
}

if (-not $Monitor) {
    # Just launch without monitoring
    Write-Host "Starting Element (background)..." -ForegroundColor Green
    Start-Process -FilePath $elementExe -WindowStyle Hidden
    Write-Host "Done. Element is running." -ForegroundColor Green
    exit 0
}

# Check config
Write-Host ""
Write-Host "── System Checks ──────────────────────────────────────" -ForegroundColor DarkGray
if (Test-Path $configPath) {
    Write-Host "Config: $configPath (exists)" -ForegroundColor Green
} else {
    Write-Host "Config: $configPath (will be created with defaults)" -ForegroundColor Yellow
}

# Check for potential Alt+Space conflicts
Write-Host ""
Write-Host "── Hotkey Conflict Check ──────────────────────────────" -ForegroundColor DarkGray
$conflictingApps = @(
    @{Name="PowerToys"; Process="PowerToys.exe"},
    @{Name="Microsoft Teams"; Process="Teams.exe"},
    @{Name="Spotify"; Process="Spotify.exe"},
    @{Name="AutoHotkey"; Process="AutoHotkey.exe"},
    @{Name="Ditto Clipboard"; Process="Ditto.exe"},
    @{Name="Launchy"; Process="Launchy.exe"},
    @{Name="Keypirinha"; Process="Keypirinha.exe"},
    @{Name="Wox"; Process="Wox.exe"},
    @{Name="Flow Launcher"; Process="Flow.Launcher.exe"},
    @{Name="Listary"; Process="Listary.exe"},
    @{Name="uTools"; Process="uTools.exe"}
)
$foundConflict = $false
foreach ($app in $conflictingApps) {
    $proc = Get-Process -Name ($app.Process -replace '\.exe$','') -ErrorAction SilentlyContinue
    if ($proc) {
        Write-Host "WARNING: $($app.Name) ($($app.Process)) is RUNNING — may conflict with Alt+Space" -ForegroundColor Red
        $foundConflict = $true
    }
}
if (-not $foundConflict) {
    Write-Host "No known conflicting apps detected." -ForegroundColor Green
}

# Start Element
Write-Host ""
Write-Host "── Starting Element ───────────────────────────────────" -ForegroundColor DarkGray
Write-Host "Launching: $elementExe" -ForegroundColor Green
$proc = Start-Process -FilePath $elementExe -WindowStyle Hidden -PassThru
Write-Host "PID: $($proc.Id)" -ForegroundColor Green

Write-Host ""
Write-Host "Waiting for debug log..." -ForegroundColor Gray

# Wait for log to appear
$maxWait = 30
$waited = 0
while (-not (Test-Path $logPath) -and $waited -lt $maxWait) {
    Start-Sleep -Milliseconds 500
    $waited++
}

if (-not (Test-Path $logPath)) {
    Write-Host "Log file not found at: $logPath after ${maxWait}s" -ForegroundColor Red
    Write-Host "Possible issues:" -ForegroundColor Yellow
    Write-Host "  1. The app crashed before writing the log" -ForegroundColor Yellow
    Write-Host "  2. The app could not create the .element directory" -ForegroundColor Yellow
    Write-Host "  3. Anti-virus is blocking file writes" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Press any key to exit..." -ForegroundColor Yellow
    $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
    exit 1
}

Write-Host ""
Write-Host "╔════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║                    LIVE DEBUG MONITOR                     ║" -ForegroundColor Cyan
Write-Host "╠════════════════════════════════════════════════════════════╣" -ForegroundColor Cyan
Write-Host "║  Press Alt+Space and watch the log below                  ║" -ForegroundColor Cyan
Write-Host "║  Each event is timestamped and labeled for diagnosis      ║" -ForegroundColor Cyan
Write-Host "║  Ctrl+C to exit                                           ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# Read the first few lines to show start state
Start-Sleep -Milliseconds 500
Write-Host "── Initial log output ─────────────────────────────────" -ForegroundColor DarkGray
Get-Content $logPath -Tail 10
Write-Host "───────────────────────────────────────────────────────" -ForegroundColor DarkGray
Write-Host ""
Write-Host "Monitoring for new entries..." -ForegroundColor Green

# Monitor the log
$lastLength = 0
$maxWaitForHotkey = 120  # 2 minutes
$hotkeyWaitCount = 0

try {
    Get-Content $logPath -Wait -Tail 20
} catch {
    Write-Host "Monitor stopped: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "Last 50 lines of debug log:" -ForegroundColor Yellow
    if (Test-Path $logPath) {
        Get-Content $logPath -Tail 50
    }
}

# If we get here (after Ctrl+C or error), show summary
Write-Host ""
Write-Host "── Session Summary ───────────────────────────────────" -ForegroundColor DarkGray
if (Test-Path $logPath) {
    $lines = Get-Content $logPath
    $hotkeyEvents = $lines | Select-String "WM_HOTKEY" | Measure-Object | Select-Object -ExpandProperty Count
    $showEvents = $lines | Select-String "show_launcher" | Measure-Object | Select-Object -ExpandProperty Count
    $hideEvents = $lines | Select-String "hide_launcher" | Measure-Object | Select-Object -ExpandProperty Count
    $errors = $lines | Select-String "(FAILED|CRITICAL|failed)" | Measure-Object | Select-Object -ExpandProperty Count
    Write-Host "Total log lines: $($lines.Count)" -ForegroundColor White
    Write-Host "Alt+Space presses: $hotkeyEvents" -ForegroundColor White
    Write-Host "Window shows: $showEvents" -ForegroundColor White
    Write-Host "Window hides: $hideEvents" -ForegroundColor White
    Write-Host "Errors/Warnings: $errors" -ForegroundColor $(if ($errors -gt 0) { "Red" } else { "Green" })
}