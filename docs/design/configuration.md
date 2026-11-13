# Configuration

## Release groups

Optional `[[releases]]` entries declare `id`, `initial_version`, `tag_prefix`, public
`notes` directory and nonempty literal `paths`. Each group has explicit `cargo`
and/or `npm` entries. Cargo takes `manifest`, `lockfile`, exact local `packages`
and optional `workspace = true`; members then inherit the workspace version. npm
takes `manifest` and its v3 `lockfile`. Groups cannot share a version/lockfile
owner or tag prefix. All owned versions agree, with the first Cargo/npm manifest
as authority. Independent artifacts use distinct groups. See
[release design](releases.md) and [operation](../guides/releases.md).

Cargo may also declare `additional_lockfiles = ["tests/consumer/Cargo.lock"]` for independent path consumers of the same owned packages.
Each additional lockfile must contain at least one owned, source-free package at the current release version; it may contain a subset of the workspace packages.
These literal repository paths have exclusive release ownership, just like the primary lockfile.
Preparation updates those local package versions and version-qualified local dependency references while retaining registry/Git package versions.
npm preparation preserves JSON member order, including nested conditional `exports` and `imports`, and changes only the manifest and lock root versions.

The versioned repository configuration is `craftguild.toml`. Published v0.1.0 uses
schema 1; the current engine also supports the schema 2 execution extension.
Unknown fields, unknown adapters/policies, unsafe paths, overlapping source roles,
empty checks and absent required tools are errors. Paths are repository-relative;
absolute paths, `..` traversal and escaping symlinks are forbidden.

Artifacts declare their public command, package/version source, distribution kind,
manifest, locked dependency inputs, feature selection and assets. Source roles
describe actual responsibility: implementation, test, test-helper, fixture,
documentation, vendored or verified-generated. A filename is not role evidence.
Repository checks declare argv and purpose; no custom plugin language is used.

The [architecture](architecture.md) is the design authority. The parser is
[`config.rs`](../../crates/craftguild-standards/src/config.rs); the repository's
[`craftguild.toml`](../../craftguild.toml) is an executable example.

## Root

`schema_version = 1` or `2` and `standards_version = "0.1.0"` are required. `artifacts`
is nonempty. Optional arrays are `sources`, `adapters`, `policies`, `generated`,
and `commands`. Quality checks reject missing source roles/adapters/test gates;
build-only operations do not require running unrelated quality gates. Schema 2
also permits `operations`; schema 1 rejects both `operations` and artifact `build`.

## Artifacts

| Field | Meaning |
| --- | --- |
| id | Unique artifact selection name |
| command | Unique public binary/distribution command and prepared-record name |
| project | Organization project name, without a mandatory language suffix |
| kind | `rust` or `frontend` |
| manifest | Cargo.toml or frontend package.json, relative to repository root |
| lockfile | Exact Cargo workspace lockfile or frontend package-lock.json |
| package | Selected Cargo/npm package name |
| target | Optional artifact-specific named target, such as `browser` for frontend |
| features | Requested Cargo features; default empty |
| no_default_features | Cargo default-feature selection; default false |
| inputs | Additional required files, hashed individually |
| input_dirs | Complete required trees, inventoried and hashed |
| bundle | Embedded/independent Web evidence, required for frontend artifacts |
| supplemental | JSON array of explicit native/font/asset component records |

Cargo workspaces may be nested below the explicit source root; local package
dependencies outside that root fail. Selected normal dependencies become SBOM
components. Local build inputs are also sealed even when build-only packages are
excluded from the distribution's component list. Run builds from the repository
root so its configuration and source-root boundary are unambiguous.
Existing repository/package Cargo configuration files are included automatically;
declare other compiler, generator and product inputs explicitly in `inputs` or
complete `input_dirs` inventories.

The CLI `--target` selects the Rust compilation target and overrides a Rust
artifact's configured target. Frontend targets come from their artifact settings
or default to `browser`; selecting only a frontend does not probe Rust tools.

Schema 2 orchestration is defined in the
[execution contract](orchestration.md). Schema 1 rejects its artifact `build`
tables. Engine package versions and the normative baseline are separate;
published v0.1.0 does not provide this extension.

`bundle` requires `root` (bundler working-root path, or `.` for the repository root), `lockfile`, `metafile`
(esbuild JSON), `directory` (exact output tree), and a nonempty build `command`
argv executed from the repository root. Install locked npm inputs before building.
Every metafile output must match a real output file and recorded byte length;
every positive byte contribution must match a real input. Node-module inputs map
to package-lock v3 entries and installed package name/version. The actual bundle
may include a development dependency; an unused dependency is never included
merely because it exists in the lockfile. Linked npm packages need supplemental
records. Extra output files require explicit supplemental ownership; an undeclared
file still fails the exact bundle inventory. Supplemental records require `name`,
`version`, CycloneDX `type`, unique `reference`, actual `files` and human-readable
`evidence` describing inclusion. Optional `license` is an SPDX expression. The
standards artifact itself records its embedded Small Slant font and validation
schemas in [supplemental data](../../assets/supplemental.json).

Prepared files live under `.craftguild/<command>/`. Prepared seal schema
2 keeps source hashes and directory inventories in those local records. The SBOM
contains shared build identity, effective root features and dependency/component
evidence, without a source file list, source hashes or an aggregate source-input
digest. Build scripts recapture identity, compare Cargo features and verify the
local inventory/hashes and SBOM schema, then embed only the SBOM bytes. Published
schema 1 seals retain their original reading/verification rules; new builds use
schema 2. Configuration and prepared seal schemas are versioned separately.
Custom Rust target specification files are not supported by this adapter; use a
named compiler target. Frontend build facts record Node's version, while the
bundler's version and inputs are identified by its sealed package-lock/metafile.

## Source roles and adapters

Each source entry has a glob `pattern`, `role` and nonempty responsibility
`reason`. Roles are `implementation`, `test`, `test-helper`, `fixture`,
`documentation`, `vendored` and `generated`. Patterns must not overlap. Executable
sources with no role fail. Exclusion reasons are reviewable assertions about use;
they do not grant permission to put product code in a test tree.

Each adapter has `language = "rust"` or `"npm"`, nonempty `test` and `lint` argv,
and optional `format` and `typecheck` argv. Rust requires `format`; npm requires
`typecheck`. Rust and npm source inventories require their respective adapters.
All configured adapter gates execute during `check`, including changed-scope checks.
Only structural source measurement is narrowed by changed scope; global ownership
and dependency invariants remain global.
Rust test items, fields, variants, struct field values, match arms and statements
are removed only when their parsed conditions require `test`. Boolean
`all`/`any`/`not` predicates are evaluated with `test = false`; every possible
production feature branch and its original attribute lines remain counted.
Unsupported conditional contexts and test-dependent `cfg_attr` remain unmeasured.
Syntax spans retain lexical context, punctuation and original implementation
lines. The npm adapter uses the installed TypeScript AST
to identify mixed test use and reports it unmeasured when reliable separation is
unavailable. TypeScript must be installed in the product's npm tree or root.
Embedded HTML/SVG scripts are tokenized as HTML and inspected as JavaScript before
counting the original file's parent and embedded-language Code values once.
Unsupported component syntax such as Vue/Svelte remains unmeasured. Test globals
and known direct test imports are recognized; arbitrary product test wrappers need
explicit roles or a retained repository check.
`CRAFTGUILD_TOKEI` may select a counter executable path; its version is still fixed.

## Policies, generated outputs and repository gates

`kind = "publication"` takes the intended `repository` owner/name, optional Rust
`manifest`, and optional `npm_lockfiles`. At least one input scope is required.
It checks every declared dependency kind for standalone local paths and fixed,
credential-free HTTPS Git sources. Consumer common crates must use the official
standards Git source; only the owning `craftguild/standards.rs` workspace may use its
own internal common-crate paths. npm registry sources require integrity records;
Git sources require full immutable hashes, and local links/files must stay inside
the source repository. This is static source readiness, not proof of public access:
anonymous fetch, clean clone/build and actual CI remain publication acceptance gates.

`kind = "dependencies"` takes `manifest`, exhaustive first-party `layers` allowed
edges, and optional `restricted` library-to-permitted-package lists. Missing layer
packages and workspace packages without a layer fail. Every declared dependency
kind is inspected, so aliases/dev/build declarations cannot evade a layer rule.

`kind = "sea-orm"` takes `manifest`, nonempty source-glob `sources`, and
`allowed_packages`. It rejects direct drivers/query libraries, raw query/Statement
API syntax and schema-sync opt-in. The syntax policy includes macro identifiers
but does not expand arbitrary external macros or perform compiler type resolution.
Comments and literal strings are not API invocations. Repository checks must retain
stronger product contracts where this guarantee is insufficient.

Generated entries take `owner`, common ownership `manifest`, `output_root`, a
`reproduce` argv containing a separate `{output}` argument, and optional
`rust_size_exception` (false by default). The ownership JSON requires
`schema_version: 1`, matching `owner`, full `generator_revision`, nonempty `inputs`
and `files` hash maps. All paths are relative to the repository. The exact output
inventory/hashes must agree; isolated reproduction must yield identical files.
Only verified `.rs` outputs receive the configured size exception. Existing
generator-specific ownership/ABI rules remain connected through a small converter
and repository gate, rather than being replaced with weaker assertions.

Repository `commands` have unique `id`, nonempty `purpose`, nonempty `argv` and
`phase` (`test`, `check` or `acceptance`). Ordinary tests and checks execute in
`check`; acceptance remains an explicit separate entry. See the
[skill policy example](../../skills/craftguild-standardize/references/policies.toml).

## Standard Rust outputs and installers

An optional schema 2 root `rust_output_layout = "standard"`
fixes every Rust executable/library output at
`<target-dir>/<target-triple>/<debug|release>/<file>`, including native targets.
Target-root precedence remains CLI, `CARGO_TARGET_DIR`, artifact `build.target_dir`,
then `target`. Remove Rust `output` and `output_root` from base, target and variant
settings before enabling the mode; even matching explicit overrides are rejected.
Frontend bundle locations are unchanged. Without this setting schema 1/2 retains
its previous layout behavior. Candidate caches remain private build inputs.

Optional schema 2 `[[installers]]` declarations require `artifact`,
`repository` (GitHub owner/repo), `release_group` and `output` (a safe relative
script path). The optional `transport` is `curl` by default or `github-cli` for
authenticated GitHub.com releases; other values and unknown fields fail.
Authentication comes from an existing `gh` login or `GH_TOKEN`, never configuration
credentials. The artifact must be an executable Rust artifact whose package and
lockfile belong to that release group. Artifact and output ownership are unique;
unknown references, libraries, frontends, overlapping outputs and unsafe paths
fail. For example:

```toml
[[installers]]
artifact = "standards"
repository = "craftguild/standards.rs"
release_group = "standards"
output = "scripts/install.sh"
```

`installer [--artifact ID] [--check]` generates scripts deterministically from the
embedded shell template. `--check` and ordinary `check` compare bytes without
writing. Both template and generated shell remain measured implementation code.
`package [--group ID] --output DIR` packages all installer artifacts in one group;
omitting the group requires exactly one group among installer declarations.
No standard-layout opt-in is needed to package legacy receipts. See the
[distribution guide](../guides/installers.md) for verification and runtime rules.


## Initialization and context

ADR 0004 adds an unfinished valid-TOML starter with `artifacts = []` and standard
Rust output layout. Build/check validation still rejects the empty selection.
Init preserves any existing config and never infers packages, source roles or
adapter gates. Prime reads preparation status independently of operational
validation and returns the fixed engine's embedded documents. See
[initialization](initialization.md).


The init-only `craftguild-standards` and full `craftguild-standards-engine` share
one release group/version/revision, with independent artifact preparation and
receipts. Historical v0.4.0 has one full CLI.
`init [--force]` selects the invoking published release; numerical upgrades are
allowed, while downgrades and same-version revision changes require force.
Config and Cargo/npm inputs remain product-owned; manually align common-crate Git
revisions after updating. See [initialization](initialization.md) for reference
schema 3, known schema 2 baseline updates and integrity/ownership requirements.
