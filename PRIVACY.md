# Privacy

Little Big Mouse collects no analytics and sends no telemetry. There is no crash reporter, no usage reporting, and no server operated by this project. What the application knows about your displays, your pointer and your layout stays on your computer — or, if you turn on the multi-PC mode, goes to the other computers you paired with it, and nowhere else.

## When it uses the network

Four cases, all of them either initiated by you or announced here:

- **Update check** (Windows only) — the application asks the public GitHub Releases API, `https://api.github.com/repos/mgth/LittleBigMouse/releases`, whether a newer version exists. It sends no information about you or your machine beyond what any HTTP request reveals: your IP address and a `LittleBigMouse` user agent. GitHub's privacy terms apply to that request. On Linux the check is disabled entirely — your package manager owns updates.
  - *When.* In v6 the agent (`lbm-agent`, the program in the notification area) asks a minute after it starts, then once a day while it runs, and only while **Check for updates automatically** is on and its tray icon is shown. It also asks whenever you choose **Check for update** in its tray menu, whatever that option says. **Include beta versions** decides whether pre-releases are offered; it changes nothing about what is sent. The 5.x C# application asked at startup only.
  - *Installing.* Nothing is downloaded until you click the notice that announces a new version. The installer then comes from the release's own file on GitHub (`github.com/mgth/LittleBigMouse/releases/download/…`, served through GitHub's download host) into `%TEMP%\LittleBigMouse-update\`. It is run, elevated, only if its Authenticode signature is valid and signed by SignPath Foundation (see [SIGNING.md](SIGNING.md)). Checking that signature may make Windows contact the certificate's revocation servers. An installer that fails the check is deleted unrun, and the notice offers the release page in your browser instead. The folder is emptied at the next download.
- **Smart-TV discovery** — when you use the monitor control features, the application sends SSDP multicast on your local network (`239.255.255.250:1900`) and probes the devices that answer. Samsung Tizen sets are probed on `http://<tv>:8001/api/v2/`, Hisense VIDAA sets on their device descriptor endpoint.
- **Smart-TV control** — pairing and commands travel to the television over your local network, by WebSocket (Samsung) or MQTT (Hisense).
- **Multi-PC mode** — off by default; see the next section.

Smart-TV traffic never leaves your network. Multi-PC traffic goes directly to the machines you paired, at the addresses you typed or that discovery found on your local networks. Nothing is relayed through a third party.

The 5.x installer additionally downloads the .NET 10 Runtime from Microsoft's `aka.ms` channel link if it is missing; the v6 installer downloads nothing.

## The multi-PC mode

One mouse and keyboard, on a **master** computer, drive the screens of other computers, the **remotes** (see `docs/multi-pc.md`). The mode is off until you turn it on in the Machines panel; off, nothing listens and no network socket is opened for it.

**What it opens, once you turn it on**

- **TCP 24817** (or the port you set) — the agent listens on every network interface for the other machines, and a master dials the address you type or the machine you pick.
- **UDP 24817** — discovery: the agent broadcasts a query on each of its IPv4 networks when the mode starts (and again after 1 s and 3 s), answers the queries it hears, and says goodbye when the mode stops. A datagram carries this machine's name (your host name, unless you give another), its machine id (the fingerprint of its certificate) and its port; nothing else, and no timer repeats it. What is heard is only listed as a proposal: a discovered machine is not trusted for being heard.
- **TCP 24818** — only on Windows, only if you ticked *Let another PC drive this one everywhere* in the installer, and only once an administrator has approved a master with `lbm-service trust`. This is the `LittleBigMouseInput` service, running as LocalSystem so that a master can drive the UAC prompt and the lock screen. It listens only while at least one master is approved, admits only those masters, and takes only the input stream from them.

The Windows installer adds firewall rules for TCP and UDP 24817 (and TCP 24818 with the service), limited to `lbm-agent.exe` (or `lbm-service.exe`) and to peers on the local subnet. The rules exist whether or not you use the mode; with the mode off nothing listens behind them. Uninstalling removes them.

**Who can connect**

Every link is TLS 1.3 with mutual certificates. Each machine makes its own key and self-signed certificate the first time the mode runs; there is no certificate authority and no account. Pairing is trust on first use: a machine whose fingerprint you have not accepted gets a greeting and nothing else, and you confirm the fingerprint on **both** machines before anything else crosses. A paired machine that later presents a different certificate at the same address is refused.

**What crosses a paired link**

- **The machine's description** — its name, operating system, LittleBigMouse version and role.
- **Its screens** (remote → master) — position, size, scale, orientation, refresh rate, physical size, the connector and monitor name, the raw EDID block of each monitor (which may include the monitor's serial number), and, when monitor control is on, the input currently selected on a monitor. Sent at connection and on every display change.
- **Pointer and keyboard** (master → remote) — while the pointer is on a remote's screen, and only then, the master sends that remote its pointer positions, buttons, wheel and **every key you type**, which the remote injects as its own input. Keys travel as physical key codes. When the pointer comes back, the keyboards are handed back to the master and nothing more is sent.
- **Wallpapers** (remote → master) — while a master is linked, the remote reads what wallpaper its desktop shows (Plasma's configuration, `IDesktopWallpaper` on Windows), draws a reduced copy (at most 384 pixels wide, at most 96 KiB per display) and sends it, so that the master's map can show it. The master keeps these pictures in memory and writes them nowhere. No other screen content is read or sent.
- **The clipboard** — off by default, on each machine and each way (*send* and *receive*). With sending on, every copy you make is announced to your paired machines by its formats only — text, picture, files. The content crosses only when an application on the other machine pastes it. Copies a password manager marks as secret (`x-kde-passwordManagerHint` under KDE, `ExcludeClipboardContentFromMonitorProcessing` on Windows) are not announced. A master passes a remote's copy on to its other remotes; remotes never talk to each other directly.
  - **Files** — copying files on one machine and pasting on another sends their content: the receiver fetches every file you copied, folders included, into a new folder under `Downloads/LittleBigMouse` (or the reception folder you chose), and the pasting application copies from there. Those received files stay until you delete them. A file name that would land outside that folder refuses the whole transfer.
  - Limits: 64 MiB per text or picture, 4 GiB and 10 000 entries per set of files, by default.
  - A copy received is kept out of the Windows clipboard history and the cloud clipboard.
- **Monitor input switching** — when you choose which machine a shared monitor shows, a request to switch the monitor's input (DDC/CI VCP 0x60) and the answer.
- **Rulers** (master → remote) — the measurement overlay to draw on the remote's screens, and, when you work on it there, your gestures on it back to the master.
- Keep-alive messages, every second while idle.

Everything on a link is encrypted by its TLS session. Nothing of it is written to disk, except what is listed under storage below.

## What is stored, and where

**Windows** — in `%LOCALAPPDATA%\Mgth\LittleBigMouse`:

- `options.json`, `models.json`, `layouts\` — layout options, monitor positions and border settings. Version 5.x kept these in `HKCU\SOFTWARE\Mgth\LittleBigMouse`; v6 reads that key once, imports it into these files, and leaves it as it was, so that going back to 5.x loses nothing.
- `Excluded.txt` — see below.
- `wallpaper.json`, `wallpapers\` — your wallpaper settings and the pictures LittleBigMouse composes from them.
- `network-remotes.json` and `secrets.key` — smart-TV addresses and pairing tokens, encrypted (see below). The `samsung-tizen.json` and `hisense-vidaa.json` files written by 5.x are read and imported, and left in place.
- `multi-pc.json` — whether the multi-PC mode is on, its port, the name this machine gives itself, and the clipboard options.
- `peer-identity.pem` — this machine's multi-PC certificate and private key, made the first time the mode runs.
- `peers.json` — the machines you paired: their machine id, name, address and role.
- `agent.log` (and the previous five runs' logs beside it) and `hook.log`.
- A Windows Task Scheduler entry named `LittleBigMouse_<your account>`, if you enable start-on-logon. It is removed with the setting.

With the input service installed, also:

- `%ProgramData%\LittleBigMouse\service\` — `trusted-masters.json` (the fingerprints of the masters an administrator approved) and the service's own certificate and key. Only SYSTEM and Administrators can read or change that folder.
- `HKLM\SOFTWARE\LittleBigMouse\InputService` — the service's port and fingerprint, while it listens, for the agent to announce to its masters.

**Linux**

- `~/.config/LittleBigMouse/` — `options.json`, `models.json`, `layouts/`, `wallpaper.json`, `network-remotes.json`, `secrets.key`, `multi-pc.json`, `peer-identity.pem`, `peers.json` (and, from 5.x, `samsung-tizen.json`, `hisense-vidaa.json`, `window.json`).
- `~/.local/share/LittleBigMouse/` — `Excluded.txt`, `agent.log` and its previous five, `hook.log`, `wallpapers/`, `kscreen-restore.json` (the screen positions to put back if LittleBigMouse stops while it has shifted them, under KWin Wayland). From 5.x: `Current.xml`, `ui.log`.
- `~/.config/autostart/littlebigmouse-agent.desktop`, if you enable start-on-logon.

`peer-identity.pem`, `peers.json`, `network-remotes.json` and `secrets.key` are created owner-only (`0600`) on Linux; on Windows they take the permissions of your profile folder.

**Everywhere**: files received through the shared clipboard, in `Downloads/LittleBigMouse` under your home folder unless you chose another reception folder.

**The exclusion list.** `Excluded.txt` holds the applications excluded from mouse handling, as fragments of executable paths (`\steamapps\`, `/Games/`, and whatever you add). It is seeded with defaults and only ever grows by your choice. To apply it, the hook reads the executable path of the application in the foreground each time the foreground changes. The agent keeps the list of the paths seen since it started, in memory, so that the exclusion editor can offer them, and writes each path the first time it is seen to `agent.log`. That log, and the five kept before it, are the only record of which applications ran; they stay on your machine.

## Smart-TV credentials are encrypted

Pairing tokens for Samsung Tizen and Hisense VIDAA televisions are encrypted before they are written, so `network-remotes.json` holds ciphertext rather than readable JSON.

- v6 encrypts them with AES-GCM, on every system, with a 32-byte random key in `secrets.key` beside them.
- Files written by 5.x are still read: on Windows the C# application protected them with DPAPI (`CurrentUser` scope), elsewhere with the same AES-GCM key file.

If a token file cannot be decrypted — a restored profile without its key, a file copied from another account — you are asked to pair again; the original file is preserved.

**What this does not protect against.** Anything running under your own account can read the key file, exactly as this application does. Encryption at rest keeps the tokens out of backups, synced profiles and support archives; it is not a defence against malware already running as you. The same holds for `peer-identity.pem`: whoever can read it can present themselves as this machine to the machines you paired.

These tokens authorise control of a television on your own network — changing input, volume, power. They are not accounts, and they carry no payment or identity data.

## What is read, and what is not

- **Keystrokes.** With the multi-PC mode off, Little Big Mouse does not hook your keyboard on Windows: its rescue shortcut is a system hotkey, which sees that one combination only. On Linux, the evdev backend opens the keyboard devices beside the mice, without taking them, and follows their key events only to know whether Ctrl is held (the Ctrl override) and which keys are down; it keeps no record of them and sends them nowhere. With the multi-PC mode on, while the pointer is on another machine's screen, the master takes the keyboards (`WH_KEYBOARD_LL` on Windows, an exclusive grab on Linux) and sends every key to that machine, as described above. No key is ever written to a file or a log.
- **Files.** Little Big Mouse reads its own files, the wallpaper pictures your desktop shows (to draw the layout map, and, in the multi-PC mode, to send the reduced copies above), and, with clipboard sharing on, the files you copy, when another machine pastes them. It reads no other file of yours.
- **Screen content.** Never captured, on any machine. The only pictures that leave a machine are the reduced wallpaper copies and the pictures you copy to a shared clipboard.
- **The clipboard.** Read only with clipboard sharing turned on, and then only when another machine pastes.

Logs, layouts and diagnostics leave your machine only if you choose to copy or attach them yourself, for example to a GitHub issue.

## Removing everything

Uninstall through Programs & Features (Windows) or your package manager (Linux), then delete the directories listed above to remove the remaining settings. Uninstalling on Windows also removes the firewall rules, the input service and `%ProgramData%\LittleBigMouse\service\` (an update keeps that folder, so that the approved masters and the service's identity survive it).

Questions about any of this belong in the [issue tracker](https://github.com/mgth/LittleBigMouse/issues).
