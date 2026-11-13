## Alternate: make console IDs unguessable

Replace the eight-hex-digit `PANEL_ID` with a cryptographically random 128-bit value. Keep deriving the socket path from that ID, but let `PanelDir` generate and validate the longer format. A foreign user who knows only that a console closed would then have to guess its stale directory name before planting `control.sock`.

This touches ID creation and parsing in `panel-control`/`panel-contract`, plus the direct OpenCode plugin's ID handling and fixtures. The MCP bridge and session helper would keep their existing connect flow; there is no `lstat` gate at either door.

Compared with the ticket's `lstat` owner-and-mode check, this reduces the chance of an attacker finding the path, while the check rejects a planted directory regardless of whether its name was guessed or learned. Entropy also avoids a filesystem metadata check in each client, but it is probabilistic and does not verify the endpoint's owner.

The risk is compatibility: inherited identities and any persisted integrations using the current eight-character format need a transition policy. More importantly, a longer ID is not an access-control boundary; leaked or observed `PANEL_DIR` still works. That makes this a poor fit for the ticket's concrete stale-path threat and widens scope across identity generation without replacing the cheap ownership check.
