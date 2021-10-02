use std::{borrow::Borrow, io::{self, Read, Write}};

mod reviewdog;
mod terraform;
use reviewdog as rd;
use terraform as tf;

fn convert<'a>(r: &'a tf::ValidateResult) -> io::Result<reviewdog::DiagnosticResult<'a>> {
    Ok(reviewdog::DiagnosticResult {
        diagnostics: r
            .diagnostics
            .iter()
            .filter_map(|diag| {
                diag.range.as_ref().map(|has_range| rd::Diagnostic {
                    message: &diag.summary,
                    location: rd::Location {
                        path: has_range.filename.to_string(),
                        range: has_range.start.as_ref().map(|has_start| rd::Range {
                            start: rd::Position {
                                line: Some(has_start.line),
                                column: Some(has_start.column),
                            },
                            end: has_range.end.as_ref().map(|has_end| rd::Position {
                                line: Some(has_end.line),
                                column: Some(has_end.column),
                            }),
                        }),
                    },
                    severity: Some(match diag.severity.borrow() {
                        "error" => rd::Severity::ERROR,
                        "warning" => rd::Severity::WARNING,
                        "info" => rd::Severity::INFO,
                        _ => rd::Severity::UNKNOWN_SEVERITY,
                    }),
                    source: None,
                    code: None,
                    suggestions: Vec::new(),
                    original_output: diag.detail.as_ref().map(Borrow::borrow),
                })
            })
            .collect(),
        source: None,
        severity: None,
    })
}

fn main() -> io::Result<()> {
    let mut input = String::with_capacity(128);
    io::stdin().read_to_string(&mut input)?;
    let r: tf::ValidateResult = serde_json::from_str(input.as_str())?;
    serde_json::to_writer(io::stdout(), &convert(&r)?)?;
    Ok(())
}
