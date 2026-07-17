# Hisense VIDAA projector control

The VCP view now exposes a Hisense VIDAA card for displays whose EDID vendor is
`HEC` (including the C1). Enter the projector IP; LittleBigMouse derives the
controller MAC from the network route, including across routed subnets. The
projector MAC is optional and is only used for Wake-on-LAN. Navigation keys are sent over the
local VIDAA MQTT endpoint (TLS, port 36669); Wake uses a standard broadcast
magic packet.

Before pairing, LittleBigMouse reads the projector's UPnP descriptor directly
by IP and selects the protocol from `transport_protocol`. Older RemoteNOW
devices such as the C1 (`2160`) use the static MQTT service credentials and a
`<CONTROLLER MAC>$normal` topic. Newer VIDAA devices use time-based credentials
and access tokens. If the descriptor is unavailable, the newer credential
algorithms are tried in order.

The C1 requires the private client certificate shipped in the official Android
VIDAA application. It cannot be redistributed with LittleBigMouse. Extract the
PKCS#12 file (`client_mobile_android.p12`, or `res/3R.p12` in obfuscated APKs)
and copy it to `~/.config/LittleBigMouse/vidaa-client.p12`. The password is
detected internally. The connection uses mutual TLS 1.2, then displays the
projector PIN and stores the issued VIDAA access tokens.
