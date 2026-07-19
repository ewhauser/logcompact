use crate::{Diagnostic, DiagnosticClass, Location, Severity};

use super::super::common::{normalize_path, split_u32_prefix};

pub(crate) fn reduce(input: &str, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.extend(parse_diagnostics(input));
}

fn parse_diagnostics(input: &str) -> Vec<Diagnostic> {
    const MAX_CONTEXT_LINES: usize = 8;
    let lines = input.lines().collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(mut diagnostic) = parse_diagnostic(line) else {
            continue;
        };
        if diagnostic
            .message
            .eq_ignore_ascii_case("cannot find symbol")
        {
            for context in lines.iter().skip(index + 1).take(MAX_CONTEXT_LINES) {
                if parse_diagnostic(context).is_some() || context.trim_start().starts_with("ERROR:")
                {
                    break;
                }
                if let Some(symbol) = context.trim().strip_prefix("symbol:") {
                    diagnostic.message = format!(
                        "cannot find symbol: {}",
                        symbol.split_whitespace().collect::<Vec<_>>().join(" ")
                    );
                    break;
                }
            }
        }
        diagnostics.push(diagnostic);
    }
    diagnostics
}

pub(crate) fn parse_diagnostic(line: &str) -> Option<Diagnostic> {
    let marker = line.rfind(".java:")?;
    let path_end = marker + ".java".len();
    let path = line[..path_end]
        .trim()
        .strip_prefix("ERROR: ")
        .unwrap_or_else(|| line[..path_end].trim());
    let (line_number, remainder) = split_u32_prefix(&line[path_end + 1..])?;
    let (column, message) = split_u32_prefix(remainder)
        .map_or((None, remainder), |(column, message)| {
            (Some(column), message)
        });
    let message = message.trim();
    let (severity, message) = if let Some(message) = message.strip_prefix("error:") {
        (Severity::Error, message.trim())
    } else {
        let message = message.strip_prefix("warning:")?;
        (Severity::Warning, message.trim())
    };
    if message.is_empty() {
        return None;
    }
    Some(Diagnostic {
        severity,
        class: DiagnosticClass::Compiler,
        code: None,
        provenance: None,
        message: message.to_owned(),
        location: Some(Location {
            path: compact_path(path),
            line: Some(line_number),
            column,
            end_line: None,
            end_column: None,
        }),
        quality: crate::EvidenceQuality::Located,
        repetition_count: 1,
    })
}

fn compact_path(path: &str) -> String {
    normalize_path(path)
}
