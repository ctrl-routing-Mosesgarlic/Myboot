# Proposal: Abstracted Filesystem Chainloading (Btrfs, ZFS, and beyond)

Status: proposal only — no code changed by this document.

## 1. The challenge, restated precisely

> True production environments often deploy complex filesystems like Btrfs
> (with nested subvolumes) or native ZFS boot pools. Writing complex
> filesystem drivers inside a memory-safe `#![forbid(unsafe_code)]`
> environment is a massive undertaking.

The premise is correct, and the code confirms it: MyBoot has **no** on-disk
filesystem driver of its own beyond the Kani-proven *structural* parsers in
`crates/storage` (GPT header/entry, MBR protective-GPT check, FAT boot
sector, PE header — `crates/storage/src/{gpt,mbr,fat,pe}/mod.rs`), and those
are not even wired up (see Proposal 01, §2). Every actual file read in this
codebase goes through `uefi::proto::media::fs::SimpleFileSystem`
(`crates/platform/src/fs/mod.rs`, `crates/platform/src/volume/mod.rs`) — i.e.
whatever filesystem the **firmware itself** already knows how to mount.
Stock UEFI firmware ships a FAT driver only (for the ESP per the UEFI spec);
it has no idea what Btrfs or ZFS is. So "MyBoot reads Btrfs/ZFS" is not a
feature MyBoot's own `ports::FileStore`/`platform` layer can ever provide
without either (a) writing a full from-scratch B-tree/DMU filesystem parser
in safe Rust, Kani-verified to the same panic-free standard as the existing
100-line parsers — genuinely the "massive undertaking" the prompt names, or
(b) delegating to something that already solves this, the way every
production boot manager in this space actually does it.

## 2. Prior art — three real, shipping precedents for (b)

### 2.1 rEFInd: load third-party `.efi` filesystem drivers before scanning

<https://rodsbooks.com/refind/drivers.html> confirms: starting at rEFInd
0.2.7 it gained the ability to `LoadImage`/`StartImage` arbitrary UEFI
**driver-type** binaries from its own `drivers`/`drivers_x64` directory
*before* it enumerates volumes. Since 0.4.0 it ships pre-built drivers named
after the filesystem they handle — `ext4_x64.efi`, `btrfs_x64.efi` — with the
Btrfs one explicitly credited as "based on the rEFIt/rEFInd driver framework
and algorithms from the GRUB 2.0 Btrfs driver." Once started, each driver
registers its own `EFI_SIMPLE_FILE_SYSTEM_PROTOCOL` instance on the matching
block-device handles. From that point on, rEFInd's (and, if MyBoot adopted
this, MyBoot's) **existing** volume-enumeration code sees Btrfs volumes as
just more `SimpleFileSystem` handles — no special-casing anywhere downstream.

### 2.2 ZFSBootMenu: push the real filesystem driver into a second-stage kernel

<https://docs.zfsbootmenu.org/> describes a different, arguably more robust
trick: don't teach the *boot manager* ZFS at all. ZFSBootMenu is itself a
tiny, self-contained Linux kernel + initramfs (or EFI stub) that an ordinary
UEFI boot manager chainloads **exactly like any other EFI-stub Linux entry**
from a plain FAT ESP. Only once that second-stage kernel is actually running
does it use the **real, in-kernel ZFS driver** to import pools and locate
boot environments, then `kexec`s into the chosen one. ZFS's full complexity
(ZAP, DMU, uberblocks, optional raidz/encryption) is handled entirely inside
a component that already has to implement it correctly for the running OS
anyway — nothing upstream of it needs to understand ZFS structure at all.

### 2.3 GRUB2 core.img with `btrfs.mod`/`zfs.mod`: the path MyBoot already supports today

GRUB added Btrfs support via `grub-probe` (root-on-Btrfs works as long as
`/boot` itself is on a GRUB-native filesystem), and ZFS support has shipped
in official GRUB releases since 1.99~rc1. Distros already drop a small GRUB
`core.img` — containing whatever `.mod` filesystem modules it needs — onto
the plain-FAT ESP at e.g. `\EFI\ubuntu\grubx64.efi`. **MyBoot already
chainloads this today, unmodified**: `crates/discovery/src/distros.rs:22`'s
`LOADERS` table tries `shimx64.efi`/`grubx64.efi` per vendor directory, and
`crates/providers/src/linux.rs`'s `BootMethod::EfiChainload` plan hands it
off exactly like any other EFI image. GRUB itself, once running, opens the
Btrfs/ZFS root using its own bundled modules — MyBoot never touches the
filesystem at all in this path.

**Caveat found during research:** Canonical has proposed dropping `/boot` on
Btrfs/HFS+/XFS/ZFS support from the *signed* GRUB build ahead of Ubuntu
26.10, specifically to shrink the Secure-Boot-signed attack surface (reported
by Phoronix and OMG! Ubuntu, citing the Canonical engineering proposal). This
does not affect unsigned/self-built GRUB or other distros' signed builds, but
it means option 2.3 should not be treated as a permanent guarantee on every
Secure-Boot system going forward — strengthening the case for having 2.1 or
2.2 available as well.

## 3. Recommendation, ranked by fit to MyBoot's architecture

| # | Approach | New unsafe code in MyBoot? | New parsing code in MyBoot? | Effort |
|---|----------|------------------------------|------------------------------|--------|
| 2.3 | GRUB2 core.img (already works) | none | none | **zero** — already shipping |
| 2.2 | Second-stage kernel (ZFSBootMenu-style) | none | none | small — one new discovery source |
| 2.1 | Load rEFInd-style `.efi` fs drivers | confined to `platform` (already allowed) | none (reuse existing binaries) | medium |
| — | Write a native Btrfs/ZFS parser crate | must be 0 unsafe to respect `forbid(unsafe_code)`, so technically possible, but… | thousands of LOC of B-tree/checksum/compression logic | **not recommended** |

**Do 2.3 first (it needs nothing — verify and document it), then 2.2, then
offer 2.1 as an advanced option.** Do not attempt the native-driver column;
it inverts the effort/value ratio the other three already solve.

### 3.1 Verify 2.3 is actually exercised (no code change, just a test gap to close)

Nothing in `TASKS.md`/`TESTLOG.md` records an ISO/real-disk test against a
distro whose `/boot` or root is Btrfs. Before building anything new, run the
existing §3.4 ISO test (root `CLAUDE.md`) against a Btrfs-rooted distro ISO
(e.g. an openSUSE or Fedora Btrfs-by-default install) to turn "should already
work" into "observed working," per the project's own PROVEN/UNPROVEN
discipline.

### 3.2 Add 2.2 — a ZFSBootMenu-shaped discovery source

This fits the existing `discovery` architecture exactly, with no port
changes:

1. New file `crates/discovery/src/zfsbootmenu.rs` (one job, SRP, matching the
   shape of every existing source module such as `esp.rs`/`distros.rs`):
   look for a well-known second-stage image — ZFSBootMenu publishes itself as
   a single self-contained EFI binary, so detect it the same way `esp.rs`
   detects the fallback loader (a known path under `\EFI\`, e.g.
   `\EFI\zbm\VMLINUZ.EFI` or a user-configured path per ZFSBootMenu's own
   install docs — **confirm the exact installed path convention against
   ZFSBootMenu's current install documentation before hard-coding it**, since
   it is user/installer configurable) — and emit a `RawEntry` with
   `BootMethod::EfiChainload`, `OsKind::Linux` (or a new `OsKind::ZfsBootMenu`
   if the project wants it visually distinct in the menu — a `graph::kinds`
   enum change, trivially additive).
2. Wire it into `crates/discovery/src/lib.rs::scan_volume` alongside the
   other four sources (`lib.rs:78-89`) — one line added to the `raw.extend(...)`
   chain.
3. No `crates/providers` change needed: `BootMethod::EfiChainload` already
   routes to `LaunchPlan::Chainload` via the existing Linux/generic provider
   path, and `health::assess_entry` already correctly reports
   `Unbootable`/`Healthy` from a plain FAT-visible loader path — identical to
   every other entry in the graph today.
4. No `ports` or `platform` change at all. No new `unsafe`. This is the
   cheapest of the three options to implement and test, because MyBoot
   treats the ZFS pool as 100% the second-stage kernel's problem, exactly as
   ZFSBootMenu's own design intends.

### 3.3 Add 2.1 as an opt-in advanced path (only if a user wants MyBoot itself,
not a second-stage kernel, to resolve files on a Btrfs volume directly — e.g.
reading a kernel/initrd straight off a Btrfs `@` subvolume for a Linux
EFI-stub entry with no GRUB in between)

1. New file `crates/platform/src/volume/drivers.rs` (one job: load and start
   pre-staged `.efi` driver binaries from a well-known location such as
   `\EFI\MyBoot\drivers\*.efi`, via `boot::load_image` +
   `boot::start_image` — the exact same two calls `volume/mod.rs` already
   uses for chainloading, just with a driver-type image and no
   `LoadedImage::set_load_options`/no expectation of non-return). Call this
   **before** `connect_all_controllers()` runs in
   `MultiVolume::discover()` (`volume/mod.rs:195-198`), so freshly-registered
   filesystem protocols are present when `volumes()` enumerates handles.
2. This confines all new `unsafe` to `crates/platform`, which CLAUDE.md §0.7
   already permits — no change to `forbid(unsafe_code)` anywhere else, and no
   new parsing code is written by MyBoot at all; the Btrfs-reading logic is
   entirely inside the vendored driver binary.
3. **Licensing/provenance must be checked before vendoring any binary**: the
   rEFInd Btrfs driver derives its algorithms from GRUB 2.0's Btrfs module,
   which is GPLv3; confirm the exact license terms of whichever driver binary
   is actually redistributed with MyBoot before shipping it, and prefer
   building the driver from its published source under MyBoot's own release
   process over redistributing a pre-built binary of unclear provenance.
4. Treat failure to load a driver as non-fatal (best-effort, matching
   `connect_all_controllers`'s own `let _ =` discard-on-error style) — a
   missing/incompatible driver file must never block boot of everything
   else.

## 4. Explicitly out of scope / not recommended

- Writing a native Btrfs or ZFS reader inside `crates/storage` or a new pure
  crate. The existing parsers it would need to imitate (GPT/MBR/FAT/PE) are
  each under 150 lines because they parse a handful of fixed-offset header
  fields; Btrfs requires traversing a copy-on-write B-tree forest with
  checksummed (CRC32C or xxhash, depending on feature flags) nodes and
  optional transparent compression (zlib/LZO/zstd — GRUB's own 2.14 added
  zstd support only recently per its NEWS file), and ZFS requires the DMU
  object layer, ZAP hash tables, and uberblock/txg validation. Matching the
  project's existing Kani-proof rigor for something this size is a
  multi-month undertaking with a large unsafe-free parsing surface to get
  bit-exact — exactly the "massive undertaking" the prompt warns about, for a
  problem options 2.1–2.3 already solve by delegation.

## Sources

- rEFInd driver loading & bundled filesystem drivers: <https://rodsbooks.com/refind/drivers.html>
- ZFSBootMenu architecture and boot flow: <https://docs.zfsbootmenu.org/>, <https://zfsbootmenu.org>
- GRUB Btrfs/ZFS support history: Debian `grub2` NEWS (<https://sources.debian.org/src/grub2/2.14-2/NEWS>)
- Proposed Secure-Boot GRUB filesystem-support drop ahead of Ubuntu 26.10: Phoronix forum discussion thread (<https://phoronix.com/forums/node/1063728>); OMG! Ubuntu coverage (via <https://www.linux.org/threads/omg-ubuntu-ubuntu-26-10-could-drop-btrfs-zfs-and-luks-support-from-grub.64427/>)
