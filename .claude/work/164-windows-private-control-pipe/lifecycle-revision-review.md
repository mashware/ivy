# Windows lifecycle revision review

## Finding

The owner’s answer does not make AC5 achievable by adding cancellation around pipe I/O. `converse` invokes `SessionHandler`, publisher callbacks, the plan reader, document reader, publication inspector and `wake` directly on connection workers ([listener.rs](/work/repo/crates/panel-control/src/listener.rs:45), [listener.rs](/work/repo/crates/panel-control/src/listener.rs:525), [listener.rs](/work/repo/crates/panel-control/src/listener.rs:687)). Any supplied `Fn` may block forever, so a join cannot have a finite bound. Current app callbacks include foreground ancestry inspection ([provenance_route.rs](/work/repo/crates/app/src/panel/provenance_route.rs:10)), overlay admission and authority locks ([overlay.rs](/work/repo/crates/app/src/panel/overlay.rs:195)), session routing from PTY setup ([launch.rs](/work/repo/crates/app/src/launch.rs:323)), and a restoration wake closure ([launch.rs](/work/repo/crates/app/src/launch.rs:341)). `Reader::read` also takes a blocking mutex ([document.rs](/work/repo/crates/app/src/panel/document.rs:85)). Thus arbitrary synchronous callback completion cannot be bounded while retaining these callback APIs on channel-owned threads.

## Feasible Windows boundary

Keep Unix implementation and constructors unchanged. On Windows, introduce a separate service surface made only of owned snapshot capabilities: immutable bounded document/plan snapshots published by the UI, copied through `try_lock` (busy means a retryable “temporarily unavailable” reply); fixed-size native session/inspection snapshots only where verified support exists; no user `Fn` executes on accept/read/plan-reader/inspector/session/publisher workers. Bound serialized snapshot size by the existing protocol ceiling. Preserve off-frame reads by reading these published snapshots directly, independent of UI frame scheduling or minimized state.

Use a concrete non-inheritable Windows event object for wake. Workers call `SetEvent`; the UI-side integration waits/consumes it without a closure. Make release delivery independent of the bounded publication inbox: a per-connection atomic release bitset/registry (capped at eight connections) plus the event lets `Closed` be observed even when the inbox is saturated. Shutdown signals stop, cancels and drains all overlapped operations, then joins workers; all worker code is limited to bounded parsing, try-lock snapshots and event signaling.

Refuse legacy callback-bearing Windows constructors with `Unsupported` if any callback is supplied; do not accept and silently ignore callbacks. Add explicit Windows snapshot-capability constructors. Keep native session authorization unsupported; transport availability must not imply session authority. App integration must supply the concrete event and snapshot publishers.

## Decision still required

This is a public API split and app service redesign, larger than the current M estimate. The high remaining decision is whether Windows may return temporary unavailable for a busy snapshot (including plan reads) and whether session/publisher callback services remain unavailable there until separately represented as bounded capabilities. Without that contract change, zero workers at `Drop` return is impossible to promise for arbitrary synchronous callbacks.
