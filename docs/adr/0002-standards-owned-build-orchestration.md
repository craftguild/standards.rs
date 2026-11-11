# ADR 0002: Build orchestration in standards

Status: Accepted; delivery and CLI entry updated by [ADR 0004](0004-repository-initialization-and-prime.md)
and [ADR 0006](0006-initializer-cli-and-controlled-engine-upgrades.md)

Extends [ADR 0001](0001-artifact-identity-and-offline-presentation.md).
The implementation uses versioned configuration, seals and an entry protocol.
Product migration and publication remain separate; published v0.1.0 is unchanged.

## Context

Product adoption exposed orchestration that still lives outside standards.
Oriha coordinates artifact preparation, package builds and installation through
its Makefile and local scripts. Chigiri additionally selects toolchains, targets
and profiles, invokes Cargo, and records build outputs. Its debug adapter adjusts
a release preparation record because the common builder only prepares release
builds.

These operations determine which artifact was built and which SBOM belongs to it.
Leaving them in each product creates competing implementations of the same
contract. Reducing Makefile length alone would leave that ownership unchanged.

Products also have real differences: Expo and native binding generation,
platform SDK checks, cross-compilation, generated-contract verification, and
ordinary versus extended validation. Those operations need explicit extension
points without giving each product another execution engine.

## Decision

### Standards owns execution

The standards engine owns the sequence from artifact selection through
prerequisite checks, asset preparation, dependency evidence, SBOM validation and
sealing, compiler invocation, output verification and successful build recording.
It resolves the effective toolchain, target, profile, features and output paths
once and passes that context to each step.

Rust and npm drivers implement reusable language and packaging behavior. The
engine must not branch on product names. Consumer libraries retain Rust 1.85
compatibility; execution-tool requirements remain separate.

Debug and release are explicit build profiles. Identity and SBOM preparation
must describe the selected profile from the start. Products must not rewrite
prepared metadata to implement another profile. Application build scripts retain
ADR 0001's verification and embedding responsibilities and never recurse into
Cargo.

The engine also owns dispatch for install, test, check, acceptance and release.
It uses the existing [development](../../spec/development.md) and
[release](../../spec/releases.md) contracts. Install copies a matching existing
artifact without building. Acceptance remains explicit. Release preparation
remains separate from commits, tags, pushes and publication, and preserves the
OpenPGP verification and stop requirements.

### Products declare configuration and command hooks

`craftguild.toml` declares artifact identity, toolchain and build settings,
required inputs and outputs, preparation dependencies, and product commands.
Standards determines execution order and success conditions from those
declarations. Products retain their domain contracts and specialized operations.

External commands expressed as argv are the default extension interface, building
on the current [configuration model](../design/configuration.md). Commands run at
defined preparation or verification stages with the selected artifact context.
They declare the files they consume and produce so that the engine can validate
outputs and bind them to the artifact's evidence.

Hooks implement a bounded product operation, such as generating bindings or
checking an SDK. They must not duplicate the common sequence of preparation,
SBOM creation, compilation and recording. Normal adoption must not require a
product-specific Rust adapter crate. A reusable capability can later become a
standard driver when multiple products need it.

A failed or cancelled prerequisite or hook stops dependent work and cannot
produce a successful artifact receipt. Existing owned outputs must not be
replaced by an unverified candidate. Required toolchains and platform SDKs remain
explicit prerequisites; an ordinary build must not silently acquire them.

### Make and CI use the same engine

The v0.5.0 initializer creates a fixed engine reference and thin common Make/Python
entry. Explicit init or setup acquires the verified engine in a repository-local
cache. Ordinary operations use that cache without downloading or updating it.
A standalone checkout needs the committed entry and explicit setup, without a
sibling standards checkout, global engine or copied rules/binary archive.

The compatible product entry is:

```makefile
include standards/make/common.mk
```

This replaces the former bundle entry path; historical releases are unchanged.
A product Makefile may add aliases that delegate to the engine. It must not
reproduce orchestration. Make, the pinned launcher and CI execute the same
configured operations and validations through `craftguild-standards-engine`.
The installed `craftguild-standards` only initializes or updates that entry.

### Preserve compatibility and technical provenance

Migration preserves documented product targets, toolchain boundaries and platform
prerequisites through explicit configuration or forwarding aliases. New adoption
uses ADR 0003's standard Rust output layout and updates callers of old output
locations explicitly. `TARGET`, `CARGO_TARGET_DIR`, `PREFIX`, `BINDIR` and `DESTDIR` retain their
documented behavior. Artifact discovery and installation use one shared output
resolver.

Product-specific tests, generated ownership and exact reproduction, runtime
dependency inclusion, ABI guarantees and existing validation scopes remain
required. Release preparation must remain repeatable when subsequent builds
refresh derived evidence; cached ownership conversions must not create unrelated
tracked edits during a pending release.

Each invocation has a resolved execution plan and ordered step results. Logs
record execution, failure or cancellation; successful artifact receipts bind
outputs to the effective settings and verified inputs. These are transient
technical operations with local provenance, not persistent business entities.
The transition-history baseline is satisfied at that technical scope through
execution records and receipts. Artifact and file collections are not business
line items, so no business state table or Header-Detail database model is added.
Product domain models remain unchanged.

Consumer SBOMs describe dependency/component inclusion and artifact identity.
Source file inventories and hashes belong in the developer's local prepared seal,
execution history and receipt. They and their aggregate digest are excluded from
new SBOM metadata. Prepared seal schema 2 makes this separation explicit while
preserving verification of existing schema 1 artifacts.

## Alternatives and consequences

Sharing Make fragments alone reduces duplication in recipes but leaves execution
ownership distributed. Requiring product Rust adapters introduces another crate
and dependency boundary for normal adoption. Both are rejected as the default.
Embedding product-specific branches in standards is also rejected; specialized
operations use configuration and command hooks.

Standards gains responsibility for build context, scheduling and artifact
recording. Products gain smaller entry files and fewer local infrastructure
adapters. Configuration and hook contracts therefore need versioned compatibility
and conformance tests before rollout.

## Acceptance and rollout

Implementation must demonstrate:

- A fresh standalone checkout resolves the pinned entry files and engine without
  a sibling repository or a global installation.
- Make, direct invocation and CI select the same operations and artifact facts.
- Debug/release, target, feature and output-directory selection produce matching
  build identity, SBOM and receipts without product metadata rewriting.
- Product generation and verification hooks execute in the required order;
  failure, cancellation, missing outputs and stale inputs prevent success.
- Failed preparation or compilation preserves the previous verified output and
  does not report that output as the result of the failed invocation.
- Install honors destination and artifact selectors, never invokes Cargo/npm,
  and rejects missing or mismatched builds with a build instruction.
- Product targets, generated ABI/ownership, language gates and platform-specific
  checks retain their existing guarantees and validation scopes.
- Build and evidence refresh after release preparation preserve its planned-file
  boundary and do not cause another version increment.

Keep historical published revisions intact. The original bundle implementation
is historical evidence; v0.5.0 integration follows ADR 0006's initializer and fixed
engine entry. ADR acceptance, engine implementation and product adoption are
separate steps. Configuration schema 2, reference schema 3, entry protocol
`craftguild-entry-1` and execution protocol `craftguild-execution-1` are distinct.
Use the [product integration criteria](0006-initializer-cli-and-controlled-engine-upgrades.md#product-integration-and-local-completion)
for current adoption, rather than treating successful init as completion.
The original implementation used real Rust/npm consumers and isolated product pilots.
Execution contracts are defined in the [design](../design/orchestration.md);
verification evidence and unexecuted host gates are recorded in `bd` issue
`standards-fcr`. Acceptance of this decision does not certify foreign platforms or
authorize resuming product adoption, changing versions, committing or publishing.
