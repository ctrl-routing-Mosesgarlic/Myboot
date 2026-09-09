#Requires -Version 5.1
<#
MyBoot one-line installer for Windows.

    irm https://raw.githubusercontent.com/ctrl-routing-Mosesgarlic/Myboot/main/install.ps1 | iex

Downloads the latest released myboot.efi from GitHub, verifies its SHA-256,
installs it to <ESP>\EFI\MyBoot\BOOTX64.EFI, writes a default config (only if
none exists), and registers a UEFI firmware boot entry via bcdedit. It NEVER
formats a disk or removes another OS's loader -- it only writes \EFI\MyBoot
and adds one firmware boot entry. This is the Windows counterpart to
install.sh (same env-var knobs, same "coexist, never destroy" contract).

Options via environment variables (same names as install.sh):
    MYBOOT_REPO            override the source repo (default below)
    MYBOOT_TAG             install a specific release (default: latest)
    MYBOOT_ESP             override ESP auto-detection, e.g. "S:"
    MYBOOT_NO_REGISTER=1   skip the bcdedit entry (just place the files)
    MYBOOT_MAKE_DEFAULT=1  put MyBoot first in the firmware display order
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$InstallScriptUrl = 'https://raw.githubusercontent.com/ctrl-routing-Mosesgarlic/Myboot/main/install.ps1'
$Repo   = if ($env:MYBOOT_REPO) { $env:MYBOOT_REPO } else { 'ctrl-routing-Mosesgarlic/Myboot' }
$Tag    = if ($env:MYBOOT_TAG)  { $env:MYBOOT_TAG }  else { 'latest' }
$Label  = 'MyBoot'
$SubDir = 'EFI\MyBoot'

function Say  ($m) { Write-Host "[myboot] $m" -ForegroundColor Cyan }
function Ok   ($m) { Write-Host "[ ok ]   $m" -ForegroundColor Green }
function Warn ($m) { Write-Host "[warn]   $m" -ForegroundColor Yellow }
function Die  ($m) { Write-Host "[fail]   $m" -ForegroundColor Red; exit 1 }

# --- must run elevated: self-relaunch with a UAC prompt if we aren't --------
$currentIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
$isAdmin = ([Security.Principal.WindowsPrincipal]$currentIdentity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Say "not running elevated; requesting Administrator (UAC prompt)..."
    if ($PSCommandPath) {
        Start-Process -FilePath 'powershell.exe' -Verb RunAs -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"")
    } else {
        # Invoked as `irm ... | iex`: there is no file on disk to relaunch, so
        # re-fetch this same script and re-run it elevated, in-process env vars forwarded.
        $prologue = @()
        foreach ($name in 'MYBOOT_REPO', 'MYBOOT_TAG', 'MYBOOT_ESP', 'MYBOOT_NO_REGISTER', 'MYBOOT_MAKE_DEFAULT') {
            $val = [Environment]::GetEnvironmentVariable($name)
            if ($val) { $prologue += "`$env:$name = '$val'" }
        }
        $body = Invoke-RestMethod -Uri $InstallScriptUrl -Headers @{ 'User-Agent' = 'myboot-install.ps1' }
        $tmp = Join-Path $env:TEMP 'myboot-install.ps1'
        Set-Content -Path $tmp -Value ($prologue + $body) -Encoding UTF8
        Start-Process -FilePath 'powershell.exe' -Verb RunAs -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$tmp`"")
    }
    exit 0
}

# --- preflight ----------------------------------------------------------------
# HKLM\SYSTEM\CurrentControlSet\Control\PEFirmwareType: 1 = legacy BIOS, 2 = UEFI
# (the registry value the Windows kernel's GetFirmwareType() reads; documented by
# Microsoft, e.g. KB pages on detecting BIOS vs UEFI without third-party tools).
$fwType = (Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control' -Name 'PEFirmwareType' -ErrorAction SilentlyContinue).PEFirmwareType
if ($fwType -ne 2) { Die "not a UEFI system (PEFirmwareType=$fwType); MyBoot is UEFI-only." }

# --- locate the ESP ------------------------------------------------------------
$Esp = $env:MYBOOT_ESP
$espPartition = $null
$mountedAccessPath = $null
if (-not $Esp) {
    # {c12a7328-f81f-11d2-ba4b-00a0c93ec93b} is the UEFI-spec EFI System Partition GUID.
    $espPartition = Get-Partition -ErrorAction SilentlyContinue | Where-Object { $_.GptType -eq '{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}' } | Select-Object -First 1
    if (-not $espPartition) { Die 'could not find the EFI System Partition; set $env:MYBOOT_ESP = "X:" (an already-mounted drive letter) and re-run.' }

    $existingPath = $espPartition.AccessPaths | Where-Object { $_ -match '^[A-Za-z]:\\$' } | Select-Object -First 1
    if ($existingPath) {
        $Esp = $existingPath.TrimEnd('\')
    } else {
        $letter = 68..90 | ForEach-Object { [char]$_ } | Where-Object { -not (Test-Path "${_}:\") } | Select-Object -First 1
        if (-not $letter) { Die 'no free drive letter to mount the ESP.' }
        $mountedAccessPath = "${letter}:\"
        Add-PartitionAccessPath -DiskNumber $espPartition.DiskNumber -PartitionNumber $espPartition.PartitionNumber -AccessPath $mountedAccessPath
        $Esp = "${letter}:"
        Say "mounted the ESP at $Esp (will unmount when done)"
    }
}
$Esp = $Esp.TrimEnd('\')
Say "using ESP: $Esp"

try {
    # --- resolve the release asset URL -----------------------------------------
    $apiBase = "https://api.github.com/repos/$Repo/releases"
    $metaUrl = if ($Tag -eq 'latest') { "$apiBase/latest" } else { "$apiBase/tags/$Tag" }
    Say "resolving myboot.efi from $Repo ($Tag)"
    try {
        $release = Invoke-RestMethod -Uri $metaUrl -Headers @{ 'User-Agent' = 'myboot-install.ps1' }
    } catch {
        Die "could not reach GitHub API for $Repo. $($_.Exception.Message)"
    }
    $efiAsset = $release.assets | Where-Object { $_.name -eq 'myboot.efi' } | Select-Object -First 1
    $shaAsset = $release.assets | Where-Object { $_.name -eq 'myboot.efi.sha256' } | Select-Object -First 1
    if (-not $efiAsset) { Die "release '$Tag' has no myboot.efi asset. Has a release been published with build artifacts?" }

    # --- download + verify -------------------------------------------------------
    $tmpDir = Join-Path $env:TEMP ([Guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Path $tmpDir | Out-Null
    try {
        $efiPath = Join-Path $tmpDir 'myboot.efi'
        Say 'downloading myboot.efi'
        Invoke-WebRequest -Uri $efiAsset.browser_download_url -OutFile $efiPath -Headers @{ 'User-Agent' = 'myboot-install.ps1' }

        if ($shaAsset) {
            $shaPath = Join-Path $tmpDir 'myboot.efi.sha256'
            Invoke-WebRequest -Uri $shaAsset.browser_download_url -OutFile $shaPath -Headers @{ 'User-Agent' = 'myboot-install.ps1' }
            $expect = (Get-Content $shaPath -Raw).Trim().Split(' ')[0].ToLowerInvariant()
            $actual = (Get-FileHash -Algorithm SHA256 -Path $efiPath).Hash.ToLowerInvariant()
            if ($expect -ne $actual) { Die "checksum mismatch (expected $expect, got $actual)." }
            Ok 'checksum verified'
        } else {
            Warn 'no checksum available; skipping verification.'
        }

        # --- place on the ESP (coexisting) ---------------------------------------
        $dest = Join-Path "$Esp\" $SubDir
        New-Item -ItemType Directory -Path $dest -Force | Out-Null
        Copy-Item -Path $efiPath -Destination (Join-Path $dest 'BOOTX64.EFI') -Force
        $cfgPath = Join-Path $dest 'config.toml'
        if (-not (Test-Path $cfgPath)) {
            @'
# MyBoot configuration (created by install.ps1; safe to edit).
default = "auto"
timeout_secs = 5
policy = "last-good-then-default"
confirm = true
max_tries = 3
rollback = "last-good"
'@ | Set-Content -Path $cfgPath -Encoding ascii
            Say "created $cfgPath"
        } else {
            Say 'config exists, left unchanged'
        }
        Ok "MyBoot placed at $dest (other loaders untouched)"

        # --- register the firmware entry -----------------------------------------
        if ($env:MYBOOT_NO_REGISTER -eq '1') {
            Say 'to make MyBoot selectable, register it manually:'
            Say '  bcdedit /copy {bootmgr} /d "MyBoot"'
            Say "  bcdedit /set <new-guid> device partition=$Esp"
            Say "  bcdedit /set <new-guid> path \$SubDir\BOOTX64.EFI"
            Say '  bcdedit /set {fwbootmgr} displayorder <new-guid> /addlast'
        } else {
            $enumOut = (& bcdedit /enum firmware) -join "`n"
            if ($enumOut -match "(?m)^description\s+$([Regex]::Escape($Label))\s*$") {
                Ok "a '$Label' firmware boot entry already exists; leaving it (updating the file is enough)"
            } else {
                $copyOut = & bcdedit /copy '{bootmgr}' /d $Label
                if ($LASTEXITCODE -ne 0) { Die "bcdedit /copy failed: $copyOut" }
                if (($copyOut -join ' ') -notmatch '(\{[0-9a-fA-F-]{36}\})') { Die "could not parse new boot entry GUID from: $copyOut" }
                $guid = $Matches[1]
                & bcdedit /set $guid device "partition=$Esp" | Out-Null
                & bcdedit /set $guid path "\$SubDir\BOOTX64.EFI" | Out-Null
                $addFlag = if ($env:MYBOOT_MAKE_DEFAULT -eq '1') { '/addfirst' } else { '/addlast' }
                & bcdedit /set '{fwbootmgr}' displayorder $guid $addFlag | Out-Null
                Ok "registered a '$Label' firmware boot entry ($guid) on $Esp"
            }
        }
    } finally {
        Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
} finally {
    if ($mountedAccessPath -and $espPartition) {
        Remove-PartitionAccessPath -DiskNumber $espPartition.DiskNumber -PartitionNumber $espPartition.PartitionNumber -AccessPath $mountedAccessPath -ErrorAction SilentlyContinue
        Say 'unmounted the temporary ESP drive letter'
    }
}

Ok "Done. Reboot and pick 'MyBoot' from the firmware boot menu (F12/F2/Esc, varies by OEM)."
Say "Your existing bootloader is untouched -- keep it as a fallback until MyBoot is proven."
