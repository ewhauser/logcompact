# Problem matchers

A problem matcher is a JSON description of how diagnostic text maps to a
structured finding. Its regular expression recognizes one or more log lines,
and numeric fields select capture groups for the filename, position, severity,
code, and message.

For example, this matcher:

```json
{
  "problemMatcher": [
    {
      "owner": "example-compiler",
      "source": "example compiler",
      "pattern": {
        "regexp": "^(.+):(\\d+):(\\d+): (error|warning): (.+)$",
        "file": 1,
        "line": 2,
        "column": 3,
        "severity": 4,
        "message": 5
      }
    }
  ]
}
```

turns this line:

```text
src/main.rs:12:4: error: unknown variable
```

into a located error with path `src/main.rs`, line 12, column 4, and message
`unknown variable`. Capture group zero means the complete regex match. Remember
that backslashes must be escaped inside JSON strings.

The format is shared with
[GitHub Actions problem matchers](https://github.com/actions/toolkit/blob/main/docs/problem-matchers.md)
and inline
[VS Code problem matchers](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/contrib/tasks/common/problemMatcher.ts).
Logcompact compiles the portable, self-contained part of that format into its
bounded streaming parser plan.

## Configuring the CLI

Save the definition as JSON and pass it before parsing a file or stdin:

```console
logcompact build.log --problem-matcher .github/example-compiler.json
some-command 2>&1 | logcompact --problem-matcher matchers/compiler.json --format github
```

Repeat `--problem-matcher` to register multiple files:

```console
logcompact build.log \
  --problem-matcher matchers/compiler.json \
  --problem-matcher matchers/linter.json
```

Each matcher needs a non-empty `owner`. If the same owner is registered more
than once, the last definition replaces the earlier definition. This only
resolves collisions among configured problem matchers; it does not replace a
Logcompact built-in parser.

A file may contain the usual `{ "problemMatcher": [...] }` wrapper, one inline
matcher object, or an array of inline matcher objects. Definitions are fully
validated and compiled before Logcompact consumes the log. An invalid matcher
fails the command instead of silently dropping diagnostics.

## Pattern fields

The surrounding matcher configures identity and defaults:

| Field | Meaning |
| --- | --- |
| `owner` | Required stable identity used when later definitions replace earlier ones |
| `source` | Optional human-readable source stored with finding provenance |
| `severity` | Default `error`, `warning`, or informational severity when the pattern does not capture one |
| `fileLocation` | `relative`, `absolute`, or `autodetect`; a relative/autodetect pair may include a lexical path prefix |
| `pattern` | One pattern object or an array of consecutive line patterns |
| `applyTo` | Only `allDocuments` is portable and accepted |

Each pattern has a `regexp` plus numeric capture mappings. Supported mappings
are `file`, `fromPath`, `location`, `line`, `column`, `endLine`, `endColumn`,
`severity`, `code`, and `message`. `kind: "file"` creates a whole-file finding
without a line number. `location` can capture `line`, `line,column`, or
`line,column,endLine,endColumn` in one group.

Single-line matchers use VS Code's defaults when mappings are omitted: file
group 1, line group 2, column group 3, and the complete match as the message.
Explicit mappings are clearer and more portable.

### Multiline matchers

An array of patterns is a small consecutive-line state machine. Earlier
patterns capture context inherited by later patterns. Only the final pattern
may set `loop: true`:

```json
{
  "problemMatcher": [
    {
      "owner": "eslint-stylish",
      "pattern": [
        {
          "regexp": "^([^\\s].*)$",
          "file": 1
        },
        {
          "regexp": "^\\s+(\\d+):(\\d+)\\s+(error|warning)\\s+(.+?)\\s{2,}(\\S+)$",
          "line": 1,
          "column": 2,
          "severity": 3,
          "message": 4,
          "code": 5,
          "loop": true
        }
      ]
    }
  ]
}
```

This captures a filename header followed by one or more diagnostics. The lines
must be consecutive. A nonmatching line ends and resets the partial match; it
is also considered as the possible first line of a new match.

## Precedence with built-ins

The CLI always enables the built-in parsers. Configured problem matchers are
additive and run on the same normalized lines; they do not short-circuit or
disable a built-in parser.

The deterministic arbitration rules are:

1. Among configured problem matchers with the same `owner`, the last loaded
   definition is the only one that runs.
2. A located problem-matcher finding suppresses an overlapping generic
   fallback. This is the normal result for a format the built-ins did not
   recognize precisely.
3. A recognized built-in located or structured finding is not suppressed by a
   problem matcher. Both findings remain, because the built-in may contain
   richer language-specific evidence and the custom matcher may express a
   caller-specific interpretation.
4. Output budgets retain errors before warnings and notes. At equal severity,
   the generic ranker orders built-in compiler, test, lint, and infrastructure
   diagnostics before custom problem-matcher diagnostics, which use the
   `tool` class.

Therefore, use a problem matcher to extend coverage, not to override an
existing built-in interpretation. If both recognize the same line, JSON output
will preserve their different provenance and may contain both findings.
Library consumers that require replacement semantics can construct a
`ParserPlan` containing only their `ProblemMatcherParser` or provide a custom
`Ranker`; the CLI deliberately keeps its built-ins enabled.

## Performance guidance

For the CLI, built-ins alone perform best because every configured matcher adds
at least one regex attempt per input line. Matcher cost grows with matcher
count, pattern count, input line length, and how often multiline prefixes
match. The regex engine guarantees linear-time matching, but it cannot remove
that additive work.

Choose the smallest parser plan that meets the accuracy requirement:

| Input | Recommended choice |
| --- | --- |
| A compiler, test framework, or runtime already recognized by Logcompact | Built-ins only |
| One unsupported, stable single-line format | One anchored single-line problem matcher |
| A header followed by several consecutive diagnostics | A multiline matcher with `loop` only on the final pattern |
| A high-volume service that accepts one fixed format | A library `ParserPlan` containing only the compiled matcher, if built-in coverage is not needed |
| Mixed or evolving logs | Built-ins plus the minimum number of custom matchers needed to fill gaps |

For the narrow fixed-format corpus in the repository benchmark, the expected
speed order is matcher-only, built-ins-only, then combined. Those modes do not
provide equal coverage: matcher-only wins by skipping every language and test
parser it does not need, while combined deliberately pays for both forms of
recognition. In the CLI, where built-ins are always enabled, built-ins-only is
the fastest choice and combined is the compatibility-first choice.

For faster custom definitions:

- anchor expressions with `^` and `$` when the whole line has a fixed shape;
- begin with a selective literal or character class instead of a broad `.*`;
- combine related multiline diagnostics with a final looping pattern;
- avoid registering unused matchers; cost is approximately additive;
- compile a `ProblemMatcherRegistry` once and reuse its parser plan in a
  long-lived library process;
- feed reasonably sized chunks to the streaming session. Results are
  chunk-invariant, but very small chunks add framing overhead.

The reproducible Criterion cases in [BENCHMARKS.md](BENCHMARKS.md) compare
built-ins only, a matcher-only plan, and built-ins plus a matcher. Run them on
the target hardware before making a throughput-sensitive choice.

## Deliberate compatibility limits

Definitions use Rust's linear-time `regex` engine. Common ECMAScript
expressions work, but backreferences and look-around are rejected. This
prevents matcher-controlled catastrophic backtracking.

Named references such as `$gcc`, named `pattern` references, and `base`
inheritance require an external VS Code contribution registry and are rejected
with a configuration error. Inline the referenced definition instead.

`fileLocation: "search"`, `openDocuments`, and `closedDocuments` require a
filesystem or editor document state. The reusable parser performs no I/O, so
these modes are rejected. `autodetect` is resolved lexically: absolute captured
paths remain absolute and other paths use the configured prefix without
probing the filesystem.

Watch/background metadata is accepted but has no effect on diagnostic
extraction because Logcompact does not manage task lifecycle.

## Bounds and output policy

Matcher JSON size, owner count, patterns per owner, regex source and compiled
size, partial state count, and total retained capture bytes all have explicit
defaults in `ProblemMatcherLimits`. Crossing a runtime state bound marks the
reduction as truncated.

Captured messages, codes, paths, matcher labels, and provenance pass through
the same path mapping, redaction, deduplication, ranking, item limit, and
serialized-byte budget as built-in diagnostics.
