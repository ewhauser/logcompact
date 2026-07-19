# Diagnostic testcases

Diagnostic regression cases live beside their language module:

```text
<language>/testcases/<framework>/<case>.yaml
```

Each YAML document has exactly two top-level sections. `output` is the raw tool
or framework output. `assertion.diagnostics` is the exact ordered diagnostic
list expected from the full built-in reducer pipeline.

```yaml
output: |
  src/main.go:12:4: undefined: total
assertion:
  diagnostics:
    - severity: error
      class: compiler
      message: "undefined: total"
      location:
        path: src/main.go
        line: 12
        column: 4
      quality: located
      repetition_count: 1
```

The integration harness discovers cases automatically, compares the batch
result with the assertion, and verifies the same result across input chunk
widths. Keep captured output realistic and review assertion changes as golden
diffs.
