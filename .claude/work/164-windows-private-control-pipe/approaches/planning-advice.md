# Planning advice

## Recommendation: one atomic PR

| n | wave | depends_on | includes | risk if left alone | lines_est | files_est |
|---|---|---|---|---|---:|---:|
| 1 | 1 | none | Secure PanelDir creation and inherited ACL/owner checks; `PANEL_SERVER_PID` launch/forwarding contract; common transport facade; Windows overlapped named-pipe bind/connect/read/write, DACL, remote rejection, first-instance ownership, PID verification and stop/cancel/drain; listener worker ownership; capability/MCP discovery; native and Unix regression tests, CI/manual proof. | No partial state: merged delivery exposes Windows capability only after all checks and bounded teardown are active. | ~1,600 | ~25 |

The endpoint, its ACL, the client PID check, capability discovery, and its lifetime form one security boundary. Each can compile or be hidden behind a flag independently, but none has enough product value to justify a separate merge; enabling any subset risks advertising an unusable channel or leaving an exposed one. The design explicitly rejected “enable BINDS first then add ACL/authentication.” PID and directory handling also require coordinated changes across `panel-contract`, launch environment, client APIs, and guides. Keep the scope in one PR rather than splitting for scaffolding.

## Merge test and exposure audit

A one-PR plan has no same-wave pairs or dependency links to merge-test; the atomic choice is itself the result of evaluating the candidate cuts. Combining transport implementation with ACL/PID verification loses no independent delivery: the former without the latter is an unsafe endpoint. Combining capability/MCP discovery with transport loses no independently usable feature: discovery before native bind can promise a nonexistent channel. Combining private directory hardening with the endpoint avoids a separate, unvalidated Windows-only hardening change and ensures its creation and inherited checks agree.

No intermediate endpoint is exposed because there is no intermediate merge. On the final merge, bind must fail closed on descriptor, first-instance, or PID errors; only then may `BINDS` and MCP instructions advertise support. Principal risk is implementation complexity in overlapped I/O and shutdown races, requiring native Windows CI for the actual DACL, cancellation and zero-worker assertions. Manual intended-pane painting remains an acceptance proof, not a reason to split.
