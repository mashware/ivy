# Gentle native conversation — client and platform evidence

This is the first native-conversation delivery of [#280](https://github.com/mashware/ivy/issues/280). Cross-project Board navigation, artifact review and feedback remain the next delivery. No conversation persistence or reopening is added.

## Observations

| Environment | Exact client | Observed result | Boundary |
|---|---|---|---|
| Linux, Ubuntu 26.04, Node 24.14.1 | Gentle Shell 5.1.0 / Pi 1.6.1 | Two real isolated processes emitted text deltas, final messages and todo tool activity; numeric todo IDs and declared state came from the snapshot owner; a follow-up retained its session ID while its neighbor stayed unchanged | Deterministic loopback model, temporary homes, offline provider configuration; no subscription account exercised |
| Linux graphical host | Same pinned runtime | Desktop interaction pending | Virtual X11 driver prepared; process evidence alone does not prove desktop input |
| Older/missing snapshot capability | Production decoder/monitor fixtures | No optional read before advertised support; foreign/late responses ignored; rejected/malformed/timed-out observations preserve last-known values as unavailable | Fixtures establish receiver behavior, not an older installed runtime |
| macOS | Not exercised in this delivery | Native desktop result unverified | Compilation and Linux fixtures cannot establish macOS interaction |
| Windows | Not exercised in this delivery | Native desktop result unverified | Existing `.cmd` launcher limitation remains; an executable discoverable in an absolute desktop PATH is required for native launch |
| Official Claude Code / optional Gentle AI | Not exercised in this delivery | Login and client/platform acceptance remain pending | Clients own authentication; no credentials are bridged or provider keys injected |

The complete epic remains open until the review journey and its required client/platform evidence are accounted for. Unsupported interactions can use **New Gentle session** in an ordinary console. Capability names alone never establish a live owner.

## Receiver and display limits

- A process owns each native conversation and its private RPC pipes; session selection does not change another process's destination.
- The retained transcript and its UI cache each have a 2 MiB row/text budget and 4,096-row bound. Row metadata is charged; changed immutable chunks share a 64 KiB window-wide frame budget and sessions rotate fairly.
- Adjacent text deltas form bounded chunks. Final assistant content replaces the partial message. Non-text content is identified without guessing an image or thinking renderer.
- Display folding precedes both bounded semantic control handoffs. Control saturation becomes an explicit connection failure, including when the GUI is not draining; terminal state does not require a free display or control slot.
- Optional todo/activity snapshots use correlated `get_snapshot` reads with `sections: ["todo", "activity"]`. The receiver accepts at most 200 tasks, within the ordinary 128 KiB desktop envelope limit, and retains producer-issued numeric IDs, titles, states and truncation.
- Native polling and conversation painting are named subspans of integrated chrome in the frame report; they remain included in its existing subtraction once. No speed improvement is claimed from fixture timings.

## Reproduction and remaining checks

The work evidence records the pinned runtime installation, deterministic two-process probe, native frame-input regression, targeted mutations and desktop captures. Use temporary HOME/configuration directories to keep accounts and layout isolated. Do not substitute a successful build or package for a native desktop observation.

Remaining platform checks: launch from desktop discovery, a directory containing spaces, missing/older runtime, child loss, ordinary-console fallback and official-client login on each named OS. Results not observed stay unverified.
