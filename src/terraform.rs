// Terraform JSON output definitions
use serde::Deserialize;
use std::borrow::Cow;

#[derive(Debug, Deserialize)]
pub struct ValidateResult<'a> {
    #[serde(borrow)]
    pub format_version: Cow<'a, str>,
    pub valid: bool,
    pub error_count: u32,
    pub warning_count: u32,
    pub diagnostics: Vec<Diagnostic<'a>>,
}

#[derive(Debug, Deserialize)]
pub struct Diagnostic<'a> {
    #[serde(borrow)]
    pub severity: Cow<'a, str>,
    #[serde(borrow)]
    pub summary: Cow<'a, str>,
    #[serde(borrow)]
    pub detail: Option<Cow<'a, str>>,
    pub range: Option<Range<'a>>,
    pub snippet: Option<Snippet<'a>>,
}

#[derive(Debug, Deserialize)]
pub struct Range<'a> {
    #[serde(borrow)]
    pub filename: Cow<'a, str>,
    pub start: Option<SourcePosition>,
    pub end: Option<SourcePosition>,
}

#[derive(Debug, Deserialize)]
pub struct SourcePosition {
    pub byte: u32,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Deserialize)]
pub struct Snippet<'a> {
    pub context: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub code: Cow<'a, str>,
    pub start_line: u32,
    pub highlight_start_offset: u32,
    pub highlight_end_offset: u32,
    pub values: Vec<Expression<'a>>,
}

#[derive(Debug, Deserialize)]
pub struct Expression<'a> {
    #[serde(borrow)]
    pub traversal: Cow<'a, str>,
    #[serde(borrow)]
    pub statement: Cow<'a, str>,
}
