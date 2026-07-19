use crate::{Diagnostic, DiagnosticClass, Location, Severity};

use super::super::common::{bounded_text, normalize_path, split_u32_prefix};
use super::compiler::path_end;

pub(crate) fn reduce(input: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut parser = CppLinkerDiagnosticParser::default();
    for line in input.lines() {
        if let Some(diagnostic) = parser.observe_line(line) {
            diagnostics.push(diagnostic);
        }
    }
}

#[derive(Debug, Default)]
struct CppLinkerDiagnosticParser {
    apple_undefined_symbols: bool,
    symbols_seen: usize,
}

impl CppLinkerDiagnosticParser {
    const MAX_SYMBOLS: usize = 64;

    fn observe_line(&mut self, line: &str) -> Option<Diagnostic> {
        let line = line.trim();
        if line.starts_with("Undefined symbols for architecture ") {
            self.apple_undefined_symbols = true;
            return None;
        }
        if self.apple_undefined_symbols {
            if let Some(symbol) = parse_apple_undefined_symbol(line) {
                if self.symbols_seen >= Self::MAX_SYMBOLS {
                    return None;
                }
                self.symbols_seen = self.symbols_seen.saturating_add(1);
                return Some(linker_diagnostic(
                    format!("undefined symbol: {}", bounded_text(symbol, 1_000)),
                    None,
                ));
            }
            if line.starts_with("ld:") || line.starts_with("clang:") {
                self.apple_undefined_symbols = false;
            }
        }
        parse_diagnostic(line)
    }
}

fn parse_apple_undefined_symbol(line: &str) -> Option<&str> {
    let symbol = line.strip_prefix('"')?;
    let (symbol, _) = symbol.split_once("\", referenced from:")?;
    (!symbol.is_empty()).then_some(symbol)
}

pub(crate) fn parse_diagnostic(line: &str) -> Option<Diagnostic> {
    let line = line.trim();
    if let Some(index) = line.find("undefined reference to ") {
        let symbol = trim_linker_symbol(&line[index + "undefined reference to ".len()..]);
        if symbol.is_empty() {
            return None;
        }
        return Some(linker_diagnostic(
            format!("undefined reference to {symbol}"),
            parse_linker_location(&line[..index]),
        ));
    }
    if let Some(index) = line.find("undefined symbol:") {
        let symbol = trim_linker_symbol(&line[index + "undefined symbol:".len()..]);
        if symbol.is_empty() {
            return None;
        }
        return Some(linker_diagnostic(
            format!("undefined symbol: {symbol}"),
            None,
        ));
    }
    if let Some(index) = line.find("unresolved external symbol ") {
        let remainder = &line[index + "unresolved external symbol ".len()..];
        let symbol = remainder
            .split_once(" referenced in ")
            .map_or(remainder, |(symbol, _)| symbol)
            .trim();
        if symbol.is_empty() {
            return None;
        }
        return Some(linker_diagnostic(
            format!("unresolved external symbol {symbol}"),
            None,
        ));
    }
    None
}

fn parse_linker_location(prefix: &str) -> Option<Location> {
    let path_end = path_end(prefix, ':')?;
    let (line_number, _) = split_u32_prefix(&prefix[path_end + 1..])?;
    let path = prefix[..path_end]
        .rsplit_once(": ")
        .map_or(&prefix[..path_end], |(_, path)| path);
    Some(Location {
        path: compact_path(path),
        line: Some(line_number),
        column: None,
        end_line: None,
        end_column: None,
    })
}

fn trim_linker_symbol(symbol: &str) -> &str {
    symbol
        .trim()
        .trim_end_matches(':')
        .trim_matches(|character| matches!(character, '`' | '\'' | '"'))
}

fn linker_diagnostic(message: String, location: Option<Location>) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        class: DiagnosticClass::Compiler,
        code: None,
        provenance: None,
        message,
        location,
        quality: crate::EvidenceQuality::Structured,
        repetition_count: 1,
    }
}

fn compact_path(path: &str) -> String {
    normalize_path(path)
}
