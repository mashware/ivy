# Operations approach

Put one Unix ownership check in `panel-control::endpoint::private_to_this_user(dir)`. Use `symlink_metadata`/`lstat`, require the entry to be a directory, compare its uid with `geteuid()`, and reject any `mode & 0o077 != 0`. A missing path gets a distinct gone error naming `PANEL_DIR`; other failures say the path is “not a private directory of this user.” Keep it a no-op on Windows until channel binding exists there.

Call it at inherited identity boundaries only: `Producer::of_this_pane` before `endpoint::connect`, and `session::helper::exchange` before connecting. Do not put it in generic `connect` or `Producer::of`, which can serve explicitly selected endpoints. In `ivy-session`, propagate `exchange`’s actual error text instead of replacing it with a generic reachability message.

In `ivy-opencode.tsx`, synchronously `lstat` and check uid/mode at the start of every `connect()` attempt, before `createConnection`. On refusal, record/report no connection and schedule the existing one-second retry; that retry must rerun the check, so a directory that later becomes private can recover. Disposal sets `stopped` and clears both retry timers, preventing retries after cleanup.

The main risk is check-then-connect: a directory can be replaced in the tiny interval between validation and opening the socket. The ticket accepts that race; pinning directory identity through socket open would expand this change substantially. The bad idea is checking a symlink’s target with `stat`: a planted symlink to a legitimate private directory would pass despite the inherited path no longer naming the owned directory.
