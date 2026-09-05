# TASKS.md — active work for Claude Code

Read `CLAUDE.md` first (the binding rules). These are the current tasks, in order.
Record every run in `TESTLOG.md`.

---

## Task 1 — Confirm the smoke-test fix (should already pass)

The smoke test previously failed with "never saw 'UEFI Interactive Shell'". Root
cause (confirmed against the code): the default-picked entry (Windows, healthiest
first) was a copy of `myboot.efi`, so MyBoot chainloaded **itself** recursively and
never reached the shell before the timeout ("7 handoff attempts" = 7 recursion
levels). Fix applied: `pyboot/esp.py` now backs the default-picked entry (Windows,
and Ubuntu) with the **real** loader (the UEFI shell), so the first handoff launches
a genuine, different app and stops.

Do:
```
./manage.py check
```
Expect the smoke test to PASS now (it should see "UEFI Interactive Shell" quickly).
If it still fails, capture the serial and diagnose — but do NOT revert the esp.py
change; the recursion diagnosis is correct.

---

## Task 2 — Volume-scoped entries (the real multi-disk fix)

**UPDATE: self-exclusion DONE.** graph.retain_entries (pure, tested) + platform::own_volume_id (via LoadedImage::device) + engine drops MyBoot's own \EFI\BOOT loader on its own volume after discovery. 108 pure tests pass. Menu now shows only the real OSes; n-OS discovery + graph + volume-scoping preserved. One likely uefi-0.33 build nit: LoadedImage::device() may return Handle (not Option) — drop the `?` if so.

**STATUS: implemented. Pure crates (graph/discovery) compile + pass (107 tests, incl. 2 new multi-volume tests). platform/engine written but need an on-machine build — expect a couple uefi-0.33 nits (likely `node.data()`, `LoadImageSource::FromDevicePath`, or the `&dyn FileStore` slice).**

**Why.** MyBoot now scans every volume, but discovery de-duplicates entries by a
**path-based** stable id, and `MultiVolume::chainload` loads the **first** volume
that has a path. So two volumes that share a loader path — e.g. `\EFI\BOOT\BOOTX64.EFI`
on both MyBoot's own medium and a Ventoy USB, or two disks each with Windows at
`\EFI\Microsoft\Boot\bootmgfw.efi` — collapse into one entry and chainload the wrong
volume (often MyBoot's own medium → recursion). Installed OSes with **distinct**
paths already work; this fixes the **same-path-on-two-volumes** case.

**Design (implement, then test — keep diffs surgical, no stubs):**

1. `crates/platform/src/volume/mod.rs`
   - Give `Volume` a stable, opaque `volume_id: String` derived from its device
     path: `boot::open_protocol_exclusive::<DevicePath>(handle)` then
     `dp.to_string(DisplayOnly, AllowShortcuts)` (or the 0.33 equivalent — verify
     the signature). Expose `Volume::id(&self) -> &str`.
   - Add `MultiVolume::chainload_on(&self, volume_id: &str, path: &str, options) ->
     Status` that loads from the named volume specifically; keep the old
     `chainload` as a fallback that scans (used only when no volume id is known).
   - Expose an iterator of `(volume_id, &Volume)` so discovery can scan per volume.

2. `crates/graph` (pure — no uefi)
   - Add an opaque `volume: Option<String>` field to `BootEntry` (and to the
     `RawEntry` in `discovery`). It is just an id string; the graph crate stays
     `no_std`, `forbid(unsafe_code)`.
   - Include `volume` in `EntryId::derive(...)` so two same-path loaders on
     different volumes get **distinct** ids. Update the graph tests.

3. `crates/discovery`
   - Change the scan to run **per volume**: for each `(volume_id, fs)` run the
     filesystem sources and tag every `RawEntry` with that `volume_id`. Firmware
     `variables` entries (NVRAM) have no volume — leave `volume = None`.
   - In `build_graph`, only merge entries that share the same `volume` (or merge a
     `None`-volume NVRAM entry into a volume-bearing one when their loader path
     matches). Update/extend the merge tests, including a new test:
     "same loader path on two volumes yields two entries."

4. `crates/engine/src/pipeline.rs`
   - When chainloading, pass the chosen entry's `volume` to
     `MultiVolume::chainload_on(volume_id, path, options)`; fall back to the
     scanning `chainload` only when `volume` is `None`.
   - The `MultiVolume::discover()` path must expose per-volume scanning to
     discovery (adjust the call site so discovery gets the tagged volumes).

5. Keep all pure-crate host tests green (`./manage.py test`) and add the new tests
   above. Then re-run `./manage.py check`.

Follow the uefi-0.33 compile-fix discipline (CLAUDE.md §5) for any API mismatch —
verify `DevicePath::to_string` / display params against docs.rs for 0.33.

---

## Task 3 — Real-OS test with the user's media

The user has a **64 GB Ventoy USB** carrying three images (NixOS, Arch, a live OS).
Ventoy is itself an EFI boot manager at `\EFI\BOOT\BOOTX64.EFI` on the USB; booting
it shows a menu of those ISOs. Two ways to test, after Task 2:

### 3a. In QEMU (safe, no writes to the USB)
Attach the Ventoy USB **read-only** as an extra disk and boot MyBoot's synthetic ESP
with a display so Ventoy's menu is visible. Add support in `pyboot` if not present:
- `pyboot/qemu.py`: allow a raw, read-only extra drive
  (`-drive file=<dev>,format=raw,readonly=on,if=virtio`) and a `--display` toggle.
- Add a `disk-test` subcommand that stages the ESP, attaches the given block device
  read-only, runs QEMU **with a display**, and asserts on serial that MyBoot
  `scanning >= 2 volume(s)`, hands off to the Ventoy loader on the USB's volume, and
  does not bounce back. Visual pass = Ventoy's menu appears; pick an image and it
  boots.
- NEVER attach the device writable. Require an explicit `--device`.

### 3b. On real hardware (the real proof) — user-driven, do NOT do this yourself
Guidance to give the user (they perform it, you do not touch hardware):
- Best first test: the laptop's **own installed OSes** have distinct loader paths and
  already work. Put MyBoot on a small USB (`./manage.py usb --device /dev/sdX`),
  boot it from the firmware menu (F12/F9/Esc), and pick an installed OS. Removing the
  USB reverts everything.
- The Ventoy USB works once Task 2 lands (it resolves the `\EFI\BOOT` collision).
- Only after USB tests pass should internal-disk install be considered, with
  systemd-boot/Windows-boot-manager kept as the firmware default and a rescue USB
  ready. That step is the user's decision.

---

## Reporting

After each task, append to `TESTLOG.md`: date, commands, key serial lines, and any
PROVEN/UNPROVEN change. Task 2 flips "device-path chainload across volumes" from
UNPROVEN to PROVEN only when a real loader on a *second* volume boots.
