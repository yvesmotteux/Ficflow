# Ficflow AUR packaging

This directory contains the source files for the [`ficflow-bin`](https://aur.archlinux.org/packages/ficflow-bin) AUR package.

The AUR repo itself lives at `ssh://aur@aur.archlinux.org/ficflow-bin.git` and is published separately — see below.

## Files

- `PKGBUILD` — the package definition.
- `ficflow.desktop` — XDG desktop entry, installed to `/usr/share/applications/`.
- The icon and LICENSE are pulled directly from the tagged release on GitHub by `makepkg`.

## Prerequisites

```sh
sudo pacman -S --needed base-devel pacman-contrib namcap
```

`pacman-contrib` provides `updpkgsums`. `namcap` is optional but recommended for linting the package.

## First-time AUR publish

1. Create an account at https://aur.archlinux.org and add your SSH public key in account settings.
2. Make sure the matching GitHub release exists with the `ficflow-linux-amd64` asset attached (the release workflow does this automatically when you push a `v*` tag).
3. From this directory, regenerate the checksums against the live release artifacts:
   ```sh
   updpkgsums
   ```
4. Test locally:
   ```sh
   makepkg -si        # builds and installs the package on your machine
   namcap PKGBUILD    # lint the recipe
   namcap *.pkg.tar.zst   # lint the built package
   ```
   Launch the app via your application menu to confirm the `.desktop` entry registers and the icon shows up.
5. Generate the metadata file AUR uses for search/indexing:
   ```sh
   makepkg --printsrcinfo > .SRCINFO
   ```
6. Clone the AUR repo and push:
   ```sh
   git clone ssh://aur@aur.archlinux.org/ficflow-bin.git /tmp/ficflow-bin-aur
   cp PKGBUILD ficflow.desktop .SRCINFO /tmp/ficflow-bin-aur/
   cd /tmp/ficflow-bin-aur
   git add PKGBUILD ficflow.desktop .SRCINFO
   git commit -m "Initial release: ficflow-bin 1.0.0"
   git push
   ```

## Per-release update

Each time you cut a new GitHub release:

1. Bump `pkgver` in `PKGBUILD` (and reset `pkgrel=1`).
2. Refresh checksums: `updpkgsums`.
3. Test: `makepkg -si`.
4. Regenerate metadata: `makepkg --printsrcinfo > .SRCINFO`.
5. Copy the updated files into the AUR repo clone, commit, push.

If you only need to change packaging (not the upstream version), keep `pkgver` and bump `pkgrel`.

## Notes

- `provides=('ficflow')` and `conflicts=('ficflow')` mean this package satisfies the `ficflow` virtual name and prevents installing alongside a hypothetical source-built `ficflow` package.
- Runtime dependencies were derived from inspecting the release binary plus the deps that `wgpu`/`eframe`/`winit` typically `dlopen`. After the first real install, run `namcap` on the resulting package — it will flag any over- or under-declared dependencies and you can adjust the `depends=()` list accordingly.
- The icon currently installs to `/usr/share/pixmaps/ficflow.png` (legacy XDG path that accepts any image dimensions). If a properly-sized 256×256 PNG or scalable SVG is added to the repo later, switch to `/usr/share/icons/hicolor/256x256/apps/ficflow.png` and add `hicolor-icon-theme` to `depends`.
