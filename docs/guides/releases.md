# Releases

Release preparation uses [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/)
and [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html). A breaking `!` or
`BREAKING CHANGE` / `BREAKING-CHANGE` footer selects MINOR at 0.x and MAJOR from 1.0.0 onward; otherwise `feat` selects
MINOR and `fix` selects PATCH. Other types do not increment. The largest increment
in the selected range is applied once. Normal 0.x releases never cross to 1.x.

Use a clean checkout with full history, all release tags, Cargo/npm lockfiles and
the public keys needed to verify the history. The command rejects unsigned,
unverifiable or invalid commit messages. It supports stable versions; prerelease
and build-metadata versions require a separate explicit release procedure.

Preview the comparison revisions, current/next versions, affected paths and English
notes, then apply the same local preparation:

```sh
make release RELEASE_ARGS=--json
make release
```

The tool equivalents are `craftguild-standards-engine release --json` and
`craftguild-standards-engine release --apply`. Neither creates commits/tags, pushes,
publishes, or installs anything. Changes come from literal paths configured for
the group. A nonconventional merge message is an error; squash/prepare a compliant
signed history before release, within the request's authorization.

A normal first release has no baseline tag: all scoped history is included and the
configured `initial_version` is retained without a second increment. Standards
starts at **v0.1.0**. Later preparation compares the highest reachable group tag
with the observed full HEAD. No bumping changes means no file changes.

`[[releases]]` declares the version authority and synchronized Cargo/npm ownership;
the first listed manifest owns the version. See the
[design and consistency rules](../design/releases.md) and the repository's
[`craftguild.toml`](../../craftguild.toml). npm dependency versions remain separate.
The independent sample defaults to its backend group; its frontend release is
selected explicitly with `make release RELEASE_GROUP=frontend`.

If an independent test or example workspace consumes local packages through path dependencies, list its lockfile in the owning Cargo declaration's `additional_lockfiles` array.
For example, `additional_lockfiles = ["tests/consumer/Cargo.lock"]` keeps that consumer's selected local package versions synchronized without changing its own version or external dependency versions.
Review these paths in the normal preview and receipt; do not repair prepared lockfiles by hand.

Generated notes and a deterministic JSON receipt go into the group's public notes
directory. Repeating preparation verifies those exact files and reports the same
pending release. It also works after committing only the prepared edits. New source
work while a release is pending is rejected; finish that release or explicitly
revert its preparation first. This includes documentation embedded in the engine:
restore the pending version/notes/receipt edits before committing the additional
source, then prepare again and qualify the new final revision. Preserve previous
release tags, assets and notes. Never overwrite a release tag.

Review and sign the prepared changes with the approved personal OpenPGP key, verify
the full commit, then build the final SBOM/binary from that clean commit. Run the
required checks and anonymous-clone acceptance before creating the authorized
release tag. Stop immediately on actual signing/verification failure. Isolated
tests exercise failure handling with disposable test resources and keys.


Explicit `release --major` selects current major + 1 with minor and patch zero,
including first releases (using `initial_version`), empty post-tag ranges and
non-bumping history. It overrides increment selection only: message/signature,
full-history, ownership, dirty-input and tag protections still apply. Breaking
explanations and full commit identities remain in notes. Use `--major` for preview,
apply and every retry, for example `make release RELEASE_ARGS="--major --json"`
then `make release RELEASE_ARGS="--major --apply"`. Receipt schema 2 records
`mode` (`normal` or `explicit-major`) and the effective `bump`; initial normal
preparation has no increment. Conflicting pending modes fail before writes.
Historical receipts do not authorize an implicit 0.x to 1.x transition.
