# MyBoot TESTLOG

Append one entry per test/fix run: date, command, exact result (paste key serial
lines), and any PROVEN/UNPROVEN status change. See CLAUDE.md §6.

## 2026-09-09 — added install.ps1 (Windows counterpart to install.sh)

User reported the one-line installer fails for Windows users, since
`curl ... | sudo sh` assumes POSIX `sh`/`sudo`/`efibootmgr`, none of which exist on
Windows. Added `install.ps1`: self-elevates via UAC (no `sudo`), finds the ESP with
`Get-Partition` (GptType `{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}`, the UEFI-spec ESP
GUID), mounts it to a spare drive letter only if not already mounted, downloads +
verifies the same GitHub release asset (`myboot.efi` + `.sha256`) as `install.sh`,
places `\EFI\MyBoot\BOOTX64.EFI` + default `config.toml`, and registers a firmware
boot entry with `bcdedit /copy {bootmgr}` + `bcdedit /set {fwbootmgr} displayorder`
(the Windows equivalent of `efibootmgr --create`; confirmed idempotent the same way
install.sh is — checks `bcdedit /enum firmware` for an existing "MyBoot" description
before creating another). Same env-var knobs as install.sh
(`MYBOOT_REPO`/`MYBOOT_TAG`/`MYBOOT_ESP`/`MYBOOT_NO_REGISTER`/`MYBOOT_MAKE_DEFAULT`).

This is installer tooling only — no Rust/`no_std` core code touched, no change to
the UEFI target build or the boot transaction logic. Not yet run on a real Windows
machine (I have no Windows host in this sandbox); NOT run against real hardware —
per CLAUDE.md §0.4, that step is the user's to do and confirm, on a machine and ESP
they name explicitly. Documented in README.md, INSTALL_GUIDE.md (new "Windows"
section), and DISTRIBUTION_GUIDE.md.
