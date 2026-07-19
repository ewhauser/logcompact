use std::collections::BTreeSet;

use crate::{Diagnostic, DiagnosticClass, Location, Severity};

pub(crate) fn reduce(
    input: &str,
    diagnostics: &mut Vec<Diagnostic>,
    messages: &mut BTreeSet<String>,
) {
    let mut parser = JavaTestDiagnosticParser::default();
    for line in input.lines() {
        if let Some(diagnostic) = parser.observe_line(line) {
            messages.insert(diagnostic.message.clone());
            diagnostics.push(diagnostic);
        }
    }
    if let Some(diagnostic) = parser.finish() {
        messages.insert(diagnostic.message.clone());
        diagnostics.push(diagnostic);
    }
}

/// Stateful extractor for Java exceptions followed by JVM stack frames.
#[derive(Debug, Default)]
pub struct JavaTestDiagnosticParser {
    pending: Option<Diagnostic>,
    pending_is_explicit: bool,
    frames_seen: usize,
}

impl JavaTestDiagnosticParser {
    const MAX_STACK_FRAMES: usize = 64;

    /// Observes one normalized test-log line and emits an exception once an
    /// application frame, another exception, or the end of its stack is seen.
    pub fn observe_line(&mut self, line: &str) -> Option<Diagnostic> {
        if let Some((message, explicitly_java)) = parse_exception_line(line) {
            let previous = self.take_confirmed();
            self.pending = Some(Diagnostic {
                severity: Severity::Error,
                class: DiagnosticClass::Test,
                code: None,
                provenance: None,
                message: message.to_owned(),
                location: None,
                quality: crate::EvidenceQuality::Structured,
                repetition_count: 1,
            });
            self.pending_is_explicit = explicitly_java;
            self.frames_seen = 0;
            return previous;
        }
        self.pending.as_ref()?;
        if let Some((location, framework_frame)) = parse_stack_frame(line) {
            self.frames_seen = self.frames_seen.saturating_add(1);
            if !framework_frame {
                let mut diagnostic = self.pending.take()?;
                diagnostic.location = Some(location);
                self.pending_is_explicit = false;
                self.frames_seen = 0;
                return Some(diagnostic);
            }
            if self.frames_seen >= Self::MAX_STACK_FRAMES {
                return self.take_confirmed();
            }
            return None;
        }
        if line.trim().is_empty() {
            return None;
        }
        self.take_confirmed()
    }

    /// Emits an exception that reached end-of-file without an application frame.
    pub fn finish(&mut self) -> Option<Diagnostic> {
        self.take_confirmed()
    }

    fn take_confirmed(&mut self) -> Option<Diagnostic> {
        let confirmed = self.pending_is_explicit || self.frames_seen > 0;
        self.pending_is_explicit = false;
        self.frames_seen = 0;
        if confirmed {
            self.pending.take()
        } else {
            self.pending = None;
            None
        }
    }
}

pub(crate) fn exception_message(line: &str) -> Option<&str> {
    parse_exception_line(line).map(|(message, _)| message)
}

fn parse_exception_line(line: &str) -> Option<(&str, bool)> {
    let mut line = line.trim();
    let mut explicitly_java = false;
    if let Some(remainder) = line.strip_prefix("Exception in thread \"") {
        let (_, remainder) = remainder.split_once("\" ")?;
        line = remainder;
        explicitly_java = true;
    } else if let Some(remainder) = line.strip_prefix("Caused by: ") {
        line = remainder;
        explicitly_java = true;
    }
    let exception_type = line.split_once(':').map_or(line, |(name, _)| name);
    if exception_type.is_empty()
        || exception_type
            .bytes()
            .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$')))
    {
        return None;
    }
    let class_name = exception_type.rsplit('.').next()?;
    let recognized = class_name.ends_with("Error")
        || class_name.ends_with("Exception")
        || class_name.ends_with("Failure");
    (recognized && (explicitly_java || exception_type.contains('.')))
        .then_some((line, explicitly_java))
}

fn parse_stack_frame(line: &str) -> Option<(Location, bool)> {
    let frame = line.trim().strip_prefix("at ")?;
    let (callable, source) = frame.split_once('(')?;
    let source = source.strip_suffix(')')?;
    let (file, line_number) = source.rsplit_once(':')?;
    if !file.ends_with(".java") {
        return None;
    }
    let line_number = line_number.parse::<u32>().ok()?;
    let callable = callable.rsplit_once('/').map_or(callable, |(_, name)| name);
    let class_name = callable.rsplit_once('.')?.0;
    let package = class_name.rsplit_once('.').map(|(package, _)| package);
    let path = package.map_or_else(
        || file.to_owned(),
        |package| format!("{}/{}", package.replace('.', "/"), file),
    );
    let framework_frame = [
        "java.",
        "javax.",
        "jdk.",
        "sun.",
        "junit.",
        "org.junit.",
        "org.hamcrest.",
        "org.opentest4j.",
        "com.google.testing.junit.",
    ]
    .iter()
    .any(|prefix| callable.starts_with(prefix));
    Some((
        Location {
            path,
            line: Some(line_number),
            column: None,
            end_line: None,
            end_column: None,
        },
        framework_frame,
    ))
}
