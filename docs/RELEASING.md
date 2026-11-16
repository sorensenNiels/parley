# Releasing Parley

How installers are built (R-NF-1) and how a release is made. The tool is
[cargo-packager](https://github.com/crabnebula-dev/cargo-packager) (decision D9); its settings are in
`[package.metadata.packager]` of `crates/parley-app/Cargo.toml`.

| OS | Package | Built on | Notes |
|---|---|---|---|
| Windows | `.msi` | `windows-latest` | WiX 3, downloaded by cargo-packager. |
| macOS | `.dmg` (Apple silicon and Intel) | `macos-latest` | Minimum macOS 11. |
| Linux | `.deb`, `.AppImage` | `ubuntu-22.04` | Built on the oldest supported Ubuntu, so they run on its glibc (2.35) and newer. |

Every installer holds both programs, `parley` and `parley-sql` (D26), and makes both commands work in a terminal
where the OS allows it (R-NF-2):

| Package | Commands | Menu |
|---|---|---|
| `.msi` | The install folder goes on the system PATH (removed on uninstall): both in a new terminal. | Parley 5250 and Parley SQL in the Start menu. |
| `.deb` | Both in `/usr/bin`. | Parley 5250 and Parley SQL (`linux/parley-sql.desktop`). |
| `.AppImage` | It starts `parley`; `--sql`, or a link named `parley-sql`, starts Parley SQL. The user makes the links. | Parley 5250 only. |
| `.dmg` | Both in `Parley 5250.app/Contents/MacOS`; the user links them into `/usr/local/bin`. | Parley 5250 only. |

The Windows PATH entry and Start menu shortcut are a WiX fragment in `[package.metadata.packager.wix]`. The install
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
`.dmg` on macOS. An installer can only be built on its own OS.

## Make a release

1. Make sure `docs/FEATURES.md` is up to date. CI runs only on release tags (or by hand from the Actions tab),
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

3. The tag starts the **Release** workflow and **CI**. Release builds every installer, checks that the tag
   matches the version, and **publishes** a GitHub release with them and `SHA256SUMS.txt` attached, marked latest.
   It then deletes all but the newest three published releases (their tags stay; drafts are not touched).
4. Download the installers and run the checks below on each OS. Edit the release notes on GitHub if needed. If an
   installer is broken, mark the release as a draft again (or delete it).

To try the installers without releasing, run the workflow by hand: **Actions → Release → Run workflow**. The
installers are then attached to the run as artifacts.

## Check an installer

On each OS, after installing:

- The app starts from the Start menu / Launchpad / application menu, with the Parley icon.
- `parley --version` and `parley-sql --version` print the release version in a **new** terminal (on Windows a
  new one after installing, for the PATH; on macOS after the links of `.github/release-install.md`; for the
  AppImage through its links, and `Parley….AppImage --sql --version`).
- Parley SQL is in the Start menu (Windows) and the application menu (`.deb`), and starts from there.
- Connect to an IBM i (PUB400.COM works without an account up to the sign-on screen), over telnet and over TLS.
- Close and reopen the app: the connection form still holds the last settings.
- Uninstall: the app is removed, and on Windows its folder is gone from PATH; its saved settings are left alone.

Record what was checked, and on which OS, in `docs/FEATURES.md`.

## App icon

The icon is drawn in `crates/parley-app/icons/parley.svg`. After changing it, run `make icons` (needs
`resvg`: `cargo binstall resvg`) and commit the regenerated PNG and ICO files. The PNGs are used by the
Linux packages, the macOS app and the window; the ICO is embedded in `parley.exe` and used by the Windows installer.
