# MyBoot — Build, Run & Troubleshooting Guide

This repo has two kinds of crates:

1. **Pure crates** (`graph`, `storage`, `ports`, `persistence`, `config`,
   `policy`, `health`, `discovery`, `transaction`, `security`, `providers`,
   `ui-shell`, `ui-gfx`) — `no_std` + `alloc`, no firmware. **90 unit tests**,
   all passing on the host.
2. **UEFI-dependent crates** (`arch`, `platform`, `engine`, `bin/myboot`) —
   require the `x86_64-unknown-uefi` target to compile. Plus two **host std
   tools** (`host-cli`, `xtask`) that build for your normal OS target.

Everything is production code — no stubs, no TODOs.

---

## 1. Prerequisites

```bash
rustup toolchain install stable
rustup target add x86_64-unknown-uefi          # for the firmware crates
# For running under emulation:
#   Debian/Ubuntu: sudo apt install qemu-system-x86 ovmf
#   Fedora:        sudo dnf install qemu-system-x86 edk2-ovmf
#   Arch:          sudo pacman -S qemu-full edk2-ovmf
# Optional verifiers:
cargo install --locked kani-verifier && cargo kani setup   # proofs on storage
# TLA+: get tla2tools.jar (TLA+ Toolbox) for `xtask tlc`
```

The repo pins the default build target to `x86_64-unknown-uefi` in
`.cargo/config.toml`, so a plain `cargo build` builds the firmware app. The two
host tools are **excluded** from the workspace and built with an explicit host
target (see §5).

---

## 2. Build the UEFI application

```bash
cargo build -p myboot --target x86_64-unknown-uefi            # debug
cargo build -p myboot --target x86_64-unknown-uefi --release  # release
# output: target/x86_64-unknown-uefi/{debug,release}/myboot.efi
```

Or use the automation (see §5):

```bash
cargo run --manifest-path xtask/Cargo.toml --target $(rustc -vV | sed -n 's/host: //p') -- build --release
```

## 3. Run the host unit tests

The default target has no test runner, so pass a host target:

```bash
HOST=$(rustc -vV | sed -n 's/host: //p')
cargo test -p graph -p storage -p ports -p persistence -p config -p policy \
           -p health -p discovery -p transaction -p security -p providers \
           -p ui-shell -p ui-gfx --target "$HOST"
```

(Every pure crate declares `#[cfg(test)] extern crate std;`, so `cargo test`
links `std` only for the test binaries; the crates themselves stay `no_std`.)

## 4. Boot it under QEMU + OVMF

```bash
cargo run --manifest-path xtask/Cargo.toml --target "$HOST" -- run --release
```

`xtask run` builds `myboot.efi`, assembles an ESP tree under `target/esp/`
(`EFI/BOOT/BOOTX64.EFI` + a sample `EFI/MyBoot/config.toml`), copies a writable
`OVMF_VARS`, and launches:

```
qemu-system-x86_64 -machine q35,accel=tcg -m 512M \
  -drive if=pflash,format=raw,unit=0,readonly=on,file=$OVMF_CODE \
  -drive if=pflash,format=raw,unit=1,file=target/OVMF_VARS.rw.fd \
  -drive format=raw,file=fat:rw:target/esp -serial stdio -net none
```

If OVMF lives elsewhere, override the paths:

```bash
export OVMF_CODE=/usr/share/edk2/x64/OVMF_CODE.4m.fd
export OVMF_VARS=/usr/share/edk2/x64/OVMF_VARS.4m.fd
```

## 5. Host tools (`host-cli`, `xtask`)

Both are std binaries **excluded** from the no_std workspace, so build them with
an explicit host target (the repo default is the UEFI triple):

```bash
HOST=$(rustc -vV | sed -n 's/host: //p')
cargo build --manifest-path host-cli/Cargo.toml --target "$HOST"
cargo build --manifest-path xtask/Cargo.toml    --target "$HOST"
```

`host-cli` (the `myboot` OS-side tool) talks to firmware via efivarfs and needs
root for writes:

```bash
sudo ./target/$HOST/debug/myboot status        # Secure Boot + BootOrder
sudo ./target/$HOST/debug/myboot bless          # confirm the current boot (good)
./target/$HOST/debug/myboot config path ./config.toml   # validate a config
```

## 6. Formal verification

```bash
cargo kani -p storage                          # panic-freedom of the parsers
# or: cargo run --manifest-path xtask/Cargo.toml --target "$HOST" -- kani
cargo run --manifest-path xtask/Cargo.toml --target "$HOST" -- tlc   # TLA+ model check
```

---

## 7. Troubleshooting — likely errors and fixes

Because the UEFI crates cannot be compiled in the environment they were authored
in, a few call sites may need a nudge against **your exact `uefi` patch version**
(the manifests pin `uefi = "0.33"`; the API moved a lot across 0.2x→0.33). The
spots most likely to need a one-line tweak, and how to fix each:

- **`can't find crate for std`** when building `host-cli`/`xtask`.
  Cause: the repo default target is `x86_64-unknown-uefi`. Fix: always pass
  `--target "$HOST"` (or `--manifest-path .../Cargo.toml --target "$HOST"`) for
  those two, as in §5.

- **`no method named set_load_options` / signature mismatch**
  (`crates/engine/src/pipeline.rs`, `launch`).
  In some `uefi` versions the method takes `(u32 size, *const u8 ptr)` in the
  other order, or lives on `LoadedImage` differently. Fix: check
  `LoadedImage::set_load_options` in your `uefi` docs and match the argument
  order; the buffer is `opts16` (UTF-16, NUL-terminated) and its byte length is
  `opts16.len() * 2`.

- **`LoadImageSource::FromBuffer` field names**
  (`pipeline.rs`, `launch`). Some versions name the field `file_path:
  Option<&DevicePath>`; older ones omit it. Fix: adjust to the variant your
  version exposes (we pass `file_path: None`).

- **`get_variable_boxed` / `variable_keys` not found**
  (`crates/platform/src/vars/mod.rs`). These are the 0.33 free functions in
  `uefi::runtime`. On slightly older 0.3x they may be `get_variable` (into a
  caller buffer) and `variable_keys()` returning a different iterator. Fix: use
  the two-call size-then-read idiom if `_boxed` is absent; map each key's
  `vendor`/`name()` the same way.

- **`get_image_file_system` returns a different handle type**
  (`crates/platform/src/fs/mod.rs`). We call
  `boot::get_image_file_system(boot::image_handle())?.open_volume()`. If your
  version returns a `ScopedProtocol<SimpleFileSystem>` needing `&mut`, bind it to
  a `let mut` first (already done) — if it still complains, open the protocol via
  `boot::open_protocol_exclusive::<SimpleFileSystem>(handle)`.

- **`read_entry` buffer / `FileInfo` alignment**
  (`fs.rs`, `list_dir`). `read_entry` wants an **aligned** buffer; if you hit
  alignment panics, replace the `vec![0u8; 512]` with `FileInfo`-aligned storage
  (uefi provides `FileInfo`/aligned-buffer helpers) or grow-and-retry on
  `BUFFER_TOO_SMALL` (already implemented).

- **GOP `blt` / `BltPixel` / `current_mode_info`**
  (`crates/platform/src/gop/mod.rs`). Field/enum names around `BltOp`,
  `BltRegion::Full`, and `.resolution()` are stable in 0.33 but were renamed
  earlier. Fix: match your version's `gop` module names.

- **`#[entry]` signature** (`bin/myboot/src/main.rs`). 0.33 uses
  `fn efi_main() -> Status` with no arguments and `uefi::helpers::init()`. If
  your version still wants `(handle, system_table)`, switch to that older
  signature and thread the table through — but prefer upgrading `uefi` to 0.33.

- **QEMU: black screen / drops to UEFI shell.** The firmware didn't find
  `EFI/BOOT/BOOTX64.EFI`. Confirm `xtask esp` populated `target/esp/` and that
  you passed `-drive format=raw,file=fat:rw:target/esp`. Watch `-serial stdio`
  for the engine's log lines.

- **`Secure Boot` refuses the image.** Loading from buffer still runs firmware
  verification. For local testing use OVMF **without** Secure Boot, or enroll
  your keys. The `security` crate already prefers deferring to firmware.

If a fix isn't obvious, the failing call is isolated to `platform`/`engine`; the
pure crates it depends on are covered by the 90 host tests, so the logic beneath
the firmware call is already verified.

---

## Direct-boot path (LinuxDirect) — uefi-rs 0.33 version-sensitive spots

`engine/src/direct_boot.rs` uses the lower-level firmware APIs, which are the
ones most likely to differ across uefi-rs point releases. If the engine fails to
compile, these are the lines to check against your installed uefi-rs version:

- `boot::exit_boot_services(Some(MemoryType::LOADER_DATA))` returns a
  `MemoryMapOwned`. In some versions the argument or return type differs; adjust
  the call and the `to_e820` signature to match.
- `boot::allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, pages)`
  returns a `NonNull<u8>`-like handle; we take `.as_ptr()`. Check the exact
  return type.
- Memory-map iteration: we use `MemoryMap::entries()` yielding descriptors with
  `phys_start`, `page_count`, and `ty`. Field/method names occasionally change.
- `MemoryType` variant names (`CONVENTIONAL`, `ACPI_RECLAIM`, `ACPI_NON_VOLATILE`,
  `UNUSABLE`, `BOOT_SERVICES_*`, `LOADER_*`) — confirm against your version.

The E820 conversion (`to_e820`) and the boot-protocol structures
(`arch::x86_64::boot_params`) are pure and host-tested, so any breakage here is
confined to the thin firmware glue above, not the protocol logic.

## Installing on real hardware

```
# 1. build the loader for UEFI
cargo build -p myboot --release --target x86_64-unknown-uefi

# 2. build the host CLI
cargo build --manifest-path host-cli/Cargo.toml \
    --target $(rustc -vV | sed -n 's/host: //p') --release

# 3. install onto the ESP (default /boot/efi) and create the config
sudo ./host-cli/target/.../release/myboot install \
    --esp /boot/efi \
    --efi target/x86_64-unknown-uefi/release/myboot.efi

# 4. register with firmware (the command install prints, adjust disk/part)
sudo efibootmgr --create --disk /dev/nvme0n1 --part 1 \
    --loader '\EFI\MyBoot\BOOTX64.EFI' --label 'MyBoot' --unicode
```

`myboot install` creates `EFI/MyBoot/config.toml` (never overwriting an existing
one) — that is the answer to "where does config.toml come from".

## Distro discovery

At boot, `discovery` scans `\EFI\` and identifies installed distributions from
their vendor directories (ubuntu, debian, kali, parrot, zorin, mint, fedora,
arch, nixos, gentoo, …), preferring `shimx64.efi` under Secure Boot and reading
`os-release` `PRETTY_NAME` when a copy is present on the ESP. No configuration is
needed for a distro to appear.

---

## uefi 0.33 target build notes (fixes verified against the real API)

Building for `x86_64-unknown-uefi` surfaced five issues the host build could not.
Each was fixed against the actual uefi-rs 0.33 API (compiler output + CHANGELOG),
and each is recorded here so a contributor on a different uefi version knows what
to adjust.

1. **Assembly: ELF-only directives.** `x86_64-unknown-uefi` emits PE/COFF, so the
   ELF-only `.type …, @function` and `.size …` directives in `arch/…/handoff.s`
   fail with "expected absolute expression". Removed them (they are metadata
   only); use `.balign` (portable) not `.align`. AT&T syntax is selected with
   `options(att_syntax)` on the `global_asm!` (not a `.att_syntax` line — that
   trips the `bad_asm_style` lint).

2. **`variable_keys()` shape (0.33).** `runtime::variable_keys()` returns the
   `VariableKeys` iterator directly (no `Result`), and each item is a
   `Result<VariableKey>` (an error for non-UCS-2 names, which does not stop
   iteration). `VariableKey.name` is a public `CString16` field; `name()` is
   deprecated. `platform/vars` iterates and skips `Err` items.

3. **`exit_boot_services` signature (0.33).** Takes a plain `MemoryType`
   (`boot::exit_boot_services(MemoryType::LOADER_DATA)`). The `Option<MemoryType>`
   form is 0.35+. If you bump uefi to ≥0.35, wrap it back in `Some(...)`.

4. **Unused imports.** `String` in `engine/pipeline.rs` and `Vec` at the top of
   `transaction` (the latter belongs in the test module). Removed/relocated.

5. **Global allocator.** The `alloc` feature enables heap APIs but does NOT set a
   `#[global_allocator]`. The final binary (`bin/myboot`) enables the uefi
   `global_allocator` feature, which designates `uefi::allocator::Allocator`
   (backed by the firmware pool allocator). Without it: "no global memory
   allocator found but one is required". `uefi::helpers::init()` then initialises
   it (plus logger + panic handler) at runtime.

With these, `cargo build -p myboot --release --target x86_64-unknown-uefi`
produces `target/x86_64-unknown-uefi/release/myboot.efi` (a PE32+ EFI
application).
