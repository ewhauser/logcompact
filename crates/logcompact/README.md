# logcompact

`logcompact` incrementally parses generic compiler, test, lint, and tool
logs from stdin or files. It is a presentation adapter over
`logcompact-core` and `logcompact-builtins`; it does not launch commands
or retain raw evidence.

```console
some-command 2>&1 | logcompact --format human
logcompact build.log --format sarif > diagnostics.sarif
logcompact test.log --format github --fail-on error
logcompact build.log --problem-matcher .github/compiler.json
```

Formats are `human`, `json`, `jsonl`, `sarif`, and `github`. Repeated
`--redact-literal` arguments replace matching output text before findings are
deduplicated or serialized. Repeated `--strip-prefix` arguments provide a pure
CI workspace-path projection without giving the reducer filesystem access.
Repeated `--problem-matcher` arguments load self-contained GitHub Actions or
inline VS Code matcher definitions. Later definitions replace earlier owners,
and a custom owner matching a stable built-in owner replaces that built-in.
