use crate::{Diagnostic, DiagnosticClass, Location, Severity};

use super::super::common::{bounded_text, normalize_path};

const MAX_MESSAGE_BYTES: usize = 1_000;
const MAX_PATH_BYTES: usize = 512;

pub(crate) fn is_failure_summary(line: &str) -> bool {
    failure_summary_parts(line).is_some()
}

/// Parses one pytest short-summary failure into bounded test evidence.
pub(crate) fn parse_failure_summary(line: &str) -> Option<Diagnostic> {
    let (path, message) = failure_summary_parts(line)?;
    Some(Diagnostic {
        severity: Severity::Error,
        class: DiagnosticClass::Test,
        code: Some("pytest.failure".to_owned()),
        provenance: None,
        message: bounded_text(message, MAX_MESSAGE_BYTES),
        location: Some(Location {
            path: bounded_text(&normalize_path(path), MAX_PATH_BYTES),
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        }),
        quality: crate::EvidenceQuality::Structured,
        repetition_count: 1,
    })
}

fn failure_summary_parts(line: &str) -> Option<(&str, &str)> {
    let summary = line.trim().strip_prefix("FAILED ")?;
    let (node_id, message) = summary.split_once(" - ")?;
    let node_id = node_id.trim();
    let message = message.trim();
    let (path, _) = node_id.split_once("::")?;
    (!path.is_empty() && path.ends_with(".py") && !message.is_empty()).then_some((path, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_pytest_short_summary() {
        let diagnostic = parse_failure_summary(
            "FAILED tests/test_invoice.py::test_total - AssertionError: expected 45",
        )
        .unwrap();

        assert_eq!(diagnostic.code.as_deref(), Some("pytest.failure"));
        assert_eq!(diagnostic.message, "AssertionError: expected 45");
        assert_eq!(diagnostic.location.unwrap().path, "tests/test_invoice.py");
    }

    #[test]
    fn rejects_non_python_failure_summaries() {
        let line = "FAILED tests/invoice.txt::test_total - assert 40 == 45";
        assert!(parse_failure_summary(line).is_none());
        assert!(!is_failure_summary(line));
    }
}
