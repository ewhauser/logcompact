mod node;
mod swc;

pub use node::JavaScriptTestDiagnosticParser;
pub(super) use node::{compact_path, exception_message, reduce as reduce_tests};
pub(crate) use swc::SwcParseOutput;
#[cfg(feature = "test-support")]
pub(crate) use swc::parse_diagnostics as parse_swc_diagnostics;
pub(super) use swc::{is_action_wrapper as is_swc_action_wrapper, reduce as reduce_swc};
