# MyBoot Windows Installer UEFI Detection Bug Report and Patch Proposal

## Summary

While installing **MyBoot** on a UEFI-based Windows system, the Windows installer incorrectly reported:

```text
[fail]   not a UEFI system (PEFirmwareType=); MyBoot is UEFI-only.
```

The machine was in fact booted in UEFI mode. The system had a valid 200 MB EFI System Partition, Windows reported `BiosFirmwareType = Uefi`, and Secure Boot was disabled.

The installer failed before copying `BOOTX64.EFI` or registering the MyBoot firmware entry.

The issue was traced to the UEFI preflight check in `install.ps1`, which reads a registry value named `PEFirmwareType` and requires it to equal `2`. On the affected normal Windows installation, that registry value was absent, producing an empty value and a false negative.

A local patch replaced this registry-based check with Windows' `Get-ComputerInfo -Property BiosFirmwareType`, which correctly returned `Uefi`. After the change, the installer completed successfully.

---

## Repository

**Project:** MyBoot  
**Repository:** https://github.com/ctrl-routing-Mosesgarlic/Myboot

**Affected file:** `install.ps1`

**Affected area:** UEFI preflight detection

The installer documentation states that the Windows installer downloads `myboot.efi`, installs it to:

```text
<ESP>\EFI\MyBoot\BOOTX64.EFI
```

and registers a UEFI firmware boot entry using `bcdedit`.

---

## Environment and Initial Symptoms

The affected computer was a Windows system using UEFI firmware.

Initial firmware information showed:

```text
Firmware Boot Manager
Windows Boot Manager
EFI USB Device
Internal Hard Disk or Solid State Disk
```

The system also contained a 200 MB EFI System Partition.

The EFI partition was mounted as:

```text
S:
```

Initial inspection showed:

```text
S:\EFI\
    Boot
    Microsoft
    HP
```

There was no:

```text
S:\EFI\MyBoot
```

and therefore no MyBoot EFI executable.

The initial firmware configuration also contained no `MyBoot` entry.

---

## First Installation Attempt

The standard installer was executed from PowerShell.

The installer terminated with:

```text
[fail]   not a UEFI system (PEFirmwareType=); MyBoot is UEFI-only.
```

The empty value was suspicious because the system was already using UEFI.

Further checks were performed.

### Windows firmware-mode check

```powershell
(Get-ComputerInfo).BiosFirmwareType
```

returned:

```text
Uefi
```

### Secure Boot status

```powershell
Confirm-SecureBootUEFI
```

returned:

```text
False
```

Therefore:

- The computer was running in UEFI mode.
- Secure Boot was disabled.
- The failure was not caused by Secure Boot.
- The installer was rejecting a valid UEFI system.

---

## Root Cause

The original installer contained this preflight check:

```powershell
$fwType = (Get-ItemProperty `
    -Path 'HKLM:\SYSTEM\CurrentControlSet\Control' `
    -Name 'PEFirmwareType' `
    -ErrorAction SilentlyContinue).PEFirmwareType

if ($fwType -ne 2) {
    Die "not a UEFI system (PEFirmwareType=$fwType); MyBoot is UEFI-only."
}
```

The problem is that the installer treats the presence and value of the registry entry:

```text
HKLM\SYSTEM\CurrentControlSet\Control\PEFirmwareType
```

as the authoritative firmware-mode check.

On the affected normal Windows environment, the value was not available, so the expression evaluated to an empty value:

```text
PEFirmwareType=
```

The comparison:

```powershell
$fwType -ne 2
```

therefore evaluated as true and the installer stopped.

This happened before the installer reached the EFI partition installation and `bcdedit` registration stages.

---

## Why This Was a False Negative

The computer itself provided independent evidence that it was UEFI:

```powershell
(Get-ComputerInfo).BiosFirmwareType
```

returned:

```text
Uefi
```

The machine also had an EFI System Partition with the GPT type associated with the UEFI EFI System Partition.

The installer already contained code for identifying the EFI System Partition using:

```powershell
Get-Partition |
    Where-Object {
        $_.GptType -eq '{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}'
    }
```

Therefore, the failure was specifically in the firmware-mode preflight check, not in the machine's boot configuration.

---

# Patch

## Original code

```powershell
$fwType = (Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control' -Name 'PEFirmwareType' -ErrorAction SilentlyContinue).PEFirmwareType
if ($fwType -ne 2) { Die "not a UEFI system (PEFirmwareType=$fwType); MyBoot is UEFI-only." }
```

## Proposed replacement

```powershell
$firmwareType = (Get-ComputerInfo -Property BiosFirmwareType).BiosFirmwareType
if ($firmwareType -ne 'Uefi') {
    Die "not a UEFI system (BiosFirmwareType=$firmwareType); MyBoot is UEFI-only."
}
```

This keeps the existing installer behavior while replacing the unreliable detection mechanism.

---

## Why This Patch

The replacement uses a PowerShell-supported system information interface to query the firmware type directly.

It also makes the expected value explicit:

```text
Uefi
```

rather than relying on a numeric registry value:

```text
2
```

The resulting logic is easier to understand:

```powershell
$firmwareType = ...
if ($firmwareType -ne 'Uefi') {
    ...
}
```

This makes the failure message more useful as well.

---

# Validation

Before running the patched installer, the modified script was syntax-checked successfully:

```text
Syntax OK
```

The patched installer was then executed.

It successfully progressed through all installation stages:

```text
[myboot] using ESP: S:
[myboot] resolving myboot.efi from ctrl-routing-Mosesgarlic/Myboot (latest)
[myboot] downloading myboot.efi
[ ok ]   checksum verified
[myboot] created S:\EFI\MyBoot\config.toml
[ ok ]   MyBoot placed at S:\EFI\MyBoot (other loaders untouched)
[ ok ]   registered a 'MyBoot' firmware boot entry ({9f4321f6-effc-11ee-a21d-f0c63f66aed5}) on S:
[ ok ]   Done. Reboot and pick 'MyBoot' from the firmware boot menu (F12/F2/Esc, varies by OEM).
```

The resulting EFI directory was:

```text
S:\EFI\MyBoot
    BOOTX64.EFI
    config.toml
```

The installed EFI executable was:

```text
BOOTX64.EFI
268800 bytes
```

The firmware configuration then contained:

```text
description             MyBoot
device                  partition=S:
path                    \EFI\MyBoot\BOOTX64.EFI
```

The existing Windows Boot Manager remained present.

This demonstrates that the patched preflight check allowed the installer to proceed and that the remainder of the installation process worked correctly on the affected system.

---

# Before-and-After Behavior

## Before patch

```text
Windows is UEFI
        |
        v
Installer checks registry PEFirmwareType
        |
        v
Value unavailable / empty
        |
        v
Installer assumes BIOS/Legacy
        |
        v
Installation stops
        |
        v
No EFI\MyBoot directory
No BOOTX64.EFI
No MyBoot firmware entry
```

## After patch

```text
Windows is UEFI
        |
        v
Installer checks BiosFirmwareType
        |
        v
Returns "Uefi"
        |
        v
UEFI check passes
        |
        v
EFI System Partition detected
        |
        v
myboot.efi downloaded
        |
        v
SHA-256 verified
        |
        v
EFI\MyBoot\BOOTX64.EFI created
        |
        v
MyBoot firmware entry registered
```

---

# Safety / Compatibility Considerations

The patch does not change the actual MyBoot installation procedure.

It only changes the firmware-mode preflight check.

The following installer behavior remains unchanged:

- Existing Windows Boot Manager is not deleted.
- The EFI System Partition is not formatted.
- MyBoot is installed under its own `EFI\MyBoot` directory.
- The downloaded EFI binary is checksum-verified when a checksum asset is available.
- The MyBoot firmware entry is registered through `bcdedit`.
- Existing boot loaders remain available.

The affected test system also had Secure Boot disabled, so Secure Boot was not part of the failure being fixed.

---

# Recommended Additional Testing

Before merging the patch, the installer should ideally be tested on at least:

1. A normal Windows UEFI installation.
2. A Windows system using Legacy BIOS/CSM.
3. A UEFI system with Secure Boot enabled.
4. A UEFI system with the EFI System Partition mounted.
5. A UEFI system where the EFI System Partition is not assigned a drive letter.
6. A system where the EFI System Partition cannot be detected.

The important regression requirement is:

> A genuine UEFI Windows installation must not be rejected merely because the `PEFirmwareType` registry value is unavailable.

---

# Suggested Commit

## Commit message

```text
fix(windows): use reliable UEFI detection in installer
```

## Commit body

```text
Replace the PEFirmwareType registry lookup with
Get-ComputerInfo BiosFirmwareType detection.

On some normal Windows UEFI installations, the PEFirmwareType
registry value is unavailable, causing the installer to report
"not a UEFI system" even though Windows is booted in UEFI mode.

This prevented MyBoot from reaching the EFI installation and
firmware-entry registration stages.

The new check validates the firmware type as "Uefi" before
continuing with the existing installation flow.
```

---

# Suggested Pull Request

## PR title

```text
Fix false UEFI detection in Windows installer
```

## PR description

### Problem

The Windows installer can incorrectly reject a UEFI system with:

```text
[fail] not a UEFI system (PEFirmwareType=); MyBoot is UEFI-only.
```

This occurs when:

```text
HKLM\SYSTEM\CurrentControlSet\Control\PEFirmwareType
```

is unavailable in a normal Windows environment.

### Root cause

`install.ps1` currently uses the `PEFirmwareType` registry value as its firmware-mode check.

On an affected UEFI installation, the value was empty even though:

```powershell
(Get-ComputerInfo).BiosFirmwareType
```

returned:

```text
Uefi
```

### Fix

Replace the registry-based check with:

```powershell
$firmwareType = (Get-ComputerInfo -Property BiosFirmwareType).BiosFirmwareType
if ($firmwareType -ne 'Uefi') {
    Die "not a UEFI system (BiosFirmwareType=$firmwareType); MyBoot is UEFI-only."
}
```

### Validation

The patched installer was successfully tested on the affected UEFI Windows system.

Successful results included:

```text
[ ok ] checksum verified
[ ok ] MyBoot placed at S:\EFI\MyBoot
[ ok ] registered a 'MyBoot' firmware boot entry
[ ok ] Done.
```

The resulting files were:

```text
S:\EFI\MyBoot\BOOTX64.EFI
S:\EFI\MyBoot\config.toml
```

and the firmware entry pointed to:

```text
\EFI\MyBoot\BOOTX64.EFI
```

Windows Boot Manager remained untouched.

### Scope

This PR only changes UEFI firmware detection. The existing MyBoot installation, checksum verification, EFI placement, and firmware-entry registration logic are unchanged.

---

# Recommended Git Workflow

After creating your fork/branch:

```bash
git checkout -b fix/windows-uefi-detection
```

Edit `install.ps1`, make the two-line replacement above, then:

```bash
git diff -- install.ps1
```

Review the diff carefully.

Then:

```bash
git add install.ps1
git commit -m "fix(windows): use reliable UEFI detection in installer"
```

Push the branch:

```bash
git push -u origin fix/windows-uefi-detection
```

Then open the pull request against the original repository's `main` branch.

---

# Evidence

The original installer explicitly describes itself as installing `myboot.efi` into `EFI\MyBoot\BOOTX64.EFI` and registering a UEFI firmware entry. The repository currently contains the affected `PEFirmwareType` registry check in `install.ps1`. 

The local investigation confirmed that the affected system reported `Uefi` through `Get-ComputerInfo`, while the installer saw an empty `PEFirmwareType` value.

After replacing the check, the installer successfully downloaded and checksum-verified MyBoot, created the EFI files, and registered the firmware entry.

---

# Conclusion

The failure was not caused by the computer being configured for Legacy BIOS. It was an installer-side firmware detection problem.

The patched detection allowed the existing MyBoot installation process to operate normally without modifying or removing the Windows bootloader.

This patch is small, isolated, and directly addresses the observed failure while preserving the installer's existing safety behavior.
