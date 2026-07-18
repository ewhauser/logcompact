use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tokencompact"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("tokencompact should start");
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
