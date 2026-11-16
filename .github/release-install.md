## Installing

Each installer holds three programs: **Parley 5250**, the 5250 terminal, **Parley SQL**, SQL for IBM i (Java 11 or
later needed for SQL), and **Parley SSH**, an SSH client.

| OS | Install | Then in a terminal |
|---|---|---|
| Windows | Run the `.msi`. Parley 5250, Parley SQL and Parley SSH are in the Start menu. | Open a **new** terminal: `parley`, `parley-sql`, `parley-ssh` |
| Linux (Debian, Ubuntu) | `sudo apt install ./parley_*_amd64.deb`. All three are in the application menu. | `parley`, `parley-sql`, `parley-ssh` |
| Linux (AppImage) | `chmod +x parley_*_x86_64.AppImage`, then run it. | Link it once, for each command: `for c in parley parley-sql parley-ssh; do ln -s ~/Apps/parley_*_x86_64.AppImage ~/.local/bin/$c; done` (or `parley_*_x86_64.AppImage --sql`, `--ssh`) |
| macOS | Open the `.dmg` and drag Parley 5250 to Applications. | Link them once: `for c in parley parley-sql parley-ssh; do sudo ln -s "/Applications/Parley 5250.app/Contents/MacOS/$c" /usr/local/bin/$c; done` |

`parley --sql` starts Parley SQL and `parley --ssh` Parley SSH on every OS. The installers are not signed yet: on macOS right-click the app and
choose **Open** the first time; on Windows SmartScreen choose **More info → Run anyway**.
