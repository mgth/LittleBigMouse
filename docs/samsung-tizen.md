# Samsung Tizen monitor control

LittleBigMouse can control Samsung smart monitors such as the Odyssey OLED G8
G80SD over the local network when DDC/CI is unavailable.

## Pairing

1. Put the computer and monitor on the same LAN/VLAN.
2. Open the VCP view for the Samsung monitor.
3. Select **Discover**, or enter the monitor IPv4 address manually.
4. Select **Pair** and allow `LittleBigMouse` in the prompt displayed by the monitor.

Before pairing, enable both **Settings > All Settings > Connections > Network >
Expert Settings > Power On with Mobile** and **IP Remote**. Samsung documents
Power On with Mobile as a prerequisite for IP Remote on this monitor. **Access
Notification** belongs to the separate mobile/content-sharing device manager and
does not enable the IP-control channel.

If a pairing prompt is displayed, allow `LittleBigMouse`. Depending on the Access
Notification setting and firmware, the monitor may authorize the connection
without displaying a prompt.

The association, Wi-Fi MAC address and Tizen remote token are stored per EDID
monitor in `samsung-tizen.json` under the LittleBigMouse configuration directory.
The MAC address is obtained automatically from Samsung's device-information API;
manual entry is only a fallback. On Unix the file is created with user-only
permissions.

## Controls

The panel provides power-off, Wake-on-LAN, Home, Settings, Source and directional
remote keys. Wake-on-LAN also requires the monitor's network wake option to be
enabled; its wording and menu location vary by firmware.

Tizen's local remote channel does not expose readable brightness, contrast or RGB
levels. Picture settings can instead be automated with an explicit key macro. A
macro contains `KEY_` names and optional delays in milliseconds:

```text
KEY_MENU,700,KEY_DOWN,KEY_ENTER
```

Commas, semicolons and `+` are accepted as separators. A number changes the delay
after the preceding key; keys otherwise wait 150 ms. Delays are limited to ten
seconds. Build a macro against the monitor's current firmware and picture mode,
because Samsung can change menu order between SDR, HDR, Game Mode and firmware
versions.

## Network details

- Discovery uses SSDP multicast and falls back cleanly to manual IPv4 entry.
- Device information is read from `http://MONITOR:8001/api/v2/`.
- Remote commands use the paired TLS WebSocket endpoint on port 8002.
- The monitor uses a device-generated TLS certificate, accepted only by the client
  instance connected to the IPv4 address explicitly selected in the UI.
- Power-on sends three standard Wake-on-LAN magic packets to UDP port 9.
