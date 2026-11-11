# ADR 0006: Initializer CLI and controlled engine upgrades

Status: Accepted; published in v0.5.0; product integration criteria clarified 2026-11-07

Date: 2026-11-07

This supersedes the single public CLI, existing-pin retention and cross-version
init delegation decisions in [ADR 0004](0004-repository-initialization-and-prime.md).
Its other initialization, acquisition, prime and verification-ownership decisions
remain in force. Published v0.4.0 still provides one full CLI and retains existing
pins; accepting this ADR does not change that binary or qualify a new release.

## Context

An initial `init` needs an installed tool, while ordinary repository operations
need a verified, repository-pinned engine. Giving both roles to the same public
command makes it harder to explain which version controls a build and how to
upgrade an initialized repository.

CraftGuild needs a small public interface for initialization and explicit engine
updates. The installed initializer selects its own published release, while the
repository entry continues to select the fixed engine for ordinary operations.
Accidental downgrades and same-version revision changes require an explicit
override.

## Decision

### Separate initialization from full engine operations

Distribute two executables in the same release group, with the same version and
full clean source revision:

| Command | Responsibility |
| --- | --- |
| `craftguild-standards` | `init [--force]` plus ordinary offline help, version and SBOM presentation |
| `craftguild-standards-engine` | Full engine operations, including initialization and prime |

The public download installer installs `craftguild-standards`. Running its `init`
acquires the matching engine for the repository. Give each executable its own
artifact identity, embedded SBOM, sidecar SBOM and checksums. Release metadata and
packaging distinguish the two executables; neither may impersonate the other.

The initializer does not expose build, check, release or prime as global operation
commands. Initialized repositories use their common Make/Python entry for those
operations. The engine and initializer share one initialization policy; direct
engine initialization must not provide a bypass around the update guards.

### Initialize or update to the invoking tool's release

The public interface is:

```sh
craftguild-standards init
craftguild-standards init --force
```

Use exactly cwd as the repository root. Select the invoking tool's own published
version and full revision, never Latest, an arbitrary requested version, an
unpublished source build or a sibling checkout. Release metadata and downloaded
engine identity must match that selection. The normative baseline remains
separate from the CLI/engine package version.

Compare supported stable SemVer versions numerically, not as strings. Apply this
policy before engine acquisition or repository writes:

| Repository pin compared with invoking tool | `init` | `init --force` |
| --- | --- | --- |
| Absent | Initialize | Initialize |
| Older version | Upgrade | Upgrade |
| Same version and revision | Revalidate; restore a missing verified cache if needed | Same |
| Same version, different revision | Error | Replace with the invoking tool's release |
| Newer version | Error | Downgrade to the invoking tool's release |

Revision ordering is not inferred from Git ancestry. An exact-identity retry
reuses the recorded reference without a mutable release lookup and does not
rewrite identical managed files. Other inconsistent identity or ownership data
is an error, not an exact-identity retry.

`--force` authorizes only the two otherwise-rejected version/revision transitions.
It does not disable SHA-256, embedded/sidecar SBOM, source identity, host,
compatibility, path safety, ownership or concurrent-change checks. It cannot
invent a missing release, rewrite public tags/assets, repair an untrusted pin by
guessing its identity, or overwrite consumer edits. Historical tools do not gain
this interface retroactively; unsupported reference/protocol migrations remain
errors, including with `--force`. Legacy bundles remain retired.

Errors show the installed and pinned version/revision and the reason for refusal.
Where the only conflict is an allowed override, explain the `init --force` option.
A successful update reports the previous and selected identities and whether an
upgrade, downgrade or same-version replacement occurred.

### Preserve product ownership and the previous working engine

Update the engine, its fixed reference and the standards-managed entry, local
Skill and AGENTS common block together. Prove that generated content still matches
its previous managed baseline before replacing it. If that ownership cannot be
established, or the consumer edited it, report a conflict requiring adaptation;
`--force` does not grant ownership of those edits.

Preserve existing product configuration, Make recipes, AGENTS repository
contracts and unrelated files. Do not edit Cargo/npm dependency declarations or
lockfiles. Report that common-crate Git revisions must follow the selected full
revision, and that configuration/API adaptation and product checks remain adoption
work. Initialization or an engine update does not certify product conformance.

Retain the exclusive init lock, owned staging, path checks and optimistic input
rechecks. Acquire and verify the selected engine before publishing managed
replacements, publish the pin last, and restore still-owned prior bytes on failure
without overwriting concurrent changes. A failed update preserves the previous
pin and valid engine cache; verification failure must not activate a replacement.
Keep cache entries distinct by fixed identity/reference and host so that the old
valid engine is not overwritten during acquisition.

This is a technical file operation, not a durable business lifecycle. The fixed
reference and version-controlled managed changes retain provenance; staging and
cache identities define the update boundary. Rejected and identical attempts are
not new state transitions. No business transition-history table or Header-Detail
aggregate is introduced (design-baselines).

### Keep setup and ordinary operations pinned

`make setup` acquires only the already-fixed engine; it never upgrades or selects
Latest. A person cloning an initialized repository needs the committed entry and
explicit setup, not a global initializer installation.

Build, check, release, install, package and prime continue to use the verified
repository pin, with no automatic download or update. Prime stays offline and
read-only and renders the fixed engine's embedded rules. Update the local Skill
and guidance to explain this distinction and the manual dependency follow-up.

## Consequences

The entry point for adopting or updating standards is one explicit `init` command.
Ordinary operations remain reproducible at the repository pin, regardless of the
globally installed initializer version. A forced downgrade is possible when the
target release and reference are supported, but is never an implicit side effect.

The CLI split changes distribution and direct global command usage. Implementation
must align installer generation, build/package receipts, release metadata, shared
entry dispatch, documentation and CI. Preserve artifact-specific consumer SBOMs,
local-only source integrity ledgers, standard Rust output paths and Rust 1.85
compatibility for public consumer libraries. ADR 0005's release policy is unchanged;
this breaking 0.x change does not itself authorize a 1.0.0 release.

## Product integration and local completion

For v0.5.0 adoption, fix both command roles and common Rust dependencies to
`69ef0ba960e4d61bff3f363c07f8d49c4d53d77e`, published in the
[v0.5.0 release](https://github.com/craftguild/standards/releases/tag/v0.5.0).
The normative baseline remains 0.1.0; configuration schema 2 and reference schema
3 are separate. This section clarifies integration obligations without changing
the published engine or expanding a product's authorized execution scope.
The product includes `standards/make/common.mk`, which dispatches through
`standards/launcher.py` to the verified full engine in `.craftguild/engine/`.

Initialization prepares integration. A successful init, setup or prime is not
product conformance. Apply these criteria to the final product source:

| Criterion | Required local evidence |
| --- | --- |
| I1 — Fixed identity | One `standards/engine.json`, matching managed entry/Skill/common AGENTS block, and common Rust dependencies at the selected full revision; distinguish package version, normative baseline and protocols. |
| I2 — Connected operations | Product Make aliases and CI use the pinned launcher. Configuration declares real artifacts, bounded product hooks, source roles, language gates and release ownership. Retire copied rules, archives, old pin authorities and custom common loaders; preserve product-owned content. |
| I3 — Standalone workflow | An isolated export of the current intended public source works without sibling checkouts, private archives or a global engine. After explicit setup and locked dependency preparation, prime and ordinary operations use the verified cache without automatic engine/toolchain/SDK acquisition. |
| I4 — Actual artifacts | Build the actual product in debug and release profiles and contractually required targets at standard Rust output paths. Verify artifact-specific SBOM and identity, relevant offline CLI/machine interfaces, and copy-only installation into an owned temporary destination. Preserve existing library, SDK and payload contracts. |
| I5 — Product quality | Final `make check` and applicable actual acceptance succeed. Preserve domain, ABI, resource, generated ownership and isolated canonical reproduction checks. Shared engine/acquisition/output/receipt/release/font/measurement regressions belong in standards with neutral fixtures; do not replicate their failure matrices in consumers. |
| I6 — Maintained delivery | Documentation, dependency boundaries and licenses match the integration. Measure changed implementation with the common Code convention, excluding tests and design documents by role. Declared installer scripts pass the common generation check; release configuration uses ADR 0005 without a local release algorithm. |
| I7 — Current evidence | Record source/HEAD/index basis, exact engine pin, commands, profiles, hosts, receipts and remaining qualification in the existing task system. Old-version results, incomplete prime, skipped checks and configuration inspection alone are not passing evidence. Required failing or missing local gates block completion. |
| I8 — Execution boundary | Local integration completion is distinct from signed commit, real release preparation, clean-HEAD packaging and publication. Honor the active request's pause and signing rules. Never fabricate a clean tree or release receipt to satisfy packaging. |

These criteria clarify product adoption without modifying published v0.5.0 assets
or treating the release's own qualification as product evidence. Keep execution
status and qualification milestones in the existing task system. Maintaining an
ADR or goal prompt does not by itself resume, pause or complete implementation.

Use existing product checks and one final-source integration pass. Add consumer
tests only for a concrete product connection risk not already covered. Re-run
expensive checks when changed inputs, failures or unresolved concerns justify it;
do not introduce a new build-test suite in every repository. A source export can
include intended uncommitted changes but must exclude caches and private state;
report its unknown/dirty Git identity honestly rather than calling it a clean clone.
Additional unavailable host/SDK qualification stays explicitly deferred and is
never reported as passed. No native Windows host qualification is introduced.

Legacy `standards/manifest.json` or `standards/execution/engine.json` requires
explicit preservation and retirement before init. Force does not migrate those
bundles, overwrite consumer edits or update Cargo/npm declarations. Preserve the
current index and unrelated work throughout migration. For a fresh clone of an
already integrated repository, explicit setup restores only its fixed cache;
the global initializer is not a runtime or build prerequisite.

## Acceptance and rollout

- Cover fresh init and both force modes across older, equal and newer versions,
  including equal-version revision conflicts and numeric ordering such as 0.9.0
  versus 0.10.0. Exact retries reuse the pin and avoid managed-file rewrites.
- Prove rejected transitions fail before transport or writes, and direct engine
  initialization applies the same guards as the public initializer.
- Verify both binary identities, exact embedded/sidecar SBOMs and host delivery
  metadata. Reject role substitution, missing assets and hash/revision/protocol
  mismatches even when `--force` is present.
- Exercise managed-entry updates, local-edit conflicts, concurrent changes and
  interrupted/failed acquisition or publication. Preserve the old working pin and
  cache on failed updates. Do not modify Cargo/npm inputs or product-owned files.
- Demonstrate init, explicit setup after a clean clone, offline prime and ordinary
  fixed-engine execution without a global full engine, legacy bundle or copied
  rules tree. Keep the common failure matrix in standards, not each consumer.
- Use isolated roots, temporary destinations and transport stubs; never live
  services or user state in tests. Run required checks, Rust 1.85 consumer checks,
  available-host acceptance and implementation-only code-size assessment when
  implementing. Record unexecuted hosts separately.

Implementation is tracked in bd as `standards-2bi`. Source qualification and
unexecuted hosts are recorded in the [verification guide](../guides/verification.md#adr-0006-source-qualification-2026-11-07).
That source qualification did not itself qualify publication. The v0.5.0 release
and product integration are separate evidence boundaries. Product adoption, real-user installation,
stage/commit, release preparation, tag, push, remote CI execution and publication
remain outside this implementation request.
