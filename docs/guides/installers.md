# Release assets and standalone installers

The full engine generates download installers and packages verified release
artifacts. Keep its fixed reference and common entry together. The public
installer installs the init-only CLI; repository setup acquires its pinned engine.

Declare an executable artifact, its release group and an installer in
[configuration](../design/configuration.md#standard-rust-outputs-and-installers).
Generate and check the script with the common engine:

```sh
craftguild-standards-engine installer
craftguild-standards-engine installer --artifact standards --check
make installer INSTALLER_ARGS=--check
```

Ordinary `check` also rejects script drift. The template is embedded in the engine,
so consumers need no generator script or sibling standards checkout.

After building clean release artifacts at the current full Git HEAD, package all
installer artifacts in the group into a fresh directory:

```sh
craftguild-standards-engine package --group standards --output target/release-assets
make package PACKAGE_ARGS="--group standards --output target/release-assets"
```

Group selection may be omitted only when installer declarations refer to one
group. Receipts must share version/target, match the group's version authority and
the clean current revision, and pass complete input, exact inventory, SBOM and
output validation. The result contains `<command>-v<version>-<target>`, the same
name plus `.bom.json`, and `SHA256SUMS-<target>.txt`. Package reads output paths
from receipts and never builds, resolves dependencies or acquires an engine.
The launcher requires an existing verified cache; run setup/build first.
Standards declares both initializer and engine artifacts in one release group,
with independent receipts, embedded/sidecar SBOMs and checksums. The public
`scripts/install.sh` installs only `craftguild-standards`; its `init` acquires
`craftguild-standards-engine` for the repository. The engine installer declaration
also makes the second artifact available to common packaging. Source metadata
schema 3 explicitly labels both roles per host. Historical v0.4.0 full-CLI assets
cannot substitute for the separate engine.
Standards' delivery wrapper adds flat versioned host release metadata, then
hashes every asset except checksum files. Local source ledgers stay local.

## Download installation

Use the raw script from the selected release tag, rather than a GitHub HTML blob
page. These examples select v0.6.1 and default to `~/.local/bin`:

```sh
curl -fsSL https://raw.githubusercontent.com/craftguild/standards.rs/v0.6.1/scripts/install.sh | sh -s -- --version 0.6.1
"$HOME/.local/bin/craftguild-standards" init
```

Run init in the intended repository root. It acquires the engine at the installed
CLI's exact version/revision; installing the CLI alone does not initialize or
upgrade repositories. See the [execution guide](orchestration.md) for Make wiring,
clone setup and updates.

Choose another destination with `--bindir` or `BINDIR`. The installer uses sudo
only when destination operations need elevation. Uninstall removes only the
initializer binary, preserving settings and repository engine caches:

```sh
curl -fsSL https://raw.githubusercontent.com/craftguild/standards.rs/v0.6.1/scripts/install.sh | sh -s -- --version 0.6.1 --bindir /usr/local/bin
curl -fsSL https://raw.githubusercontent.com/craftguild/standards.rs/v0.6.1/scripts/install.sh | sh -s -- --uninstall
```

For a custom destination, pass the same `--bindir` when uninstalling. The version
is stable `X.Y.Z`, without the tag prefix. Check asset availability for the target.
`--help` needs no network. Duplicate, unknown and incomplete options fail;
`--version` cannot accompany `--uninstall`.

The default installer needs POSIX sh, curl, standard utilities (including mktemp
and awk), and sha256sum or shasum. Darwin/Linux and x86_64/amd64/arm64/aarch64 map to
`x86_64-apple-darwin`, `aarch64-apple-darwin`,
`x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`. Other systems fail;
Linux does not fall back to musl.

Without `--version`, one GitHub Latest Release redirect selects a stable tag with
the configured group's prefix. A different group's Latest fails with an explicit
version instruction. Every asset request then uses that fixed tag. The script
downloads the binary and checksums without elevation, requires one well-formed
SHA-256 entry for the exact asset basename and verifies its bytes before placement.
It never runs a downloaded binary to verify it.

Destination priority is `--bindir`, environment `BINDIR`, then `$HOME/.local/bin`.
Destinations must be nonempty absolute paths; spaces are supported. HOME is needed
only for the default. The installed filename is the artifact's command. Verified
bytes are staged on the destination filesystem, set to 0755 and renamed over the
old binary only after success. Failure or interruption cleans owned temporary
files. Writable destinations and root need no sudo; otherwise only necessary
creation, placement, deletion and cleanup commands use sudo with separate argv.
Authentication or placement failure preserves the old binary. The script reports
a missing PATH entry without editing shell startup files.

Uninstall needs no curl, gh, hash tool or OS/architecture probe. It removes only the
regular file at the command destination; absence succeeds, directories and
symlinks fail. Settings, data, unrelated commands and directories remain.

Source-build `make install` is a separate copy-only operation with the existing
`PREFIX`/`BINDIR`/`DESTDIR` contract and `/usr/local/bin` default.

## Private GitHub releases

Authenticated transport requires standards v0.6.0 or later.

Add `transport = "github-cli"` to a private repository's existing `[[installers]]`
declaration and regenerate its script with `make installer`. The default remains
`curl`; setting a token does not silently change a public installer's transport.

This mode needs GitHub CLI (`gh`) instead of curl, plus the same POSIX utilities
and hash tool. Authenticate with `gh auth login --hostname github.com`, or provide
`GH_TOKEN` through the environment. The token or login must grant read access to
the selected private repository and its release assets. In GitHub Actions, the
repository's `GITHUB_TOKEN` can be passed as `GH_TOKEN` with `contents: read`.
See [GitHub CLI authentication](https://cli.github.com/manual/gh_help_environment).

Fetch the selected tag's installer through an authenticated GitHub contents
request or an authorized source checkout. The generated script uses the same
`--version`, `--bindir` and `--uninstall` options. It never performs a login,
prompts for credentials, accepts a token argument or writes authentication config.

The repository host is fixed to GitHub.com. Latest selects one tag through
`gh release view`; every exact binary/checksum request then uses that tag through
`gh release download`. Debug output and interactive prompts are disabled in these
child commands. Authentication, access and network failures stop installation
with a generic operation-specific error, suppressing credential-bearing command
diagnostics. There is no anonymous fallback. Existing checksum validation,
atomic placement, destination-only sudo and temporary-file cleanup are shared
with the public transport. Help and uninstall do not need gh or authentication.
