# Packaging MyBoot for Fedora (.rpm, Copr → dnf)

Spec: `packaging/rpm/myboot.spec`. Path: **Copr** (self-serve) now, **Fedora proper**
(package review) later.

## Copr (self-serve)
1. Create a Copr project at https://copr.fedorainfracloud.org.
2. Upload `myboot.spec` (or point Copr at the repo + tag). Copr builds the RPM.
3. Users:
   ```sh
   sudo dnf copr enable you/myboot
   sudo dnf install myboot
   sudo myboot install --register
   ```

## Fedora proper (later)
Submit a **package review** request (Bugzilla, component *Package Review*); an
approved packager sponsors it into Fedora. See `docs/packaging/PACKAGE_REVIEW.md`.
