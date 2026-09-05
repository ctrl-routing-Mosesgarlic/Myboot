# Packaging MyBoot for Arch Linux (AUR → pacman)

The AUR is the fastest way to make `pacman`/`yay` users able to install MyBoot. Two
packages: **`myboot`** (builds from source) and **`myboot-bin`** (prebuilt from a
GitHub release). Files live in `packaging/aur/`.

## One-time: publish to the AUR
1. Create an AUR account and add your SSH key (https://aur.archlinux.org).
2. Clone the (empty) package repo and add the PKGBUILD:
   ```sh
   git clone ssh://aur@aur.archlinux.org/myboot-bin.git
   cp packaging/aur/PKGBUILD-bin myboot-bin/PKGBUILD
   cd myboot-bin
   # fill in real sha256sums from the release .sha256 files, then:
   makepkg --printsrcinfo > .SRCINFO
   git add PKGBUILD .SRCINFO && git commit -m "myboot-bin 0.1.0" && git push
   ```
   Repeat with `packaging/aur/PKGBUILD` for the source package `myboot`.

## On every release
- Bump `pkgver`, update `sha256sums` (from the release `*.sha256`), regenerate
  `.SRCINFO`, commit and push. (Automate later with a release hook.)

## How a user installs
```sh
yay -S myboot-bin        # prebuilt, fastest
# or
yay -S myboot            # builds from source
sudo myboot install --register     # place on the ESP + add the firmware entry
```

## Discoverability
Add MyBoot to the ArchWiki **Boot loaders** page so people find it beside GRUB,
systemd-boot, and rEFInd, and link this guide.
