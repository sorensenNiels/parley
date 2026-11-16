## Installing

Each installer holds both programs: **Parley 5250**, the 5250 terminal, and **Parley SQL**, SQL for IBM i (Java 11 or
later needed for SQL).

| OS | Install | Then in a terminal |
|---|---|---|
| Windows | Run the `.msi`. Parley 5250 and Parley SQL are in the Start menu. | Open a **new** terminal: `parley`, `parley-sql` |
| Linux (Debian, Ubuntu) | `sudo apt install ./parley_*_amd64.deb`. Both are in the application menu. | `parley`, `parley-sql` |
| Linux (AppImage) | `chmod +x parley_*_x86_64.AppImage`, then run it. | Link it once: `ln -s ~/Apps/parley_*_x86_64.AppImage ~/.local/bin/parley` and `ln -s ~/Apps/parley_*_x86_64.AppImage ~/.local/bin/parley-sql` (or `parley_*_x86_64.AppImage --sql`) |
| macOS | Open the `.dmg` and drag Parley 5250 to Applications. | Link it once: `sudo ln -s "/Applications/Parley 5250.app/Contents/MacOS/parley" /usr/local/bin/parley` and `sudo ln -s "/Applications/Parley 5250.app/Contents/MacOS/parley-sql" /usr/local/bin/parley-sql` |

`parley --sql` starts Parley SQL on every OS. The installers are not signed yet: on macOS right-click the app and
choose **Open** the first time; on Windows SmartScreen choose **More info → Run anyway**.
