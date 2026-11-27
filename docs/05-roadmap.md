# Ivy roadmap

[Tracking epic #280](https://github.com/mashware/ivy/issues/280) · [Feasibility study](gentle-workspace-study.md) · [Previous roadmap, archived](05-roadmap-historical.md)

This is the active delivery sequence. The previous plan remains available for milestone history, toolchain references and measurements. Its unfinished milestones are outside this sequence. Existing platform fixes continue under their own issues.

# Part I — What each phase delivers

1. **Answer a helper while reviewing.** With two projects open, see which helper is waiting, read its question and answer it without losing your place in a diff.
2. **Work with Gentle from Ivy.** Start a Gentle session in the chosen directory, compose a request, read its conversation and follow delegated work in native views. Open an ordinary console alongside it.
3. **Review work across projects.** Find waiting agents on the board, inspect the plan, annotate evidence and return review comments to the session that produced the change.
4. **Use your existing clients and account.** Run official Claude Code next to Gentle, with optional Gentle AI workflows. See which interactions each installed client and platform actually support.

# Part II — How it gets built

## Phase order and state

| Phase | State | Depends on |
|---|---|---|
| Native question and reply | pending | |
| Native Gentle session | pending | Native question and reply |
| Cross-project review | pending | Native Gentle session |
| Client and platform validation | pending | Cross-project review |

## Native question and reply

- [ ] Capture a real question, reply and continuation from the documented interactive RPC host. Record runtime version, request lifetime and available acknowledgements.
- [ ] Exercise two sessions asking simultaneously. Submit from the inactive project, switch focus, cancel a question and disconnect during a reply. Keep uncertain delivery visible and suppress repeat submissions.

Done when: the intended helper continues once, the other remains waiting, and the reviewer retains the same file, hunk and position. Record any upstream limitation that prevents this result before scheduling dependent work.

## Native Gentle session

- [ ] Add a native session view with a composer, transcript, streamed activity and explicit ended/disconnected states.
- [ ] Expose tasks, helpers, questions and supported stop controls. Show missing capabilities and offer the appropriate client interaction for restricted permissions.

Done when: a user can assign work, observe delegation, handle a question and see completion from Ivy. A noisy or terminated session leaves the neighboring console responsive.

## Cross-project review

- [ ] Project producer-declared attention onto the existing board with stable session and helper identities.
- [ ] Open a session's Markdown, image and Git changes in existing Ivy viewers. Bind reviewed comments to their original session and repository, including after a focus change.

Done when: a reviewer handles a request from project B during review of project A, returns to the same hunk in A and sends feedback only to A. A closed target is shown as unavailable.

## Client and platform validation

- [ ] Verify official Claude Code login inside Ivy with an existing subscription and with optional Gentle AI configuration. Publish a client matrix covering attention, questions, controls, review and usage.
- [ ] Exercise the journey on Linux, macOS and Windows. Include native desktop launches, paths containing spaces, loss of the child process and an older Gentle runtime. Track Windows panel and hook prerequisites in [#185](https://github.com/mashware/ivy/issues/185) and [#278](https://github.com/mashware/ivy/issues/278).

Done when: each released capability has a named client, version, platform and observed result. Unsupported combinations offer a working console path. The published walkthrough distinguishes the Gentle experience from the official Claude Code experience.
