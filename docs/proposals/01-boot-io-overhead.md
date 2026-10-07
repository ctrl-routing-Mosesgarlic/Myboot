# Proposal: Optimize Boot-Time I/O Overhead

Status: proposal only — no code changed by this document. Written per CLAUDE.md
§6 ("a change bigger than a surgical fix... pause and write a short proposal
first"). Implementing this is a multi-step change across `platform` and
`discovery` and should be done one surgical commit at a time, each re-tested
against `./manage.py test` / `./manage.py smoke`.

## 1. The challenge, restated precisely

> Because MyBoot reads every partition table at every cold boot, it can create
> a noticeable lag on production setups with multiple NVMe/SATA drives.

## 2. What the code actually does today (evidence)

Grepping the workspace shows the Kani-proven partition parsers in
`crates/storage` (`gpt/mod.rs`, `mbr/mod.rs`) are **never called** from
`crates/platform` or `crates/engine` — confirmed with
`grep -rn "gpt::\|mbr::\|parse_partitions" crates/platform crates/engine crates/discovery`
(no matches). MyBoot does not parse raw GPT/MBR itself; UEFI firmware's own
partition driver already does that before exposing a
`SimpleFileSystem` handle per volume. So the literal "reads every partition
table" cost is really two separate, both real, costs inside MyBoot itself:

**(a) `connect_all_controllers()` — `crates/platform/src/volume/mod.rs:153-160`**

```rust
pub fn connect_all_controllers() {
    if let Ok(handles) = boot::locate_handle_buffer(boot::SearchType::AllHandles) {
        for &handle in handles.iter() {
            let _ = boot::connect_controller(handle, None, None, true); // recursive
        }
    }
}
```

This forces the firmware to attempt driver binding on **every handle in the
system** — NICs, USB controllers, GPUs, TPMs, every PCI function, every child
partition — not just storage. The doc comment above it (lines 147-152)
correctly explains *why* this exists (many firmwares only auto-connect the
volume they booted from, so other ESPs/disks stay invisible without this),
but the implementation is maximally broad: `SearchType::AllHandles` +
recursive `connect_controller` on literally everything, every boot, with no
filtering.

**(b) Per-volume discovery — `crates/discovery/src/lib.rs:78-89` (`scan_volume`)**

Called once per volume from `discover_multi` (`lib.rs:95-104`), itself called
once per cold boot from `crates/engine/src/pipeline.rs:56`. For **every**
volume, `scan_volume` runs four independent sources, each doing its own
UEFI `SimpleFileSystem` Open/Read/Close round trips:

- `esp::discover` (`crates/discovery/src/esp.rs`): 2 `exists()` calls, then —
  only if the fallback loader is present — up to **5 more file reads** in
  `media_name()`'s fallback chain (`BOOT.CSV`/`BOOTX64.CSV`, `.disk/info`,
  three `grub.cfg` candidates, three loader-binary string scans) before
  falling back to the volume label.
- `distros::discover` (`crates/discovery/src/distros.rs`): one `list_dir`,
  then **per vendor directory** up to 4 `exists()` calls (the `LOADERS` table)
  plus up to 2 `read()` calls for `os-release`/`PRETTY_NAME`.
- `bls::discover`, `bootspec::discover`: similar directory-listing + per-entry
  read patterns (not reproduced here, same shape).
- `refine::refine_methods` runs again over the merged set.

None of this is cached. On an N-volume machine this is
`O(N × (sources × per-source reads))` real block I/O round trips through the
firmware's `SimpleFileSystem` protocol, unconditionally, on **every single
cold boot**, even when the exact same disks were present and unchanged last
boot. This is the actual, measurable, user-visible lag on a multi-NVMe/SATA
box — not partition-table parsing, but repeated *identification* probing.

A `grep -rn "cache\|Cache" crates/ bin/ host-cli/` finds zero hits — there is
no caching layer anywhere in the Rust workspace today.

## 3. Prior art (what production boot managers do about this)

- **rEFInd's connect step** (`EfiLib/BdsConnect.c`,
  <https://code.delx.au/refind/blob/HEAD:/EfiLib/BdsConnect.c>) does not
  connect every handle indiscriminately either. `BdsLibConnectMostlyAllEfi()`
  filters out driver-binding-protocol handles and image handles (only real
  *device* handles are candidates), explicitly disconnects PCI VGA devices
  before connecting anything else (so GPUs aren't needlessly re-probed), and
  skips handles that already have a parent in the device tree (so a child
  partition handle already serviced by its parent disk's driver isn't
  reconnected again). This is a direct, shippped precedent for "connect
  selectively, not everything."
- **rEFInd's own users report multi-second startup lag from scanning**
  (<https://forum.endeavouros.com/t/refind-takes-too-long-a-lot-longer-than-grub-to-load-at-boot/30793>),
  and rEFInd's own documentation (`refind.conf-sample`) ships an explicit
  `scan_all_linux_kernels`/`dont_scan_volumes` opt-out specifically because
  deep per-kernel-file scanning on every volume is expensive — the same shape
  of cost MyBoot's `media_name()`/`pretty_name_on_esp()` chains have.
- **systemd-boot** avoids this class of cost almost entirely by relying on
  the Boot Loader Specification's convention of pre-digested `.conf` entry
  files in `\loader\entries\` (one read each) rather than scanning loader
  binaries for embedded strings — MyBoot's own `bls.rs` source already follows
  this cheaper pattern; it is `esp.rs`'s *fallback-loader naming* heuristic
  chain that is the expensive outlier, because it exists specifically to
  handle loaders (bare ISOs, Ventoy images) that carry **no** BLS/os-release
  metadata at all.

## 4. Proposed design

Two independent, separately-landable fixes — do (A) first (smallest, safest,
highest-confidence), then (B).

### (A) Narrow `connect_all_controllers()`

Mirror rEFInd's filtering, adapted to the uefi-0.33 API surface already in
use elsewhere in this file (`boot::locate_handle_buffer`,
`boot::SearchType::ByProtocol`, both already called in `volumes()` at
`volume/mod.rs:164`):

1. Before the broad connect pass, call
   `boot::locate_handle_buffer(boot::SearchType::ByProtocol(&SimpleFileSystem::GUID))`
   to get the set of handles **already** exposing a filesystem — skip those,
   they need no reconnect work.
2. Restrict the connect target set to handles exposing
   `EFI_BLOCK_IO_PROTOCOL` (`uefi::proto::media::block::BlockIO::GUID` —
   **verify this exact path against docs.rs for uefi 0.33.0 before coding**,
   per CLAUDE.md §5) rather than `SearchType::AllHandles`. Every volume that
   can ever yield a `SimpleFileSystem` sits behind a block-IO handle; NICs,
   GPUs, TPMs, HID, etc. do not, so this alone removes the large majority of
   needless connect attempts without touching any storage-adjacent handle.
3. Keep `connect_controller(handle, None, None, true)` (recursive) only for
   the remaining filtered set, so child partition handles under a disk are
   still picked up — this preserves the existing, working behaviour for
   multi-ESP discovery, it just stops wasting cycles on non-storage devices.

This is a single-file, single-function change in
`crates/platform/src/volume/mod.rs`, entirely within `platform`'s already
sanctioned `unsafe`/firmware-facing scope (CLAUDE.md §0.7), and does not touch
any pure crate or its tests.

### (B) Skip expensive *naming* probes when the volume hasn't changed, never skip *health*

The key invariant to preserve: **health/bootability must always be computed
fresh, every boot** (`health::assess_all`, `crates/health/src/lib.rs:21-31`,
already called unconditionally after discovery in `pipeline.rs:59`) — a file
that vanished since last boot must show `Unbootable` immediately. Caching
must only ever skip the *cosmetic* work (figuring out a pretty title via
`media_name()`'s 5-step fallback chain, `PRETTY_NAME` reads), never the
*exists* checks discovery/health already do cheaply.

1. New file `crates/discovery/src/cache.rs` (one job: encode/decode a tiny,
   `no_std`, `forbid(unsafe_code)` discovery-cache record — same pattern and
   rigor as `transaction::marker`). Record shape:
   ```rust
   pub struct VolumeFingerprint {
       pub volume_id: String,      // from Volume::id(), already stable (volume/mod.rs:56-70)
       pub top_level_sig: u64,     // cheap hash of list_dir("\EFI") names + a handful of exists() bits
       pub resolved_title: String, // the expensive media_name()/PRETTY_NAME result, if any
   }
   ```
   Total encode/decode, like `marker::Marker` — malformed cache entries decode
   to "absent" (treated as a cache miss), never an error that blocks boot.
2. Storage: a file on MyBoot's **own** ESP via the existing `fs: FileStore`
   already threaded through `pipeline.rs` (e.g. `\EFI\MyBoot\discovery-cache.bin`)
   — explicitly **not** a vendor NVRAM variable. `persistence/nvram.rs`'s own
   comment ("NVRAM is scarce and wears") and `pipeline.rs:220-224`'s
   `persist_tries` no-op already establish that this codebase treats NVRAM
   writes as a cost to avoid; a cache that would be rewritten on most boots
   belongs on the FAT ESP, not in NVRAM.
3. In `discovery::scan_volume` (or a thin wrapper the engine calls), compute
   `top_level_sig` first — this is itself cheap (one `list_dir`, already done
   for diagnostics in `pipeline.rs:48-49`). If it matches the cached
   fingerprint for that `volume_id`, reuse `resolved_title` instead of running
   `media_name()`'s expensive chain / `pretty_name_on_esp`'s reads; otherwise
   run the full chain as today and update the cache entry.
4. Every other part of the pipeline (health assessment, graph building,
   policy, transaction) is unaffected — this only short-circuits the naming
   heuristics inside `esp.rs`/`distros.rs`, which is where the per-volume read
   multiplier actually lives.

### (C) Make the win measurable, not just argued

Add a start/end `log::info!` timestamp pair around the discovery stage in
`pipeline.rs` (the pipeline already logs extensively via `log::info!`, so
this is consistent with existing style) and extend `pyboot/logparse.py` /
`pyboot/assertions.py` with a simple "discovery completed within N serial-log
ticks" assertion, so `./manage.py smoke` can catch a regression instead of
relying on "feels faster."

## 5. Explicitly out of scope / not recommended

- Do **not** add a general "skip scanning volume X" opt-out list (rEFInd's
  `dont_scan_volumes`) as a first step — it requires user configuration and
  does not fix the default-case lag; it's a reasonable fallback knob to add
  *after* (B) lands, not instead of it.
- Do **not** attempt to parse partition tables ourselves to "shortcut"
  discovery — the evidence above shows that is not where the cost is; adding
  a redundant GPT/MBR read path would be new, unneeded I/O and code the
  firmware's own driver already does for free.

## 6. Verification checklist before landing

- [ ] Confirm `uefi::proto::media::block::BlockIO::GUID` and
  `boot::SearchType::ByProtocol` signatures against docs.rs for uefi 0.33.0
  exactly (CLAUDE.md §5 compile-fix discipline).
- [ ] `./manage.py test` stays green (pure `discovery`/`cache` unit tests:
  fingerprint match → title reused; fingerprint mismatch → full probe reruns;
  corrupt cache → treated as absent, never a boot-blocking error).
- [ ] `./manage.py smoke` still shows `scanning N volume(s)`, 3 bootable
  entries, and a successful shell handoff, with discovery taking visibly
  fewer serial-log ticks on the second of two consecutive runs.
- [ ] Re-run the ISO test (§3.4 of the root CLAUDE.md) to confirm (A)'s
  narrower connect pass does not regress the Ventoy/ISO device-path chainload
  path, since that path depends on `connect_all_controllers()` having bound a
  filesystem driver to the ISO's/USB's handle.

## Sources

- rEFInd connect logic: <https://code.delx.au/refind/blob/HEAD:/EfiLib/BdsConnect.c>
- rEFInd scan cost reports: <https://forum.endeavouros.com/t/refind-takes-too-long-a-lot-longer-than-grub-to-load-at-boot/30793>
- rEFInd `scan_all_linux_kernels` / `dont_scan_volumes`: <https://rodsbooks.com/efi-bootloaders/refind.html>
- UEFI 2.10 spec (Protocols — UEFI Driver Model, GPT layout): <https://uefi.org/specs/UEFI/2.10/genindex.html>
