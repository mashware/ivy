# Plan proposal #56: PR split

**Two PRs, one wave.** The ticket holds two separate things:
- Five fixes for behaviour that is wrong today (items 2–6).
- One new feature: the outside label (item 1). It is the biggest piece, and it has a manual proof that needs a real Mac.

Nothing in the fixes needs the label, and the label does not need the fixes. So they go side by side in the same wave.

| n | wave | depends_on | includes | standalone-mergeable | unlocks | risk if alone on main indefinitely | lines_est | files_est |
|---|------|------------|----------|----------------------|---------|-------------------------------------|-----------|-----------|
| 1 | 1 | — | Item 3: the `Done(C)` hold state in `panel/driving.rs`. `done` hands the panel back to the file, the holder can re-claim it, `patch` works only on the holder's own document, retirement still works after `done`, and the held-only warning and backed-claim rule become «held and not done». Item 2: `endpoint::socket_present`, which gates `instructions_for` in `panel-mcp`. Item 4: the session helper and `ivy-session` use `whole_identity`. Item 5: `base_dir` is made public and used for the rc dir. Item 6: the Connect guide asks for the manual prompt only when it is needed, and the prompt and README carry the subagent rule. Amends D43 and the stat part of D88. Tests for AC5–AC12. | **yes**. Every item is a self-contained fix with no new contract and no dead code. | nothing. #2 does not need it. | none. Every fix works on its own, and the label not existing is today's behaviour. | 550 | 14 |
| 2 | 1 | — | Item 1. In term-core: `descends(pid, birth)`, plus a new `foreground/macos.rs` that gives `ForegroundScope` fields of its own on macOS. In panel-control: `peer/` split into `linux.rs` and `macos.rs` (`LOCAL_PEERPID` plus start time). The peer is now captured for every connection, plus `Services.origin` and `What::Publish.outside`, with the verdict worked out once on the connection thread. In app: the scope cell for the label (built like `resume::Route`, set at spawn and replaced on restart), `PanelState.published_by`, and the label text in `warning()`. Amends D7 and the label part of D88. Tests for AC1–AC4 on Linux, and a manual AC1/AC2 run on a Mac. | **yes**. Every new piece has a caller in the same PR. Any other target compiles and answers «cannot tell», which paints no label. | — | none. «Cannot tell» paints nothing, and nothing is gated on the label. | 900 | 16 |

## Criteria claimed

- **#1 → criterion 1.** It fixes things broken today:
  - a file producer stays refused until the bridge exits;
  - a closed console is still promised a panel;
  - the helper uses a different identity rule;
  - a relative `TMPDIR` gives a relative rc dir;
  - the guide prompt is misleading.
  Bundling these with #2 would hold them behind the macOS port and the real-Mac proof.
- **#2 → criterion 1, the other way round.** It is the feature that #1 must not wait on. Nothing more splits off it.

## Merge test

- **#1 + #2, same wave: kept apart.** Merging them would put five fixes behind the largest review in the ticket and a manual step on another machine (criterion 1).
  - Cost of keeping them apart: one small rebase. Both touch `driving.rs`: `Holder` for #1, `published_by` for #2. Both also touch D88, in different paragraphs.
  - Suggested order: #1 first. It is smaller, and #2 rebases onto it.
- **Inside #2, term-core/panel-control vs app: merged.** The lower layers have only one consumer, so they are scaffolding for it. On their own they would be dead code on main.
- **Inside #2, Linux vs macOS: merged.** The user chose «Linux and macOS» as one decision.
  - Shipping Linux first would land the label on one platform and leave a silent «cannot tell» on the Mac DMG.
  - That is a visibly half-built feature, not an asymmetric safety gain.
  - If the real-Mac proof turns out to be the bottleneck, revisit this split then.
- **Inside #1, items 2, 3, 4, 5, 6: merged.** They are all part of #56 and none closes a ticket of its own. No item depends on another, so splitting them would only add review rounds.
  - Item 3 is the heaviest. It still fits: `driving.rs` has 565 lines today, tests included.

## Shape guard (D28)

- **`listener.rs` is 1 688 lines, tests included.** #2 adds the per-connection peer capture and the verdict there. Check its production count against the 600-line cap before building. If it goes over, move the verdict into its own module inside #2.
- **`lib.rs` must not grow.** The label's cell belongs in `control/channel.rs` and `resume.rs`, not `lib.rs`.
