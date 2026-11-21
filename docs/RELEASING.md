# Releasing Parley

How installers are built (R-NF-1) and how a release is made. The tool is
[cargo-packager](https://github.com/crabnebula-dev/cargo-packager) (decision D9); its settings are in
`[package.metadata.packager]` of `crates/parley-app/Cargo.toml`.

| OS | Package | Built on | Notes |
|---|---|---|---|
| Windows | `.msi` | `windows-latest` | WiX 3, downloaded by cargo-packager. |
| macOS | `.dmg` (Apple silicon and Intel) | `macos-latest` | Minimum macOS 11. |
| Linux | `.deb`, `.AppImage` | `ubuntu-22.04` | Built on the oldest supported Ubuntu, so they run on its glibc (2.35) and newer. |

Every installer holds three programs, `parley`, `parley-sql` (D26) and `parley-ssh` (D41, R-SSH-1), and makes
every command work in a terminal where the OS allows it (R-NF-2):

| Package | Commands | Menu |
|---|---|---|
| `.msi` | The user chooses which of the three to install, all ticked (R-NF-3). The install folder goes on the system PATH (removed on uninstall): those installed in a new terminal. | Each one installed in the Start menu. |
| `.deb` | One package per program (R-NF-4): `parley-5250`, `parley-sql`, `parley-ssh`, each in `/usr/bin`. | Each one installed, from `linux/<program>.desktop`. |
| `.AppImage` | It starts `parley`; `--sql` or `--ssh`, or a link named `parley-sql` or `parley-ssh`, starts Parley SQL or Parley SSH (`siblings.rs`). The user makes the links. | Parley 5250 only. |
| `.dmg` | Three apps (R-NF-5): `Parley 5250.app`, `Parley SQL.app`, `Parley SSH.app`; the user links their programs into `/usr/local/bin`. | Each app dragged to Applications. |

The `.deb` files and the macOS apps come from `scripts/package-split.sh` (D56): it gives cargo-packager one config per program, adds Replaces and Breaks to the `.deb` files with `dpkg-deb`, and puts the apps in one `.dmg` with `hdiutil`. The AppImage and the `.msi` come from `[package.metadata.packager]`. The Windows installer uses Parley's own WiX template, `crates/parley-app/windows/main.wxs` (from cargo-packager 0.11.8's, D55: one feature per program); when cargo-packager is updated, compare its `main.wxs` with ours. The PATH entry and the SQL and SSH Start menu shortcuts are a WiX fragment in `[package.metadata.packager.wix]`. The install
notes users see head every release's notes: `.github/release-install.md`.

The installers are **not signed** yet (D10). macOS will say the app "cannot be opened because the developer
cannot be verified": right-click the app, choose **Open**, then **Open** again (needed once). Windows SmartScreen
may warn: **More info → Run anyway**.

## Build installers locally

```sh
make setup        # once: installs cargo-packager with the other cargo tools
make package      # installers for this OS in target/packages/
```

`make package` builds the optimised app and packages it: `.deb` and `.AppImage` on Linux, `.msi` on Windows,
`.dmg` on macOS. An installer can only be built on its own OS. On macOS it first builds rsync for Parley SSH from
its pinned source (`scripts/build-rsync.sh`, R-SSH-66), which needs Xcode's command-line tools; Parley SSH's
`.deb` depends on the distribution's `rsync` instead.

## Make a release

1. Make sure `docs/FEATURES.md` is up to date. CI runs only by hand (from the Actions tab),
   so `make verify` locally is the check before a release; the release script runs it too.
2. Run one of these on an up-to-date, clean `main`:

   ```sh
   make release-patch   # 0.3.0 → 0.3.1: fixes only
   make release-minor   # 0.3.0 → 0.4.0: new features
   make release-major   # 0.3.0 → 1.0.0: breaking changes (e.g. the saved settings format)
   ```

   It asks first, then sets the version in the root `Cargo.toml` (`[workspace.package] version`) and
   `Cargo.lock`, runs `make verify`, commits "Release vX.Y.Z", makes the tag `vX.Y.Z` and pushes the commit and the
   tag together. If verify fails, the version is put back and nothing is committed. If only the push fails, the
   commit and tag are local and it prints the command to push them. (It runs in Git Bash on Windows.)

   By hand, the same is: set the version, commit and push, then `git tag vX.Y.Z` and `git push origin vX.Y.Z`.

3. The tag starts the **Release** workflow (CI does not run on tags). Release builds every installer, checks that the tag
   matches the version, and **publishes** a GitHub release with them and `SHA256SUMS.txt` attached, marked latest.
   It then keeps only the three newest versions: every older release, published or draft, is deleted with its
   files, and so are the older `v*` tags. Only the three newest Release runs stay in the Actions tab.
4. Download the installers and run the checks below on each OS. Edit the release notes on GitHub if needed. If an
   installer is broken, mark the release as a draft again (or delete it).

To try the installers without releasing, run the workflow by hand: **Actions → Release → Run workflow**. The
installers are then attached to the run as artifacts.

## Check an installer

On each OS, after installing:

- The app starts from the Start menu / Launchpad / application menu, with the Parley icon.
- `parley --version`, `parley-sql --version` and `parley-ssh --version` print the release version in a **new**
  terminal (on Windows a new one after installing, for the PATH; on macOS after the links of
  `.github/release-install.md`; for the AppImage through its links, and `Parley….AppImage --sql --version` and
  `--ssh --version`).
- Parley SQL and Parley SSH are in the Start menu (Windows) and the application menu (`.deb`), and start from there.
- Windows (R-NF-3): the installer's Custom Setup page lists Parley 5250, Parley SQL and Parley SSH, all ticked; with
  none chosen, Next says to choose one. Install only Parley SSH: only `parley-ssh.exe` and its Start menu entry. Then
  Settings ▸ Apps ▸ Parley 5250 ▸ Modify ▸ Change adds Parley 5250; its Tools menu and Settings ▸ Add-ons have no SQL.
  Upgrading from an earlier version keeps the install folder.
- Linux (R-NF-4): `sudo apt install ./parley-ssh_*_amd64.deb` alone gives Parley SSH and its menu entry only; then
  `sudo apt install ./parley*_amd64.deb` over an earlier all-in-one `parley-5250` replaces it without file conflicts;
  `sudo apt remove parley-5250` leaves Parley SQL and Parley SSH working.
- macOS (R-NF-5): the `.dmg` shows the three apps and Applications. Drag only Parley 5250: Tools has no SQL or SSH.
  Drag Parley SSH too, to another folder or Applications: Tools ▸ SSH… starts it. Each has its own name in the Dock.
- Connect to an IBM i (PUB400.COM works without an account up to the sign-on screen), over telnet and over TLS.
- Close and reopen the app: the connection form still holds the last settings.
- Uninstall: the app is removed, and on Windows its folder is gone from PATH; its saved settings are left alone.

Record what was checked, and on which OS, in `docs/FEATURES.md`.

## App icon

The icon is drawn in `crates/parley-app/icons/parley.svg`. After changing it, run `make icons` (needs
`resvg`: `cargo binstall resvg`) and commit the regenerated PNG and ICO files. The PNGs are used by the
Linux packages, the macOS app and the window; the ICO is embedded in `parley.exe` and used by the Windows installer.
