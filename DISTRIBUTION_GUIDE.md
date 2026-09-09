# Getting MyBoot into the distros — distribution roadmap

Goal: a user installing or maintaining a Linux system can pick **MyBoot** from their
distribution's bootloader options the same way they pick GRUB, systemd-boot, or
rEFInd — `pacman -S myboot`, `apt install myboot`, `dnf install myboot`, `nix … myboot`
— and their installer/bootloader tooling knows about it.

This is a staged plan, cheapest and highest-impact first. Nothing here needs to be
done before people can use MyBoot — they install from GitHub today (`./manage.py
install`). Packaging is about *reach and trust*.

---

## Prerequisites (do these first — they unblock every packaging channel)

Packagers will not touch a project that isn't ready to be packaged. Get these in
order and every later step becomes easy:

1. **Public Git repo with a clear name and license.** MyBoot is dual MIT/Apache-2.0
   (good — permissive licenses are what distros expect). Publish it.
2. **Tagged releases with prebuilt assets.** Each release ships `myboot.efi`, the
   `myboot` host CLI per arch, and a **`myboot.efi.sha256`** (the installer already
   verifies this). Semantic version tags (`v0.1.0`).
3. **Reproducible build from source.** A packager must build it from a release
   tarball without your dev shell. Document the exact `cargo build … --target
   x86_64-unknown-uefi` and toolchain/MSRV. The Nix flake helps here.
4. **A man page and `--help`** for the host CLI, and a one-line project description.
5. **A stable on-disk contract**: installs to `\EFI\MyBoot\BOOTX64.EFI`, reads
   `\EFI\MyBoot\config.toml`, registers a `MyBoot` firmware entry. Document it so
   packaging scripts and other tools can rely on it.

---

## Stage 1 — Channels you control (do now)

These need no external maintainer's approval.

- **GitHub Releases** — already the install source. Keep assets + checksums current.
- **A one-line web installer** — `curl -fsSL https://…/install.sh | sudo sh` on
  Linux, `irm https://…/install.ps1 | iex` on Windows (self-elevates instead of
  `sudo`) — that downloads the release `myboot.efi` and runs the install (rustup's
  model). Host both from the repo.
- **Nix flake output** — you already have a flake; expose `packages.myboot` and an
  overlay so Nix users get it with `nix profile install github:you/myboot`. This
  also gives NixOS users the declarative `boot.loader.external` path (the correct way
  on NixOS).

---

## Stage 2 — The AUR (fastest path to a real distro repo)

Arch's **AUR** is the lowest-friction "real" repository: no distro-maintainer
gatekeeping, you can publish it yourself today.

- Publish two packages: **`myboot`** (builds from a release tag) and
  **`myboot-bin`** (drops in the prebuilt `myboot.efi`). Most Arch tooling and users
  will find them via `yay -S myboot` / the AUR web search.
- Write the `PKGBUILD` (depends: `efibootmgr`; makedepends: the Rust toolchain).
  Install the binary to a standard path and ship the host CLI + man page.
- Once it has users and votes, it's a candidate for the official **`extra`** repo via
  a Trusted User — that's Stage 4 for Arch.

This is also where you'd add MyBoot to the ArchWiki "Boot loaders" comparison page,
so people discover it alongside GRUB/systemd-boot/rEFInd.

---

## Stage 3 — Fedora Copr and an Ubuntu PPA (self-serve, per-distro)

Still self-serve, but they build and host real `.rpm`/`.deb` packages users can add:

- **Fedora Copr** — build an RPM in Copr (`dnf copr enable you/myboot`). Later,
  submit to Fedora proper via a package review (Stage 4).
- **Ubuntu/Debian PPA (Launchpad)** — build a `.deb` in a PPA
  (`add-apt-repository ppa:you/myboot`). Later, an ITP (Intent To Package) bug and a
  Debian sponsor gets it into Debian, and it flows to Ubuntu.
- **openSUSE OBS (Open Build Service)** — builds packages for *many* distros at once
  from one spec; a very efficient way to cover openSUSE, Fedora, and Debian family
  together.

---

## Stage 4 — Official distro repositories (needs a maintainer/sponsor)

The real "it's in the distro" milestone. Each has a process; all of them require the
Stage-1 prerequisites and usually an existing self-serve package (Stage 2/3) with
users:

- **Arch `extra`** — a Trusted User adopts the AUR package.
- **Debian** — file an **ITP** bug, find a Debian Developer sponsor, get the package
  into `unstable` → `testing`; Ubuntu then syncs it.
- **Fedora** — submit a **package review** request; an approved packager sponsors it
  into the Fedora repos.
- **openSUSE** — submit from OBS to `Factory`.
- **Gentoo** — an ebuild in the GURU overlay first, then the main tree.

---

## Stage 5 — Show up as a *bootloader choice* in installers (the real ask)

Being in the repo is necessary but not sufficient; you want MyBoot offered where
users pick a bootloader. This is per-installer and comes after packaging:

- **Arch** is easy: its install is manual, so a packaged `myboot` + a wiki entry is
  effectively "choosable." Provide a copy-paste install snippet (see
  `INSTALL_GUIDE.md`).
- **Calamares** (used by many distros' graphical installers) has a `bootloader`
  module; getting MyBoot recognized there puts it in front of a lot of installs.
- **Fedora/Anaconda, Ubiquity/Subiquity, YaST** each have their own bootloader
  handling — realistically a later goal, pursued once MyBoot is packaged and proven.
- **`os-prober`/BLS interop** — ensure other tools can hand off to and from MyBoot
  cleanly (it already chainloads their loaders; document the reverse).

---

## Suggested order (highest leverage first)

1. Publish the repo + tagged releases with `myboot.efi` + `.sha256` (unblocks all).
2. Ship the `curl | sh` installer and the Nix flake output.
3. **AUR `myboot-bin` + `myboot`** — first real repo, self-serve, Arch users find it.
4. Fedora Copr + Ubuntu PPA (or one openSUSE OBS project covering several distros).
5. Pursue official inclusion (Debian ITP, Fedora review, Arch `extra`) once there are
   users and votes.
6. Installer integration (Calamares first).

Keep the on-disk contract and CLI stable across all of this — packagers and installer
modules depend on it not moving.
