# Problem matchers

Logcompact can compile self-contained
[GitHub Actions problem matcher](https://github.com/actions/toolkit/blob/main/docs/problem-matchers.md)
files and inline
[VS Code problem matcher](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/contrib/tasks/common/problemMatcher.ts)
objects into its bounded streaming parser plan.

```console
logcompact build.log --problem-matcher .github/eslint-stylish.json
some-command 2>&1 | logcompact --problem-matcher matchers/compiler.json --format github
```

Repeat `--problem-matcher` to register multiple files. When owners collide,
the last definition replaces the earlier definition, matching GitHub Actions
registration behavior.

## Supported definitions

Logcompact accepts the usual `{ "problemMatcher": [...] }` document, one
inline matcher object, or an array of inline matcher objects. Each matcher must
have a non-empty `owner`; generated editor-only owner identifiers would make
results nondeterministic.

Supported matcher properties are:

- `owner`, `source`, and default `severity`;
- `pattern` as one object or an array of consecutive line patterns;
- `fileLocation` values `relative`, `absolute`, and `autodetect`, including a
  lexical prefix for relative/autodetect pairs;
- `applyTo: "allDocuments"`;
- watch/background metadata, which is accepted but has no effect on diagnostic
  extraction because Logcompact does not manage task lifecycle.

Supported pattern properties are `regexp`, `kind`, `file`, `fromPath`,
`location`, `line`, `column`, `endLine`, `endColumn`, `severity`, `code`,
`message`, and `loop`. Multiline patterns must match consecutive lines. Only
the final pattern of a multiline matcher may loop; captures from earlier lines
are inherited by each emitted diagnostic.

Single-line matchers use VS Code's capture defaults when fields are omitted:
file group 1, line group 2, column group 3, and the complete match as the
message. Explicit capture mappings are recommended for portability.

## Deliberate compatibility limits

Definitions are compiled with Rust's linear-time `regex` engine. Common
ECMAScript expressions work, but backreferences and look-around are rejected.
This prevents matcher-controlled catastrophic backtracking.

Named references such as `$gcc`, named `pattern` references, and `base`
inheritance require an external VS Code contribution registry and are rejected
with a configuration error. Inline the referenced definition instead.

`fileLocation: "search"`, `openDocuments`, and `closedDocuments` require a
filesystem or editor document state. The reusable parser performs no I/O, so
these modes are rejected. `autodetect` is resolved lexically: absolute captured
paths remain absolute and other paths use the configured prefix without
probing the filesystem.

## Bounds and output policy

Matcher JSON size, owner count, patterns per owner, regex source and compiled
size, partial state count, and retained capture bytes all have explicit
defaults in `ProblemMatcherLimits`. Crossing a runtime state bound marks the
reduction as truncated.

Captured messages, codes, paths, matcher labels, and provenance pass through
the same path mapping, redaction, deduplication, ranking, item limit, and
serialized-byte budget as built-in diagnostics.
