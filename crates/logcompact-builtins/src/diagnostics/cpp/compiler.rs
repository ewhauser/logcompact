use crate::{Diagnostic, DiagnosticClass, Location, Severity};

use super::super::common::{normalize_path, split_u32_prefix};

pub(crate) fn parse_diagnostic(line: &str) -> Option<Diagnostic> {
    let line = line
        .trim()
        .strip_prefix("ERROR: ")
        .unwrap_or_else(|| line.trim());
    if !line.contains(':') && !line.contains('(') {
        return None;
    }
    let (path, line_number, column, message) =
        parse_colon_location(line).or_else(|| parse_parenthesized_location(line))?;
    if path.contains(": ") {
        return None;
    }
    let (severity, message) = parse_severity_message(message)?;
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

fn parse_colon_location(line: &str) -> Option<(&str, u32, Option<u32>, &str)> {
    let path_end = path_end(line, ':')?;
    let (line_number, remainder) = split_u32_prefix(&line[path_end + 1..])?;
    let (column, message) = split_u32_prefix(remainder)
        .map_or((None, remainder), |(column, message)| {
            (Some(column), message)
        });
    Some((&line[..path_end], line_number, column, message))
}

fn parse_parenthesized_location(line: &str) -> Option<(&str, u32, Option<u32>, &str)> {
    let path_end = path_end(line, '(')?;
    let remainder = line[path_end..].strip_prefix('(')?;
    let (coordinates, message) = remainder
        .split_once("): ")
        .or_else(|| remainder.split_once("):"))?;
    let (line_number, column) = coordinates.split_once(',').map_or_else(
        || (coordinates.trim().parse::<u32>().ok(), None),
        |(line_number, column)| {
            (
                line_number.trim().parse::<u32>().ok(),
                column.trim().parse::<u32>().ok(),
            )
        },
    );
    Some((&line[..path_end], line_number?, column, message))
}

fn parse_severity_message(message: &str) -> Option<(Severity, &str)> {
    let message = message.trim();
    for (marker, severity) in [
        ("fatal error", Severity::Error),
        ("error", Severity::Error),
        ("warning", Severity::Warning),
        ("note", Severity::Note),
    ] {
        let Some(remainder) = message.strip_prefix(marker) else {
            continue;
        };
        let remainder = remainder
            .strip_prefix(':')
            .or_else(|| remainder.strip_prefix(' '))?
            .trim();
        if !remainder.is_empty() {
            return Some((severity, remainder));
        }
    }
    None
}

pub(crate) fn path_end(line: &str, delimiter: char) -> Option<usize> {
    const EXTENSIONS: [&str; 19] = [
        ".cpp", ".cxx", ".c++", ".cc", ".hpp", ".hxx", ".h++", ".hh", ".ipp", ".tpp", ".inc",
        ".cuh", ".cu", ".mm", ".m", ".C", ".H", ".c", ".h",
    ];
    EXTENSIONS
        .iter()
        .filter_map(|extension| {
            line.rmatch_indices(extension).find_map(|(index, _)| {
                let path_end = index + extension.len();
                line[path_end..].starts_with(delimiter).then_some(path_end)
            })
        })
        .max()
}

fn compact_path(path: &str) -> String {
    normalize_path(path)
}
