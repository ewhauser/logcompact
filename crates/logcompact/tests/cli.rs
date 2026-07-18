use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_logcompact"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("logcompact should start");
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(input)
        .expect("fixture should be written");
    child.wait_with_output().expect("process should finish")
}

#[test]
fn reduces_stdin_to_human_output() {
    let output = run(&[], b"src/main.go:12:4: undefined: total\n");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "src/main.go:12:4: error: undefined: total\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn emits_redacted_json_and_honors_failure_threshold() {
    let output = run(
        &[
            "--format",
            "json",
            "--redact-literal",
            "secret",
            "--fail-on",
            "error",
        ],
        b"secret/main.go:12:4: undefined: secret\n",
    );

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("secret"));
    assert!(stdout.contains("[REDACTED]/main.go"));
    assert!(stdout.contains("undefined: [REDACTED]"));
    assert!(output.stderr.is_empty());
}

#[test]
fn loads_github_problem_matcher_definitions() {
    let mut matcher = tempfile::NamedTempFile::new().unwrap();
    matcher
        .write_all(
            br#"{
                "problemMatcher": [{
                    "owner": "widget",
                    "source": "widget compiler",
                    "pattern": {
                        "regexp": "^MATCH (.+):(\\d+):(\\d+)-(\\d+):(\\d+) \\[(warning|error)\\] (.+)$",
                        "file": 1,
                        "line": 2,
                        "column": 3,
                        "endLine": 4,
                        "endColumn": 5,
                        "severity": 6,
                        "message": 7
                    }
                }]
            }"#,
        )
        .unwrap();
    let path = matcher.path().to_string_lossy().into_owned();
    let output = run(
        &["--problem-matcher", &path, "--format", "json"],
        b"MATCH src/widget.dsl:4:2-5:8 [warning] unknown widget\n",
    );

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = &value["diagnostics"][0];
    assert_eq!(diagnostic["severity"], "warning");
    assert_eq!(diagnostic["message"], "unknown widget");
    assert_eq!(diagnostic["location"]["path"], "src/widget.dsl");
    assert_eq!(diagnostic["location"]["line"], 4);
    assert_eq!(diagnostic["location"]["end_line"], 5);
    assert_eq!(diagnostic["provenance"]["label"], "widget compiler");
}
