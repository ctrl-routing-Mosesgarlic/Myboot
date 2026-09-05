# uefi 0.33 → 0.40: upgrade analysis (for later)

Decision **now**: stay on **uefi 0.33.0**. It builds, and it boots real OSes
(Ubuntu, Parrot proven on OVMF). This note records why, and exactly what to do when
we upgrade, so the jump is planned — not a scramble against a working build.

## Is there a SECURITY reason to upgrade? — No (as of this writing)

I checked the RustSec advisory database and the uefi-rs changelogs. **There is no
published CVE or RUSTSEC advisory against the `uefi` crate** — not for 0.33, not for
any version. So we are not sitting on a known security hole by staying on 0.33.

The one thing that *sounds* security-relevant in the newer changelogs, and whether
it touches us:

- **0.34** — "Fixed missing checks in the `TryFrom` conversion from
  `&DevicePathNode` to specific node types" (type/subtype now validated), and
  "Fixed memory leaks in the DevicePathFromText protocol."
  - Does it affect MyBoot? **No.** We never use the typed `TryFrom<&DevicePathNode>`
    conversions, and we don't use `DevicePathFromText`. Our device-path code reads
    `node.data()` directly and builds paths with `DevicePathBuilder`. So this fix is
    irrelevant to our code paths.
- The `exit_boot_services` "hang with a custom memory type" issue (uefi-rs #1375) is
  a **platform/firmware** quirk, not a uefi-rs bug, and we use `MemoryType::LOADER_DATA`
  (the recommended value), so it does not affect us.

Conclusion: the upgrade is an **ergonomics/maintenance** decision, not a safety one.

## What we GAIN on 0.40 (nice-to-have, not safety)

- Rust **2024 edition** and modern MSRV (cleaner language features).
- `Boolean` type replacing raw `bool` at FFI boundaries (more correct FFI, minor).
- `PoolDevicePath` / `PoolDevicePathNode` helpers, streamlined protocol docs, small
  API polish.
- Staying current makes *future* upgrades smaller and keeps us on the maintained line.

## What we KEEP by staying on 0.33 right now

- A build that **compiles and boots real operating systems** — the thing that took
  weeks to reach. We don't destabilise it without cause.
- Our current Rust toolchain pin (the flake's rust-overlay) — 0.36+ **requires MSRV
  1.85.1 and Rust 2024 edition**, i.e. a toolchain bump too.

## The cost of the jump (0.33 → 0.40 is SEVEN releases) — what will break

Concrete breaking changes we'll have to fix, from the changelogs:

1. **`exit_boot_services`** (0.35): now takes `Option<MemoryType>`. Our call passes a
   plain `MemoryType::LOADER_DATA` → wrap it as `Some(MemoryType::LOADER_DATA)`.
   (`crates/engine/src/direct_boot.rs`.)
2. **Rust 2024 edition + MSRV 1.85.1** (0.36): bump the flake's Rust toolchain; expect
   edition-2024 lint/keyword fallout across all crates.
3. **`bool` → `Boolean`** in raw types (uefi-raw 0.13 / 0.36.1): any place we touch raw
   FFI booleans.
4. **Device-path / `LoadImageSource` / `LoadedImage` APIs** may have shifted again —
   our newest code (`platform/src/volume/mod.rs`: `DevicePathBuilder`,
   `LoadImageSource::FromDevicePath`, `LoadedImage::device()`) is exactly the fragile
   surface. Re-verify each against the 0.40 docs.
5. Re-check `variable_keys`, `locate_handle_buffer`, `open_protocol_exclusive`
   signatures.

## The upgrade plan (when we do it)

1. Branch off the working, OS-booting commit.
2. Bump `uefi` to 0.40 in the workspace `Cargo.toml` and the flake's Rust toolchain
   to ≥ 1.85.1 (edition 2024).
3. Fix compile errors one at a time against the **0.40** docs (same discipline that
   got us through 0.33), starting with the five items above.
4. **Gate:** `./manage.py check` must stay green (pure tests + build + smoke), then the
   `iso-test` must still boot a real distro. Only merge when both pass.
5. Keep 0.33 as the fallback branch until 0.40 is proven on hardware too.

Bottom line: no fire, so no rush. Upgrade deliberately, on a branch, with the ISO
boot test as the acceptance gate.