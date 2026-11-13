# Security delivery plan review: #57

No blocker found in the proposed single-PR boundary. Rust inherited clients, the direct JS plugin, their refusal tests, and security documentation implement one invariant; splitting them or separating guards from callers would leave a known consumer able to connect through an unvalidated inherited path. Keep the atomic PR.

Actionable rollout requirement: treat release of all three consumers as one security rollout. A merged PR alone does not close the exposure while old MCP/helper binaries or the old OpenCode plugin remain in use. Publish/upgrade them together where possible, and record which versions still lack the guard if rollout is staged. The design should add this operational note to the release/handoff checklist; no wire-format migration is needed, and guarded clients remain compatible with existing PanelDir/socket producers.

Rollback is intentionally simple but restores the vulnerability. If rollback is needed, revert the complete PR (including docs and tests) as one unit; avoid reverting only JS or Rust, which creates inconsistent protection. Note the restored exposure in the rollback record and prioritize redeploying the complete fix. No feature flag is warranted because the check is fail-closed and preserves existing protocol shapes.

The design already discloses the remaining check-to-connect pathname race and non-sticky writable ancestor risk. The ownership/mode check is therefore a targeted defense against planted inherited directories, not peer authentication; retain that limitation in release notes and do not describe the rollout as eliminating pathname attacks generally.
