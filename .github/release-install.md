## Installing

There are three programs: **Parley 5250**, the 5250 terminal, **Parley SQL**, SQL for IBM i (Java 11 or later needed
for SQL), and **Parley SSH**, an SSH client. Install the ones you want.

| OS | Install | Then in a terminal |
|---|---|---|
| Windows | Run the `.msi` and choose the programs on its Custom Setup page (all three are ticked). They are in the Start menu; Settings ▸ Apps ▸ Parley 5250 ▸ Modify adds or removes one later. | Open a **new** terminal: `parley`, `parley-sql`, `parley-ssh` |
| Linux (Debian, Ubuntu) | One package per program: `parley_*_amd64.deb` (Parley 5250), `parley-sql_*_amd64.deb`, `parley-ssh_*_amd64.deb`. All three: `sudo apt install ./parley*_amd64.deb`; one: e.g. `sudo apt install ./parley-ssh_*_amd64.deb`. They replace an earlier all-in-one package. | `parley`, `parley-sql`, `parley-ssh` |
| Linux (AppImage) | `chmod +x parley_*_x86_64.AppImage`, then run it. | Link it once, for each command: `for c in parley parley-sql parley-ssh; do ln -s ~/Apps/parley_*_x86_64.AppImage ~/.local/bin/$c; done` (or `parley_*_x86_64.AppImage --sql`, `--ssh`) |
| macOS | Open the `.dmg` and drag the apps you want, Parley 5250, Parley SQL and Parley SSH, to Applications. (Before this release all three were in Parley 5250.app.) | Link them once: `sudo ln -s "/Applications/Parley 5250.app/Contents/MacOS/parley" "/Applications/Parley SQL.app/Contents/MacOS/parley-sql" "/Applications/Parley SSH.app/Contents/MacOS/parley-ssh" /usr/local/bin/` (the ones installed) |

`parley --sql` starts Parley SQL and `parley --ssh` Parley SSH on every OS. The installers are not signed yet: on macOS right-click the app and
choose **Open** the first time; on Windows SmartScreen choose **More info → Run anyway**.
