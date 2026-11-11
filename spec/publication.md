# Publication and migration contract

Stage one implements and publishes standards only. Product adoption is stage two
and requires a later request. Reference products are read-only during stage one.
Ordinary standardization does not authorize history rewriting, publishing,
installation, service operations or unrelated dirty-work edits.

Each adopted repository builds and ordinarily tests from a standalone clone.
Remove sibling/private-evaluation prerequisites. Product dependencies use their
official public names and immutable revisions, verified over unauthenticated
HTTPS. Common crates come from `https://github.com/craftguild/standards.rs` at a full
fixed `rev`, never crates.io or permanent sibling paths.

Preserve evaluation originals and inventory records in ignored, untracked
`.private/oss-archive/`. Keep current contracts, reproducible validation, required
fixtures/goldens and generation provenance public. Classify content by purpose,
not filename. Private archives and live task/agent connections are not build,
test or release inputs. Remove internal paths in context without blind replacement
of specifications or fixtures. Preserve legitimate third-party dependency names.
First-party product licensing becomes `MIT OR Apache-2.0`; retain all third-party
notices and rights.

Export a reviewed public inventory to a separate directory and initialize fresh
Git history there. Retain original history and evaluation sources locally. Publish
dependencies first; update consumers to their new full immutable revisions and
regenerate provenance before committing consumer releases. Build final binaries
from clean public commits. Never delete reachable historical hashes before
replacing their references.

Sign every actual-work and public-history commit using the approved OpenPGP key
and verify each full revision with `git verify-commit`. Stop the task immediately
on signing or verification failure; never disable signing or switch signature
formats to bypass it. Test commits use isolated disposable OpenPGP keys and must
also be signed and verified. Keep secret keys and authentication data out of
repositories, logs and distribution archives. Recreating unsigned public ancestry
requires explicit authorization and coordinated revision/lockfile updates; a
signed descendant does not correct unsigned ancestors.

Migration must reconcile existing conflicting instructions, size checkers, tests
and design rules in the same change. Keep all required product invariants and
generated ownership/reproduction gates. Do not treat standards adoption as a
reason to run unrelated acceptance campaigns or alter business models.
