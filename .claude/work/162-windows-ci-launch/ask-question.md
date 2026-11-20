Ticket 162: approve the Windows test omission accounting approach

The study proposes one PR: full workspace clippy/tests on windows-2025, visible Unix-only omissions, and a clean-profile installed Ivy.exe launch that proves console operation, saves APPDATA/ivy/layout.json and exits 0. No code has been written. #161 is already on main.

The independent design challenge found one high-severity gap: the design promises an exact omission count but has not selected a verifiable mechanism for matching compiled tests to reported omissions. Counting source files or summing harness totals can silently hide missing coverage.

Recommended response: use the pinned Rust 1.97.1 libtest listing with a small checked-in per-test omission inventory (qualified test binary + test name + platform reason). On Windows, compare the compiled ignored-test listing against the inventory, require every declared Unix-only omission to be present and ignored, and reject unexpected Unix-only omissions. On Linux, check those inventory names against compiled real tests, allowing explicitly documented platform-specific counterparts. Report matched names, reasons and exact total, including zero. Keep unrelated existing ignored tests separately reported. Add focused reporter fixtures that prove missing, extra and duplicate entries fail. Never execute ignored placeholder bodies as behavior evidence. This supersedes the initial rejection of a manifest: it is now compiler-checked rather than a manually trusted count. No product API or persistence schema changes.

Medium findings will also be incorporated: bind the shell witness to the normal resolver's actual child executable, retain inherited PATH/ComSpec rather than substituting a helper shell, and discover the launched window by PID with bounded state waits and diagnostics before cleanup; unexpected modal dialogs fail the smoke.

Apply the recommended response, or provide a different response for finding F1 (omission accounting). The study will then finish its design and delivery plan and return for approval before implementation.
