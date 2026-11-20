# LittleBigMouse egui editor

Run `lbm-app` to open the editor. There is one editor per desktop session:
launching it again (including from the tray) restores and activates the existing
window without replacing its work.

Use `lbm-app --replace` after building an updated executable. The current editor
hands over its layout draft and closes before the new build opens. Unsaved layout
edits stay unsaved; this command does not write them into the saved profile. The
mouse agent keeps running. Native OSD helpers are separate and do not acquire the
editor lock.

`lbm-app --help` prints the available options. If the current editor does not
respond, replacement fails instead of forcing it closed or opening a duplicate.

On KDE Plasma Wayland, activation goes through a KWin script, since a Wayland
client cannot raise its own window; Windows uses its native window focus request.
The editors of one session talk over a local socket, a named pipe on Windows.
