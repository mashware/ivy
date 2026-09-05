---
id: asp_staleness-comes-from-the-files-date_737kmc
slug: staleness-comes-from-the-files-date
name: >-
  The stale notice comes from the file's date and not from updated_at — and that
  is why a copied panel.json is painted as fresh
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:08:09.474Z'
updated_at: '2026-08-12T14:08:09.474Z'
last_verified: '2026-08-12T14:08:09.474Z'
file_paths:
  - crates/panel-contract/src/panel.rs
  - docs/04-contrato-panel.md
symbols:
  - 'Panel::written_at'
  - 'Panel::is_stale_at'
  - updated_at
  - STALE_AFTER
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:2f88d49d982871a2367457566cc72b098caee9f6217d84e0ed9db26e29c6b7b7'
  docs/04-contrato-panel.md: 'sha256:5af2cb170fb82f7c94d1f1a491cd2349f599e2bde0740449655e5fcc44a2e75f'
tags:
  - contract
  - stale
  - untrusted-input
  - deviation
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The decision.** A panel's age is measured by the **file's write date**, not by the `updated_at`
the producer publishes. `updated_at` is read, stored as text and painted as is, but it is not
interpreted: the box does not parse dates nor drag a calendar around.

**The two reasons, and the second one is the good one.** The cheap one: interpreting ISO 8601 with
time zones would demand a dependency in the very box that exists in order to survive rewrites. The
weighty one comes from the contract itself — **the producer clobbers the whole file on every
publication**, so the file's date *is* the date of its last publication. And that way there is a
notice **even when the producer does not write its own stamp**, which is exactly when it is most
needed: an agent that hangs half-way through publishing does not update `updated_at`, but it does
leave the file with its date.

**The trade-off, which has to be kept in mind before trusting the notice:** `mtime` **does not
survive copying the file**. A `panel.json` that arrives through a `git checkout`, an `rsync`, a
cloned repo or a freshly mounted container has today's date, so a panel whose content is six months
old will be painted as fresh. With `updated_at` that would not happen. It is accepted because the
normal case —the producer publishing in its own directory— weighs more, and because that copied file
is untrusted input anyway.

**The subtraction is signed, and it is not prudence.** A date in the future is not the rare case: an
advanced clock or a foreign file system is enough. With the naive subtraction of times that is a
panic, and **nothing that comes from outside can cause one** on a path that runs inside the frame.
Watch out for the reason that was written first —«the contract does not demand a time zone and a
producer in another zone would publish a stamp that runs ahead»—: since `updated_at` is not
interpreted, the producer's time zone makes no difference. The mechanism is still needed, but for
the other two reasons.

**And with no exceptions**, which is where D6 almost comes back in — see
[[d6-comes-back-through-the-document-not-the-code]].

## Where it lives

`crates/panel-contract/src/panel.rs`: the `Panel::written_at` field with its comment, and
`Panel::is_stale_at`. Written down as well in `docs/04-contrato-panel.md` so that the producer knows
how it is measured.
