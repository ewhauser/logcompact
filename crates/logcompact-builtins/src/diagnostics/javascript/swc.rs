use std::collections::BTreeSet;

use crate::{Diagnostic, DiagnosticClass, Location, Severity};

use super::super::common::bounded_text;
use super::node::compact_path;

#[derive(Debug, Default)]
pub(crate) struct SwcParseOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) consumed_lines: BTreeSet<String>,
}

pub(crate) fn reduce(input: &str, output: &mut SwcParseOutput) {
    *output = parse_diagnostics(input);
}

pub(crate) fn parse_diagnostics(input: &str) -> SwcParseOutput {
    const MAX_HEADER_DISTANCE: usize = 3;
    const MAX_FRAME_LINES: usize = 8;

    let lines = input.lines().collect::<Vec<_>>();
    let mut output = SwcParseOutput::default();
    for (location_index, line) in lines.iter().enumerate() {
        let Some(location) = parse_source_frame_location(line) else {
            continue;
        };
        let Some((header_index, severity, message)) = lines[..location_index]
            .iter()
            .enumerate()
            .rev()
            .take(MAX_HEADER_DISTANCE)
            .find_map(|(index, line)| {
                parse_message_header(line).map(|(severity, message)| (index, severity, message))
            })
        else {
            continue;
        };

        let mut frame_end = location_index;
        let mut source_line = None;
        let mut first_source_line = None;
        let mut caret_line = None;
        for (index, frame_line) in lines
            .iter()
            .enumerate()
            .skip(location_index + 1)
            .take(MAX_FRAME_LINES)
        {
            if parse_message_header(frame_line).is_some()
                || parse_source_frame_location(frame_line).is_some()
            {
                break;
            }
            frame_end = index;
            output.consumed_lines.insert(frame_line.trim().to_owned());
            if let Some(line_number) = source_line_number(frame_line) {
                let bounded = bounded_text(frame_line.trim(), 256);
                first_source_line.get_or_insert_with(|| bounded.clone());
                if location.line == Some(line_number) {
                    source_line = Some(bounded);
                }
            } else if caret_line.is_none() && is_caret_line(frame_line) {
                caret_line = Some(bounded_text(frame_line.trim(), 256));
            }
            if is_frame_terminator(frame_line) {
                break;
            }
        }

        output
            .consumed_lines
            .insert(lines[header_index].trim().to_owned());
        output
            .consumed_lines
            .insert(lines[location_index].trim().to_owned());
        let (context, context_lines) = failure_context(&lines, header_index, frame_end);
        for context_line in context_lines {
            output
                .consumed_lines
                .insert(lines[context_line].trim().to_owned());
        }

        let mut message = bounded_text(message, 1_024);
        if let Some(context) = context {
            message.push_str("\nSWC context: ");
            message.push_str(&bounded_text(&context, 512));
        }
        if let Some(source_line) = source_line.or(first_source_line) {
            message.push('\n');
            message.push_str(&source_line);
        }
        if let Some(caret_line) = caret_line {
            message.push('\n');
            message.push_str(&caret_line);
        }
        output.diagnostics.push(Diagnostic {
            severity,
            class: DiagnosticClass::Compiler,
            code: Some("swc.parser".to_owned()),
            provenance: None,
            message,
            location: Some(location),
            quality: crate::EvidenceQuality::Located,
            repetition_count: 1,
        });
    }
    output
}

fn parse_message_header(line: &str) -> Option<(Severity, &str)> {
    let line = line.trim();
    let (severity, message) = if let Some(message) = line.strip_prefix("error:") {
        (Severity::Error, message)
    } else if let Some(message) = line.strip_prefix("warning:") {
        (Severity::Warning, message)
    } else if let Some(message) = strip_icon(line, 'x').or_else(|| strip_icon(line, '×')) {
        (Severity::Error, message)
    } else {
        (Severity::Warning, strip_icon(line, '!')?)
    };
    let message = message.trim();
    (!message.is_empty()).then_some((severity, message))
}

fn strip_icon(line: &str, icon: char) -> Option<&str> {
    let remainder = line.strip_prefix(icon)?;
    remainder
        .chars()
        .next()
        .is_some_and(char::is_whitespace)
        .then(|| remainder.trim_start())
}

fn parse_source_frame_location(line: &str) -> Option<Location> {
    let line = line.trim();
    let opening = line.find('[')?;
    let marker = line[..opening].trim_end();
    if !marker.ends_with(",-") && !marker.ends_with("╭─") {
        return None;
    }
    let closing = line[opening + 1..].find(']')? + opening + 1;
    let coordinates = &line[opening + 1..closing];
    let (path_and_line, column) = coordinates.rsplit_once(':')?;
    let (path, line_number) = path_and_line.rsplit_once(':')?;
    if path.trim().is_empty() {
        return None;
    }
    Some(Location {
        path: compact_path(path.trim()),
        line: Some(line_number.parse::<u32>().ok()?),
        column: Some(column.parse::<u32>().ok()?),
        end_line: None,
        end_column: None,
    })
}

fn source_line_number(line: &str) -> Option<u32> {
    let line = line.trim();
    let (line_number, _) = line.split_once('|').or_else(|| line.split_once('│'))?;
    let line_number = line_number.trim();
    (!line_number.is_empty() && line_number.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| line_number.parse::<u32>().ok())
        .flatten()
}

fn is_caret_line(line: &str) -> bool {
    let line = line.trim();
    line.contains('^')
        && (line.starts_with(':')
            || line
                .split_once('|')
                .is_some_and(|(prefix, _)| prefix.trim().is_empty()))
}

fn is_frame_terminator(line: &str) -> bool {
    let line = line.trim();
    (line.starts_with('`') && line.contains("---")) || (line.starts_with('╰') && line.contains('─'))
}

fn failure_context(
    lines: &[&str],
    header_index: usize,
    frame_end: usize,
) -> (Option<String>, Vec<usize>) {
    const MAX_CONTEXT_DISTANCE: usize = 12;
    const MAX_CONTEXT_ITEMS: usize = 2;

    let mut contexts = Vec::new();
    let mut consumed = Vec::new();
    let preceding_start = header_index.saturating_sub(MAX_CONTEXT_DISTANCE);
    for (index, line) in lines
        .iter()
        .enumerate()
        .take(header_index)
        .skip(preceding_start)
    {
        if let Some(context) = parse_context_line(line)
            && !contexts.contains(&context)
        {
            contexts.push(context);
            consumed.push(index);
        }
        if contexts.len() >= MAX_CONTEXT_ITEMS {
            break;
        }
    }
    for (index, line) in lines
        .iter()
        .enumerate()
        .skip(frame_end + 1)
        .take(MAX_CONTEXT_DISTANCE)
    {
        if parse_message_header(line).is_some() || parse_source_frame_location(line).is_some() {
            break;
        }
        if let Some(context) = parse_context_line(line)
            && !contexts.contains(&context)
        {
            contexts.push(context);
            consumed.push(index);
        }
        if contexts.len() >= MAX_CONTEXT_ITEMS {
            break;
        }
    }
    contexts.truncate(MAX_CONTEXT_ITEMS);
    (
        (!contexts.is_empty()).then(|| contexts.join("; ")),
        consumed,
    )
}

fn parse_context_line(line: &str) -> Option<String> {
    if parse_message_header(line).is_some() {
        return None;
    }
    let mut line = line.trim();
    if let Some(remainder) = line.strip_prefix("Caused by:") {
        line = remainder.trim();
    }
    if let Some((number, remainder)) = line.split_once(": ")
        && !number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
    {
        line = remainder.trim();
    }
    let lower = line.to_ascii_lowercase();
    let is_context = lower.starts_with("failed to process")
        || lower.starts_with("failed to parse")
        || lower.starts_with("failed to transform")
        || lower.starts_with("failed to invoke plugin")
        || lower.starts_with("failed to handle")
        || (lower.contains("plugin") && lower.contains("failed"));
    (is_context && !line.is_empty()).then(|| line.to_owned())
}

pub(crate) fn is_action_wrapper(line: &str) -> bool {
    let line = line.trim();
    let lower = line.to_ascii_lowercase();
    line.starts_with("ERROR:")
        && (lower.contains("build did not complete successfully")
            || lower.contains("failed: error executing")
            || (lower.contains("swc")
                && (lower.contains("failed:") || lower.contains("action failed"))))
}
