# Privacy

Little Big Mouse processes display topology, pointer movement, application names used for user-configured exclusions, and settings locally on the user's computer. It does not collect analytics or send telemetry.

Network access occurs only for these features:

- automatic or user-requested update checks contact the public GitHub Releases API;
- user-requested Samsung Tizen or Hisense VIDAA discovery, pairing, and remote-control features communicate with devices on the local network;
- installer and portable artifacts are downloaded by the user from GitHub Releases.

Smart-TV credentials stay in the user's profile. They are protected with Windows DPAPI or, on Unix systems, AES-GCM using an owner-only local key. Little Big Mouse does not operate a service that receives those credentials.

Crash reports, diagnostics, and configuration are shared only when the user explicitly chooses to copy or submit them. GitHub's own privacy terms apply when the application checks GitHub or the user visits the project website.
