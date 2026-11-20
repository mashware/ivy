# Ticket 164 — Windows channel shutdown contract

The ticket requires bounded channel teardown with no worker left. The draft proposes Windows pipe cancellation and owned workers, but independent review found high finding F1: existing request handlers execute synchronous callbacks and connection teardown sends Closed through a bounded inbox (crates/panel-control/src/listener.rs:675,744-753). A callback can remain running and a live full inbox can retain its sender. Pipe cancellation alone cannot bound those paths; joining would block pane closure.

Recommended response to F1: reopen the design to include cancellation-aware, bounded Windows handler completion and release delivery, covering the actual callback implementations as well as accept/read/write workers. Reassess size and delivery scope before implementation. Preserve the ticket's zero-workers-at-return requirement; do not claim it is proved by transport timeouts alone.

Alternatively, provide a free-text response if you want to change the acceptance contract to preserve the current prompt, nonjoining Drop with eventual worker completion after callbacks finish, or specify another lifecycle requirement. Such a change would explicitly revise the original no-worker-left-at-return interpretation.

This is a study run: the answer resumes design, and the revised study still stops for approval before code. No production changes or PR have been made.
