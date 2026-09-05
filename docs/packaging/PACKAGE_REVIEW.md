# Package review & submission documents

Templates and process notes for getting MyBoot into the official distro
repositories. Use these once the prerequisites are met (public repo, tagged
releases with `myboot.efi` + `.sha256`, reproducible source build, license files,
man page). See `DISTRIBUTION_GUIDE.md` for the overall staging.

---

## Debian — ITP (Intent To Package)

File a bug against the `wnpp` pseudo-package (`reportbug wnpp`), type **ITP**:

```
Subject: ITP: myboot -- intelligent, transactional UEFI boot manager

Package name    : myboot
Version         : 0.1.0
Upstream Author : Moses (ctrl-routing-Mosesgarlic)
URL             : https://github.com/ctrl-routing-Mosesgarlic/Myboot
License         : MIT or Apache-2.0
Programming Lang: Rust
Description      : intelligent, transactional UEFI boot manager

 MyBoot discovers the operating systems on every disk at boot time (no config to
 generate), presents a menu, and chainloads the chosen OS with a transactional,
 roll-back-safe lifecycle. Its safety-critical parts are verified (TLA+, Kani).

 Why it should be in Debian: it is a maintained, permissively licensed alternative
 to GRUB/systemd-boot with auto-discovery and no config regeneration. I am the
 upstream author and intend to maintain the package; I am seeking a sponsor.
```

Then: build a policy-compliant source package (`packaging/debian/`), get a Debian
Developer to sponsor the upload to `unstable`. It migrates to `testing`; Ubuntu
syncs from Debian automatically.

---

## Fedora — package review request

Open a Bugzilla ticket, product **Fedora**, component **Package Review**:

```
Summary: Review Request: myboot - intelligent, transactional UEFI boot manager
Spec URL: https://.../myboot.spec
SRPM URL: https://.../myboot-0.1.0-1.src.rpm
Description: <as above>
Fedora Account: <your FAS name>
```

Run `fedora-review` / `rpmlint` and fix findings before requesting. An approved
packager (sponsor) reviews and sponsors you into the Fedora packagers group.

---

## Arch — moving from AUR to `extra`

No formal "request" — build users and votes on the AUR package, then a Package
Maintainer (formerly Trusted User) adopts it into `extra`. Keep the PKGBUILD clean
(`namcap` passes), split `myboot` (source) and `myboot-bin`.

---

## openSUSE — submit request from OBS

Build in your OBS home project, then `osc sr` (submit request) to `openSUSE:Factory`.
Reviewers check the spec and licensing.

---

## What reviewers will check (be ready)

- **Licensing**: SPDX-correct `MIT OR Apache-2.0`, license files shipped, no bundled
  code with incompatible licenses. (Rust crate deps: provide a vendored/audited list.)
- **Reproducible build from the release tarball**, no network during build.
- **No bundled prebuilt binaries** in the *source* package (the `-bin`/binary variants
  are separate and clearly labelled).
- **File layout & permissions**: binaries in `/usr/bin`, the EFI payload in a
  documented `/usr/lib/myboot/`, docs/licenses in the standard paths.
- **A man page** and `--help`.
- **Security posture**: MyBoot touches the ESP and firmware variables; document that
  it only writes `\EFI\MyBoot` and adds one boot entry, and never formats disks.

---

## Upstream release checklist (what each tag must carry)

- [ ] `v<semver>` tag on a green CI.
- [ ] Release assets: `myboot.efi`, `myboot-x86_64-linux`, `install.sh`, and a
      `.sha256` for each (produced by `.github/workflows/release.yml`).
- [ ] `CHANGELOG`/release notes.
- [ ] Bump `pkgver`/`Version` and checksums in `packaging/*`.
- [ ] Documented toolchain/MSRV so packagers build identically.
