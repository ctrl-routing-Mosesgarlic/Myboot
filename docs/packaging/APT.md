# Packaging MyBoot for Debian / Ubuntu (.deb, PPA → apt)

Files live in `packaging/debian/`. Two paths: a **Launchpad PPA** (self-serve, users
add it and `apt install`) now, and **Debian proper** (ITP + sponsor) later.

## Build a .deb locally (test the packaging)
```sh
sudo apt install devscripts debhelper cargo rustc efibootmgr
cp -r packaging/debian debian
dpkg-buildpackage -us -uc -b
sudo apt install ../myboot_0.1.0-1_amd64.deb
```

## Publish via a Launchpad PPA (self-serve)
1. Create a Launchpad account + PPA, add your GPG key.
2. Build a **source** package (`debuild -S -sa`) and `dput ppa:you/myboot ...`.
3. Users then:
   ```sh
   sudo add-apt-repository ppa:you/myboot
   sudo apt update && sudo apt install myboot
   sudo myboot install --register
   ```

## Debian proper (later)
File an **ITP** (Intent To Package) bug against `wnpp`, find a Debian Developer
sponsor, upload to `unstable`; it migrates to `testing` and Ubuntu syncs it. See
`docs/packaging/PACKAGE_REVIEW.md`.
