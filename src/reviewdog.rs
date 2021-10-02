// Definitions of reviewdog diagnostic format
use serde::Serialize;

// Result of diagnostic tool such as a compiler or a linter.
// It's intended to be used as top-level structured format which represents a
// whole result of a diagnostic tool.
#[derive(Debug, Serialize)]
pub struct DiagnosticResult<'a> {
    pub diagnostics: Vec<Diagnostic<'a>>,

    // The source of diagnostics, e.g. 'typescript' or 'super lint'.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source<'a>>,

    // This diagnostics' overall severity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
}

// Represents a diagnostic, such as a compiler error or warning.
// It's intended to be used as structured format which represents a
// diagnostic and can be used as stream of input/output such as jsonl.
// This message should be self-contained to report a diagnostic.
#[derive(Debug, Serialize)]
pub struct Diagnostic<'a> {
    // The diagnostic's message.
    pub message: &'a str,

    // Location at which this diagnostic message applies.
    pub location: Location,

    // This diagnostic's severity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,

    // The source of this diagnostic, e.g. 'typescript' or 'super lint'.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source<'a>>,

    // This diagnostic's rule code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<Code<'a>>,

    // Suggested fixes to resolve this diagnostic.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<Suggestion<'a>>,

    // Experimental: If this diagnostic is converted from other formats,
    // original_output represents the original output which corresponds to this
    // diagnostic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_output: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub enum Severity {
    UNKNOWN_SEVERITY,
    ERROR,
    WARNING,
    INFO,
}

#[derive(Debug, Serialize)]
pub struct Location {
    // File path. It could be either absolute path or relative path.
    pub path: String,

    // Range in the file path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
}

// A range in a text document expressed as start and end positions.

// The end position is *exclusive*. It might be a bit unnatural for you or for
// some diagnostic tools to use exlusive range, but it's necessary to represent
// zero-width range especially when using it in Suggestion context to support
// code insertion.
// pub Example: "14" in "haya14busa"
//   pub start: { pub line: 1, pub column: 5 }
//   pub end:   { pub line: 1, pub column: 7 } # <= Exclusive
//
// |h|a|y|a|1|4|b|u|s|a|
// 1 2 3 4 5 6 7 8 9 0 1
//         ^---^
// haya14busa
//     ^^
//
// If you want to specify a range that
// contains a line including the line ending character(s), then use an end
// position denoting the start of the next line.
// pub Example:
//   pub start: { pub line: 5, pub column: 23 }
//   pub end:   { pub line: 6, pub column: 1 }
//
// If both start and end position omit column value, it's
// handled as linewise and the range includes end position (line) as well.
// pub Example:
//   pub start: { pub line: 5 }
//   pub end:   { pub line: 6 }
// The above example represents range start from line 5 to the end of line 6
// including EOL.
//
// Examples for line pub range:
//  Text example. <line>|<line content>(line breaking)
//  1|abc\r\n
//  2|def\r\n
//  3|ghi\r\n
//
// pub start: { pub line: 2 }
//   => "abc"
//
// pub start: { pub line: 2 }
// pub end:   { pub line: 2 }
//   => "abc"
//
// pub start: { pub line: 2 }
// pub end:   { pub line: 3 }
//   => "abc\r\ndef"
//
// pub start: { pub line: 2 }
// pub end:   { pub line: 3, pub column: 1 }
//   => "abc\r\n"

// pub start: { pub line: 2, pub column: 1 }
// pub end:   { pub line: 2, pub column: 4 }
//   => "abc" (without line-break)
#[derive(Debug, Serialize)]
pub struct Range {
    // Required.
    pub start: Position,

    // end can be omitted. Then the range is handled as zero-length (start == end).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<Position>,
}

#[derive(Debug, Serialize)]
pub struct Position {
    // Line number, starting at 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,

    // Column number, starting at 1 (byte count in UTF-8).
    // pub Example: 'a𐐀b'
    //  The column of pub a: 1
    //  The column of 𐐀: 2
    //  The column of pub b: 6 since 𐐀 is represented with 4 bytes in UTF-8.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

// Suggestion represents a suggested text manipulation to resolve a diagnostic
// problem.
//
// Insert example ('hayabusa' -> 'haya15busa'):
//   range {
//     start {
//       pub line: 1
//       pub column: 5
//     }
//     end {
//       pub line: 1
//       pub column: 5
//     }
//   }
//   pub text: 15
// |h|a|y|a|b|u|s|a|
// 1 2 3 4 5 6 7 8 9
//         ^--- insert '15'
//
// Update example ('haya15busa' -> 'haya14busa'):
//   range {
//     start {
//       pub line: 1
//       pub column: 5
//     }
//     end {
//       pub line: 1
//       pub column: 7
//     }
//   }
//   pub text: 14
// |h|a|y|a|1|5|b|u|s|a|
// 1 2 3 4 5 6 7 8 9 0 1
//         ^---^ replace with '14'
#[derive(Debug, Serialize)]
pub struct Suggestion<'a> {
    // Range at which this suggestion applies.
    // To insert text into a document create a range where start == end.
    pub range: Range,

    // A suggested text which replace the range.
    // For delete operations use an empty string.
    pub text: &'a str,
}

#[derive(Debug, Serialize)]
pub struct Source<'a> {
    // A human-readable string describing the source of diagnostics, e.g.
    // 'typescript' or 'super lint'.
    pub name: &'a str,
    // URL to this source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub struct Code<'a> {
    // This rule's code/identifier.
    pub value: &'a str,

    // A URL to open with more information about this rule code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<&'a str>,
}
