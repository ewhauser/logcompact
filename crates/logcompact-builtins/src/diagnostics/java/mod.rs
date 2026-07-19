mod javac;
mod junit;

pub(super) use javac::{parse_diagnostic as parse_compiler_diagnostic, reduce as reduce_compiler};
pub use junit::JavaTestDiagnosticParser;
pub(super) use junit::{exception_message, reduce as reduce_tests};
