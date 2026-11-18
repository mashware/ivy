| Finding | Resolution |
|---|---|
| Same-UID peer authentication was excess machinery | Addressed. The unique socket in a 0700 directory remains sufficient for this coordination path; the design now uses the already-owned child PID and does not claim a same-user security boundary. |
| Synchronous waits could multiply UI stalls | Addressed. The Linux path now completes asynchronously at the app boundary, reserves identity/order while pending, routes errors through existing behavior, and specifies cancellation of late children. AC5 and planned tests cover event processing and cancellation. |
| Result-observation lifetime was underspecified | Addressed. The design starts a concrete 500 ms window at confirmed root exit, accepts positive OOM evidence early, then records unknown and releases the reference even if descendants remain. |

I found no unresolved high-severity correctness gap or contradiction in the revised design. The 500 ms cutoff can miss a late systemd result, but the design explicitly reports unknown rather than making a false attribution; that is a bounded accuracy tradeoff, not a correctness defect. The asynchronous completion/cancellation path is substantial implementation work, but it is now specified and directly covered by acceptance criteria and tests.
