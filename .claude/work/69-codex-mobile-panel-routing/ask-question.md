Ticket 69 study: decide how to handle the unresolved compatibility evidence.

The proposed single guidance PR would preserve Ivy’s --no-daemon safeguard and update README, Connect manual instructions, the copied setup prompt, existing integrated-chrome checks, and the existing decision rationale. It would explain the reported active-writer conflict and distinguish remote-control connected status from successfully opening and using a live conversation.

The independent design review found two high-severity gaps:
1. The ticket permits a limitation fallback if the installed Codex version cannot support both functions. That condition has not been established: exact Codex 0.167.0 help and generated schemas confirm flags and generic per-thread config, but do not prove or disprove a compatible ordinary-launch path. Invocation-scoped MCP configuration remains an untested candidate.
2. No real two-console start/resume or phone test has been performed. The Linux conflict is owner-reported; capability checks cannot be published as runtime proof or a general version limit.

Recommended response to both: reopen the design, continue investigating the supported target-version CLI/MCP configuration path before choosing a delivery, keep all runtime acceptance checks open, retain the safeguard and user configuration, and scope every compatibility statement to its evidence. A guidance-only delivery must not be treated as an approved replacement for the original runtime criteria unless you explicitly choose that scope.

Select “Apply the recommended response to each” to retain the original compatibility investigation. Or provide a response such as “Approve the guidance-only delivery with joint runtime support explicitly unresolved,” or F1/F2-specific responses. This answer applies only to the study design and authorizes no code, configuration change, push or publication.
