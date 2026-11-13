<h1 align="center">CraftGuild Standards</h1>

<p align="center">
  <a href="https://github.com/Essk/ai-contribution-level">
    <img src="https://raw.githubusercontent.com/Essk/ai-contribution-level/main/badges/level-3.svg" alt="AI Contribution">
  </a>
</p>

Shared CLI presentation, artifact SBOM preparation, quality checks and development
contracts for CraftGuild products. Language-neutral rules are backed by Rust and
npm adapters and the `craftguild-standardize` agent skill.

`craftguild-standards` initializes and updates repositories. The separate
`craftguild-standards-engine` runs builds, checks, packaging and releases through
each repository's fixed entry. `make prime` reads that engine's embedded rules.
Engine versions and the normative **0.1.0** baseline are separate.

## Quick start

Install the initializer with POSIX sh, curl, and sha256sum or shasum available.
Initialization also needs Python 3. From the root of the repository to initialize:

```sh
curl -fsSL https://raw.githubusercontent.com/craftguild/standards.rs/main/scripts/install.sh | sh
"$HOME/.local/bin/craftguild-standards" init
make prime
```

The installer selects the latest published release and defaults to `~/.local/bin`.
Init acquires that same published engine version and revision and creates the reference, common entry and local Skill.
Configure real artifacts before building.
If a Makefile already exists, wire `include standards/make/common.mk`; until then use `python3 standards/launcher.py prime`.
See the [execution guide](docs/guides/orchestration.md).

After cloning an already initialized repository, run `make setup`, then
`make prime`. The committed entry acquires its fixed engine; no global CLI is
required. The [installer guide](docs/guides/installers.md#download-installation)
covers custom destinations and uninstalling only the initializer binary.

## System requirements

Release binaries target macOS arm64 and Linux x86_64 (GNU). Both binaries' help,
version and SBOM operations, plus the engine's banner, need only the binary.

Init and explicit setup need Python 3, curl and HTTPS access to release assets.
The shared entry needs Python 3; Make is the convenient command interface.
Prime reads the verified cache offline, without compilers or product execution.
Builds and checks need the selected language tools, SDKs and dependencies.

The consumer libraries `craftguild-cli`, `craftguild-cli-build` and
`craftguild-core` support Rust 1.85. Building the standards tools requires a
separate toolchain. Platform qualification is recorded in the
[verification guide](docs/guides/verification.md).

## Build requirements

- Rust/Cargo 1.95, Git, Make and Python 3 to build standards from source.
- tokei **15.0.0** with JSON serialization, Node.js/npm and GnuPG 2 with Ed25519
  support for repository checks. CI uses Node.js 24.
- The configured bundler, type checker and language tools for each Web example.

```sh
git clone https://github.com/craftguild/standards.rs.git
cd standards.rs
cargo fetch --locked
make build
```

This produces `craftguild-standards` and `craftguild-standards-engine` under
`target/<target-triple>/release/`, each with its own embedded SBOM. Ordinary builds
use locked offline dependencies; they do not install dependencies or toolchains.
Run `npm ci` before `make test` or `make check`. The Web examples have their own
lockfiles; see the [development guide](docs/guides/development.md) for preparation.

Small Slant font data and CycloneDX schemas are pinned and included. Source-build
`make install` copies the built initializer and supports `PREFIX`, `BINDIR` and
`DESTDIR`, defaulting to `/usr/local/bin`. This differs from download installation.

## Usage

After configuring the product, use its pinned entry:

```sh
make prime PRIME_ARGS="--topic configuration"
make build
make build BUILD_ARGS="--profile debug"
make check
make acceptance
make release RELEASE_ARGS=--json
```

A missing cache requires explicit `make setup`; ordinary operations never
download or upgrade the engine. Read the [configuration](docs/design/configuration.md)
and [release guide](docs/guides/releases.md) before configuring builds or preparing a release.

To upgrade, install the desired initializer and run `craftguild-standards init`
again. It selects that CLI's exact version/revision. Older versions and
same-version revision changes require `init --force`; local edits and integrity
checks remain protected. Manually align common-crate Git revisions and finish
product adaptation after updating. See the [update policy](docs/design/initialization.md#update-policy).

Common crates use full Git revisions and are not published to crates.io. Products
retain their own argument grammar and pass `Brand`, `BuildInfo` and embedded SBOM
bytes to the output functions. See the [standalone Rust](examples/rust-only/README.md),
[embedded Web](examples/embedded-web/README.md) and
[independent frontend/backend](examples/independent/README.md) examples.
Use `make check CHECK_ARGS="--changed --json"` to narrow source measurement.

## Known limitations

- The npm adapter consumes esbuild metafiles and package-lock v3. Other bundlers
  need an explicit adapter/evidence conversion; package discovery alone cannot
  establish which code shipped.
- Rust dependency reporting follows the selected Cargo runtime graph at package
  granularity. It excludes development/build/procedural-macro edges and does not
  claim symbol-level linker reachability.
- SeaORM inspection parses syntax and macro tokens, without compiler name/type
  resolution or expansion of arbitrary external macros.
- Unsupported conditional test separation is reported as unmeasured.
- Setup selects only hosts present in the fixed release reference. Toolchains and
  SDKs remain separate prerequisites. Subprocess execution requires a Unix host.
- Old bundle references require explicit migration. Historical v0.4.0 has a single
  full CLI; it does not provide the split interface or force policy described here.

## Community

Use repository issues for ordinary reproducible bugs. Start with the
[documentation](docs/README.md) and include the standards version, command and
relevant rule ID. Keep credentials and private product data out of reports.

## Security

No private vulnerability-reporting channel has been established for this project.
Do not publish sensitive vulnerability details in public issues.

## License

First-party code is licensed under either the [MIT License](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option.
See the [font notice](assets/fonts/NOTICE.md) and
[schema notice](schemas/NOTICE.md) for third-party material.
