# Quality contract

`craftguild-standards-engine check` owns whole-repository and changed-scope checks,
including relevant untracked files. Results have stable IDs and text/JSON forms.
Inventory all implementation languages and fail for unsupported configuration,
missing tools, missing measurements, empty required checks or unexecuted gates.

Ordinary test/check commands have no implicit elapsed-time deadline. Cancellation
and subprocess cleanup remain required. Product-specific watchdogs retain their
own explicit bounds; a slow aggregate must not be interrupted by a hidden
one-hour default. Build/prepare accept an explicit `--timeout SECONDS` when needed.

Use **tokei 15.0.0 Code**, per file, at most **400** implementation lines. Tests,
test-only helpers, fixtures, ADRs and design documents are excluded by actual role
with explicit evidence, not vague names. For mixed files, syntax-aware adapters
exclude only test regions. If reliable separation is unavailable, report
unmeasured. Count embedded languages once, without dropping child-language code.

Assess responsibilities and dependency direction as well as size. Split cohesive
units; do not compress formatting, delete needed tests/comments or hide code in
large strings/macros. This follows the code-size skill's implementation-only rule.

A generated size exception requires explicit ownership, safe paths, complete
file inventory, hashes and successful canonical reproduction. Only configured,
verified canonical generated Rust may use it. Naming a tree generated grants
nothing. Formatting, compilation and other checks still apply.

Language adapters connect Rust format/Clippy/tests and npm type/lint/tests.
Selectable policies enforce dependency allow/deny edges and library layers,
generated ownership, and SeaORM-only query access without requiring SeaORM in
every product. Parse Rust syntax and dependency metadata; comments and strings
containing API names are not violations. Document macro/type-resolution limits.
Repository-specific checks retain fixed migrations, generator contracts and
business invariants. Never drop existing required checks while generalizing.
