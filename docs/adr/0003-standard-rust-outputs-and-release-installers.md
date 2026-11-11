# ADR 0003: Standard Rust outputs and release installers

Status: Accepted; standards bundle additions superseded by [ADR 0004](0004-repository-initialization-and-prime.md)

The [authenticated transport amendment](#authenticated-transport-amendment-2026-11-11)
extends the original curl-only download contract without changing packaging,
checksum verification, destination placement or uninstall behavior.

Extends [ADR 0002](0002-standards-owned-build-orchestration.md). This records the
approved design; implementation evidence is recorded in the
[verification guide](../guides/verification.md#adr-0003-source-qualification-2026-11-06).
That evidence does not certify every host or architecture. Its standard
layout mode qualifies ADR 0002's preservation of existing output locations only
for repositories that explicitly enable the mode. Other compatibility promises
remain in force.

## Context

The engine defaults to `target/<target-triple>/<profile>/<file>`, but repositories
can select other layouts through base, target and variant settings. Native
products can consequently omit the target directory while another product always
includes it. This leaves distribution tooling with several output conventions.

The standards release already provides executables, consumer SBOMs and checksums.
Its delivery packager is repository-specific. Products need the same verified
binary packaging and a small installer that runs without a checkout or build
toolchain. Copying these operations into each product would duplicate common
build and distribution responsibilities.

## Decision

### Standard layout is explicit and applies to all Rust artifacts

Configuration schema 2 gains the optional root setting:

    rust_output_layout = "standard"

The mode fixes every engine-published Rust executable and library output at:

    <target-dir>/<target-triple>/<profile>/<file>

The target triple is present for both native and cross builds. Profile remains
`debug` or `release`; the existing filename resolver supplies executable suffixes
and library names for cdylib, staticlib and rlib, including WebAssembly outputs.
An architecture name alone is not the target directory identifier.

The output root remains configurable through existing build configuration,
`CARGO_TARGET_DIR` and `--target-dir`, retaining existing selector precedence.
When standard mode is enabled, explicit Rust `output` and `output_root` settings
are rejected at base, target and variant levels, even if a supplied value would
match the standard layout. Remove those overrides when enabling the mode.
Frontend bundle directories remain governed by their existing contracts.

Build, local copy-only install and packaging use the shared output context and
verified receipts. They do not search old paths or fall back to
`target/release/<file>`. Candidate compiler caches remain internal to the engine;
the contract governs its published, verified output. Direct compiler invocations
outside the engine do not acquire this guarantee.

Repositories without the new setting retain their existing schema 1/2 behavior.
Standards itself enables the mode first; actual product migration is separate.

### Installer declarations and common generation

Schema 2 also gains optional installer declarations, for example:

    [[installers]]
    artifact = "standards"
    repository = "craftguild/standards"
    release_group = "standards"
    output = "scripts/install.sh"

Each declaration selects one executable Rust artifact, the GitHub owner/repository,
the release group owning that artifact's package, and a repository-relative script
path. Reject missing references, non-executable artifacts, duplicate artifact or
output ownership, unsafe paths and invalid repository identifiers. Multiple
commands use separate declarations and scripts. Installer declarations do not
require standard layout mode: legacy repositories can package verified receipts
while adopting that mode separately.

The common interfaces are:

    craftguild-standards-engine installer [--artifact ID] [--check]
    craftguild-standards-engine package [--group ID] --output DIR

These names follow ADR 0006's v0.5.0 CLI split; earlier releases used the single
full CLI. Make and the pinned launcher forward the same operations and arguments.
`installer` deterministically generates all declarations unless one artifact is
selected. `--check` compares without writing; ordinary `check` also verifies that
declared scripts match the generator. The template is embedded in the engine,
included in its sealed build inputs and assigned an implementation source role.
Generated shell scripts also remain measured implementation, without a new size
exception. Generation does not require a sibling standards checkout.

### Package verified outputs through the common engine

`package` selects all installer artifacts in one release group. The group may be
omitted only when the configured installers identify exactly one group. All
selected receipts must agree on release version and target and match the owning
group's version authority. The current explicit Git root must be clean and match
the full revision recorded by those clean release builds.

Reuse complete receipt verification, including sealed hashes, exact directory
inventories, SBOM bytes and output hashes. Read outputs from receipts instead of
constructing paths independently. Missing, stale, changed or mismatched evidence
fails before a final delivery directory appears. Packaging never compiles,
acquires an engine or resolves dependencies; entry dispatch requires an existing
verified cache. Cross-target receipt verification does not imply host runtime
qualification.

Write the following into a fresh output directory through temporary staging:

- `<command>-v<version>-<target>` for each executable.
- The corresponding `<command>-v<version>-<target>.bom.json`.
- `SHA256SUMS-<target>.txt`, covering the executable and SBOM files.

The standards-specific delivery wrapper reuses these common outputs and adds
role-separated release metadata under ADR 0006. The former execution/skill bundles
were retired by ADR 0004. It then computes final
checksums covering all delivery files except checksum files themselves. Keep
checksums out of intermediate asset maps that would create circular hashes.
Existing destination directories are not overwritten or incrementally merged.

Consumer SBOMs continue to contain identity and dependency/component evidence.
Source inventories, source hashes and their aggregate integrity digest remain
in producer-local seals and receipts. No new SBOM or receipt format is required.

### Generated installer contract

Each script is standalone POSIX sh. It needs curl, standard system utilities and
either `sha256sum` or `shasum`; it does not need standards, Python, jq, Git or a
build toolchain. Scripts are served as raw source, not GitHub's HTML blob view:

    curl -fsSL <Raw URL> | sh -s --
    curl -fsSL <Raw URL> | sh -s -- --version 0.2.0
    curl -fsSL <Raw URL> | sh -s -- --bindir /usr/local/bin
    curl -fsSL <Raw URL> | sh -s -- --uninstall

Supported flags are `--version VERSION`, `--bindir DIR`, `--uninstall` and
`--help`. Version selection applies to installation; reject a version selector
combined with uninstall. Invalid, duplicate or missing argument values fail
before performing work. Help requires no network or installation state.

Resolve the destination in this order: `--bindir`, environment variable `BINDIR`,
then `$HOME/.local/bin`. An explicit or environment destination must be nonempty
and absolute; support spaces without shell evaluation. HOME is only required
when computing the default. Report a missing PATH entry and an example for
adding it; never rewrite shell startup files.

Installation follows this sequence:

1. Detect the OS and architecture with `uname`, mapping Darwin/Linux and
   x86_64/amd64/arm64/aarch64 to the four supported target triples:
   `x86_64-apple-darwin`, `aarch64-apple-darwin`,
   `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`.
   Unsupported combinations fail; Linux does not guess between GNU and musl.
2. Without a version selector, resolve the repository's GitHub Latest Release
   redirect once. Validate its stable SemVer tag against the configured group's
   tag prefix, then pin all asset requests to that tag. A different group's tag
   fails with an instruction to use `--version`; do not search group-specific
   releases or require JSON tools. An explicit version is stable `X.Y.Z` without
   the tag prefix; derive the tag from the configured group.
3. Download the selected binary and its `SHA256SUMS-<target>.txt` over HTTPS to
   temporary storage. Missing assets, network errors and incomplete downloads
   fail without changing an installed binary. The runtime command name is the
   artifact command, without the release asset's version/target suffix.
4. Require exactly one well-formed SHA-256 entry for that exact asset basename
   and verify its bytes. Reject missing, duplicate, malformed or mismatched
   entries. Do not execute the downloaded binary to perform verification.
5. Stage the verified bytes on the destination filesystem, set executable mode
   0755 and rename into place. Preserve any existing binary until successful
   replacement and clean up owned temporary files on failure or interruption.

Download and checksum verification run without requesting privilege escalation.
If directory creation or destination modification requires privileges, call sudo
only for the necessary destination operations, including staging cleanup. Use
ordinary operations when writable or already root. Do not re-execute the entire
installer under sudo, pipe downloaded code into a privileged shell, or execute
the downloaded program as root. Sudo unavailability or authentication failure
ends the operation without replacing the old binary. Privileged wrapper commands
must preserve argument boundaries rather than interpolate user paths as code.

Uninstall resolves the same destination, then removes only the regular file at
`<bindir>/<command>`. It makes no network request and needs neither OS/Arch
selection nor checksum tools. Absence is successful; directories and symlinks at
the target are refused. Use sudo only when deletion requires it. Preserve all
application configuration, data, other binaries and directories. No installation
registry or package-manager integration is introduced.

### Compatibility and technical lifetime

The new download installer defaults to a user directory. Existing source-build
`make install` remains a copy-only operation with its existing `/usr/local/bin`
default and destination selectors. Runtime installation and repository-local
build entry setup remain distinct operations.

Installer stages are a transient technical sequence, not persistent business
state. Process results and temporary staging cover the invocation's lifetime;
the existing build records and receipts remain producer provenance. No durable
business transition table or Header-Detail database model is warranted. File
collections in a release are technical inventories, not business line items.

## Alternatives and consequences

Repository-specific installer and packaging scripts retain competing ownership
of naming, checksums and output discovery. The common engine owns those rules
instead, with declarative product identity. A default-only output convention
does not prevent divergence, so standard mode rejects output overrides.

A global layout change would break established products. Explicit schema 2
adoption preserves their existing behavior while allowing all Rust outputs in
an adopting repository to use one layout. Frontend contracts are outside that
layout mode.

Repository-wide Latest Release resolution keeps the installer small and avoids
JSON parsing dependencies. Repositories with multiple release groups must mark
the intended default release as Latest or use an explicit version. OS/Arch
detection alone does not certify that every platform has a published binary.
User-local installation avoids default escalation; custom destinations retain
automatic, narrowly scoped sudo when their permissions require it.

## Acceptance and rollout

- Test standard output resolution for native/cross, debug/release, executable and
  library artifacts, variants and overridden target roots. Reject prohibited
  overrides and prove configurations without the mode retain their behavior.
- Test deterministic generation, drift detection, strict declarations, shared
  Make/CLI/launcher dispatch and operation from a matching distributed engine.
- Prove packaging rejects dirty/mismatched revisions, stale receipts, added or
  changed sealed inputs, changed outputs and mismatched groups/versions/targets.
  Successful packages have exact inventories, matching SBOMs and correct hashes.
- Execute generated scripts with a real sh and checksum implementation. Cover
  four target mappings, Latest and explicit versions, group mismatches, missing
  assets, download/checksum failure, destination precedence, spaces and upgrade
  replacement. Failed install leaves the old binary intact.
- Stub network and sudo at the process boundary. Test operation ordering and
  argument handling without live services, real privilege escalation, an actual
  HOME or system installation. Uninstall must work without curl or hash tools,
  preserve settings and unrelated files, and refuse symlinks/directories.
- Run real temporary-directory install/uninstall acceptance on the available
  Linux and macOS hosts, Rust 1.85 consumer checks, required `make check` and
  implementation-only tokei 15/code-size assessment. Record unexecuted targets
  separately; this decision adds no new host runners.
- Align specifications, configuration and orchestration guides, the packaged
  skill and README in the implementation change. Document both installation
  routes and the optional layout migration without claiming an unreleased API
  is present in v0.2.0.

Implementation stays in standards. Actual product edits, user-state changes,
stage/commit/tag/push, release preparation and publication require a separate
request and are not part of the implementation goal. Preserve published v0.1.0
and v0.2.0 and retain the existing signing-stop contract for actual signed work.
Track implementation and its evidence in `bd` goal issue `standards-bgd`.

For v0.5.0 product adoption, use ADR 0006's
[I1–I8 integration criteria](0006-initializer-cli-and-controlled-engine-upgrades.md#product-integration-and-local-completion).
Standard output configuration or a passing installer generation check satisfies
only part of that contract. Final product builds, applicable acceptance and
current-source evidence remain required. Common layout, installer and packaging
failure matrices stay in standards with neutral fixtures. Local integration does
not require performing a real release or fabricating the clean HEAD and release
receipts needed by delivery packaging; those operations require separate authority.

## Authenticated transport amendment (2026-11-11)

Private GitHub releases require authenticated retrieval of the installer and its
assets. Add optional `transport = "github-cli"` to an installer declaration;
`curl` remains the default and unknown transport values fail configuration.
No secret belongs in `craftguild.toml` or script arguments. This is a compatible
configuration-schema-2 extension with no receipt or artifact-format change.

The generated script reuses an existing GitHub CLI login or environment
`GH_TOKEN`, requires read access, pins GitHub.com explicitly, and disables
prompts/debug output for its child commands. Resolve Latest once through
`gh release view`, validate the same stable group tag, then download the exact
binary and checksum names with `gh release download`. Suppress child diagnostics
that could reflect credentials and report the failed operation without secrets.
Authentication or partial-download failure must preserve the installed binary;
never fall back to anonymous retrieval. Help and uninstall remain offline and
require neither gh nor authentication.

Keep one shared checksum, staging, destination privilege and cleanup path after
retrieval. Do not copy installer algorithms into private products. Public curl
behavior and required regression cases remain in force. Qualify configuration
selection, stored-login/token transport, exact host/tag/assets, rejection and
partial failures, checksum enforcement and credential redaction with isolated
fixtures; actual private repository installation remains a consumer acceptance.
This transient technical operation adds no persisted business lifecycle or
Header-Detail record.
