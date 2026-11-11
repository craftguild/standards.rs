# Initialization and pinned context

ADR 0004 replaces archive delivery; ADR 0006 separates initializer and engine and
controls explicit updates. The [execution guide](../guides/orchestration.md)
describes installation and daily use. The normative baseline remains 0.1.0 and
the execution receipt protocol remains `craftguild-execution-1`.

## Reference and entry ownership

`standards/engine.json` is the one committed provenance authority. Reference
schema 3 records the engine version, full clean source revision, baseline,
execution protocol, entry protocol `craftguild-entry-1`, exact HTTPS binary/SBOM
assets by supported host, and managed ownership hashes, including the two entry files:
`standards/launcher.py` and `standards/make/common.mk`. No archive or source ledger
is distributed. A versioned release metadata index combines independently
qualified host deliveries; unavailable hosts are absent, never inferred from
another host's asset. Init checks the index against the invoking clean tool.

Init's root is exactly cwd. New references require published version metadata;
source builds without matching publication are refused. Init selects the invoking
initializer or engine version/revision using the update policy below.
Exact retries reuse recorded metadata without mutable lookup.
Repository prime always uses the fixed verified engine.
Consumers later use that same full revision in common-crate declarations.
Known old pins at `standards/manifest.json` or `standards/execution/engine.json`
require explicit migration. Init checks them before release lookup, engine
acquisition or publication, and refuses to create a second pin authority. It leaves
the legacy files and product Make recipes untouched and provides no legacy loader.

The small common Python launcher owns host selection, HTTPS acquisition and
verification. It acquires only on explicit setup (also called by init). Products
do not implement acquisition. Every operational dispatch verifies entry hashes,
binary/SBOM hashes, revision, version, host and compatibility. Downloaded code is
never executed before its bytes match the reference. Cache entries are identified
by reference digest and host under `.craftguild/engine/` and published from owned
staging. Invalid existing entries fail closed; a failed acquisition preserves
any valid entry. Normal operations and prime have no acquisition fallback.

## Initialization consistency boundary

The generated set consists of the reference and common entry, a short local
`.agents/skills/craftguild-standardize/SKILL.md`, the known AGENTS common block,
a starter config only if absent, a Makefile only if absent, and a deduplicated
root `/.craftguild/` ignore rule. Existing config and Make recipes remain owned
by the product. Init reports the shared include needed for manual Make wiring.
The common AGENTS block names the normative baseline and keeps product contracts
outside that managed block, matching the ordinary instruction quality gate.
The starter has an empty artifacts array and commented examples; operational
validation remains strict.

Plan all writes and check conflicts/symlinks before acquisition or publication.
Hold an exclusive init lock, stage replacements, recheck observed bytes before
each write and publish the reference last. On failure restore only still-owned
written bytes, in reverse order; preserve unrelated/concurrently changed data
and valid cache entries. Before publication, owned staging retains each replaced
file's previous bytes and permission mode. Rollback restores that staged file by
rename, without requiring write permission on a read-only replacement. A failed
restore does not stop other owned restores; retain the recovery staging and report
its location with per-file failures rather than discard the previous bytes.
Identical files require no writes. Never initialize Git,
search parent/sibling checkouts, untrack files or modify global settings.

This is a transient technical operation: immutable references, staging and cache
identity retain provenance. Rejected/no-op attempts are not transitions. There
is no business entity or business line-item aggregate, so no persistent business
transition table or Header-Detail records are introduced (design-baselines).

## Prime

Canonical contracts and adoption/configuration guidance are embedded and sealed
engine inputs. Repository prime delegates to the pin's verified cache. The internal
context renderer checks that its own identity matches the pin; it reads product
configuration without running product code. Missing, malformed or incomplete
configuration is preparation status, never conformance success.

Output is banner-free Markdown on stdout. Topics render the matching embedded
document with repository-relative links resolved to full-revision source links.
Prime never downloads, writes, compiles, generates or checks products. The
overview assigns common engine/font/measurement tests to standards and SDK/ABI,
generator, domain and actual integration checks to products. Generic counter gaps
remain upstream work; no product-specific counter is generated.

## Update policy

The initializer exposes only `init [--force]` and offline presentation;
the separate engine owns all operations. Both artifacts belong to one release
group, version and full clean revision. Release metadata schema 3 maps every host
to explicit `initializer` and `engine` binary/SBOM pairs. Machine engine identity
also includes its command, preventing initializer or historical full-CLI substitution.

Reference schema 3 retains entry hashes and adds hashes for the local Skill and
AGENTS common block. Schema 2 flat references can be updated only when their
entry hashes and actual files, Skill and common block match the frozen ADR 0004
baseline. Unknown baselines, edited blocks and unsupported protocols fail closed.
Historical v0.4.0 metadata remains readable for ownership assessment, but new
initialization requires role-separated metadata. No legacy binary is downloaded
or executed to establish ownership.

| Existing pin compared with the invoking CLI | `init` | `init --force` |
| --- | --- | --- |
| Absent | Initialize | Initialize |
| Older version | Upgrade | Upgrade |
| Same version and revision | Revalidate | Revalidate |
| Same version, different revision | Error | Replace |
| Newer version | Error | Downgrade |

Stable SemVer comparison happens before transport or writes. Newer tools upgrade;
equal version/revision revalidates the recorded reference without release lookup;
older tools and equal-version revision changes need `--force`. Force changes only
this policy. Exact retries preserve all managed bytes and restore only missing
cache. Other updates select the invoking clean published tool's exact identity.

The exclusive lock spans release lookup, acquisition and publication. All observed
inputs, including the previous pin, are rechecked before publication. The staged
engine is verified before replacements, and the pin is published last. Rollback
restores only still-owned bytes; caches remain distinct by reference and host.
Config, Make recipes, AGENTS product contracts and dependency inputs retain their
consumer ownership. Common-crate revision changes remain manual adoption work.

This remains a transient provenance/consistency boundary. There is no business
entity, transition-history table or Header-Detail aggregate (design-baselines).

## Renamed release repository

The release index, role assets and fixed common-crate dependencies use
`craftguild/standards.rs`. Old repository URLs are not accepted by the current
initializer. Before explicitly reinitializing an owned consumer, archive and
verify its old managed entry, remove only that managed entry and common AGENTS
block, then initialize with the selected published release. Preserve the product
configuration, product instructions and third-party provenance. This is an
explicit source migration, not a runtime compatibility path.
