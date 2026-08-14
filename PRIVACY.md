# Privacy

Little Big Mouse collects no analytics and sends no telemetry. There is no crash reporter, no usage reporting, and no server operated by this project. Everything the application knows about your displays, your pointer and your layout stays on your computer.

## When it uses the network

Three cases, all of them either initiated by you or announced here:

- **Update check** (Windows only) — the application asks the public GitHub Releases API, `https://api.github.com/repos/Mgth/LittleBigMouse/releases`, whether a newer version exists. It sends no information about you or your machine beyond what any HTTP request reveals: your IP address and a `LittleBigMouse` user agent. GitHub's privacy terms apply to that request. On Linux the check is disabled entirely — your package manager owns updates.
- **Smart-TV discovery** — when you use the monitor control features, the application sends SSDP multicast on your local network (`239.255.255.250:1900`) and probes the devices that answer. Samsung Tizen sets are probed on `http://<tv>:8001/api/v2/`, Hisense VIDAA sets on their device descriptor endpoint.
- **Smart-TV control** — pairing and commands travel to the television over your local network, by WebSocket (Samsung) or MQTT (Hisense).

Smart-TV traffic never leaves your network. Nothing is relayed through a third party.

The installer additionally downloads the .NET 10 Runtime from Microsoft's `aka.ms` channel link if it is missing, and you download releases from GitHub yourself.

## What is stored, and where

**Windows**

- `HKCU\SOFTWARE\Mgth\LittleBigMouse` — layout options, monitor positions and border settings.
- `%LOCALAPPDATA%\Mgth\LittleBigMouse` — `Current.xml` (the active layout), `Excluded.txt`, and the UI log.
- `%LOCALAPPDATA%\Mgth\LittleBigMouse\samsung-tizen.json` and `hisense-vidaa.json` — smart-TV addresses and pairing tokens, encrypted (see below). Up to 5.6.0 the Hisense file was written to `%APPDATA%\LittleBigMouse\` instead; it is moved on first read and the old copy deleted.
- A Windows Task Scheduler entry named `LittleBigMouse_<your account>`, if you enable start-on-logon. It is removed with the setting.

**Linux**

- `~/.config/LittleBigMouse/` — `options.json`, `models.json`, `layouts/`, `window.json`, `samsung-tizen.json`, `hisense-vidaa.json`, `wallpaper.json`, `secrets.key`.
- `~/.local/share/LittleBigMouse/` — `Current.xml`, `Excluded.txt`, `ui.log`, `wallpapers/`.

`Excluded.txt` holds the list of applications excluded from mouse handling, as fragments of executable paths (`\steamapps\`, `/Games/`, and whatever you add). It is seeded with defaults and only ever grows by your choice. No history of what you ran is kept: nothing records which applications were seen, only which ones you excluded.

## Smart-TV credentials are encrypted

Pairing tokens for Samsung Tizen and Hisense VIDAA televisions are encrypted before they are written, so the files listed above hold ciphertext rather than readable JSON.

- **Windows** — DPAPI (`CurrentUser` scope). The key comes from your logon credentials; nothing extra is stored on disk, and another Windows account cannot read the tokens even if it can reach your profile directory.
- **Linux and other Unix systems** — AES-GCM, with a 32-byte random key in `~/.config/LittleBigMouse/secrets.key`. That file is created with owner-only permissions (`0600`), as are the token files themselves.

If a token file cannot be decrypted — a restored profile without its key, a file copied from another account — it is discarded and you are asked to pair again. Files written in clear by 5.6.0 and earlier are read once and rewritten encrypted the next time the application starts.

**What this does not protect against.** Anything running under your own account can read the key file, or ask DPAPI, exactly as this application does. Encryption at rest keeps the tokens out of backups, synced profiles, support archives and other users' hands; it is not a defence against malware already running as you.

These tokens authorise control of a television on your own network — changing input, volume, power. They are not accounts, and they carry no payment or identity data.

## What is never done

Little Big Mouse does not read your files, your keystrokes or your screen content. It handles pointer coordinates and display geometry, and — for the exclusion feature — the executable path of the window under the cursor, which it compares against your exclusion list and does not store.

Logs, layouts and diagnostics leave your machine only if you choose to copy or attach them yourself, for example to a GitHub issue.

## Removing everything

Uninstall through Programs & Features (Windows) or your package manager (Linux), then delete the directories listed above to remove the remaining settings.

Questions about any of this belong in the [issue tracker](https://github.com/mgth/LittleBigMouse/issues).
