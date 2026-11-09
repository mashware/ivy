# Publishing Ivy releases

The application reads the public releases of [mashware/ivy](https://github.com/mashware/ivy/releases)
using [GitHub's list-releases API](https://docs.github.com/en/rest/releases/releases#list-releases).
A commit or standalone Git tag does not announce an update. The checker sends an anonymous HTTPS
request to GitHub on normal launches; no token, terminal contents or pane identity is sent.

## Before publishing

1. Set the workspace package version in `Cargo.toml` to the new semantic version and refresh
   `Cargo.lock`. Builds report that version as the installed version.
2. Update `resources/release-notes.md` with concise user-visible changes for that version. This file
   is compiled into the binary so installed notes remain available offline.
3. Validate and build that revision. Package the `app` and `panel-mcp` executables together for each
   platform actually tested. Do not claim platform artifacts or signatures that were not produced.
4. Create a GitHub release from that exact revision, using a tag such as `v0.2.0` or `0.2.0`, a clear
   title and release notes. Attach the tested downloads. Publishing is a separate explicit action;
   implementing the checker does not publish a release.

## What reaches users

Only published, non-draft, non-prerelease entries with stable semantic-version tags enter the list.
The twenty most recently published entries are sorted by numeric version precedence. Build metadata
is ignored; `0.10.0` is newer than `0.9.0`. A newer stable version adds a dot to the application menu.
A newer installed source build is never asked to downgrade.

The download button opens the version's GitHub release page. It does not guess an operating-system
asset or install anything. Empty releases, HTTP failures and failed checks remain distinct in the
in-app status. A network failure does not block the terminal or erase history already read in this
launch. A later manual check retries the same public endpoint.

Notes use the existing inert Markdown renderer. Individual release notes are bounded; truncation is
labelled and the complete notes remain on the release page. No automatic request runs under any
measurement launch, including those that opt into integrated chrome. No periodic background timer
or automatic retry is installed.
