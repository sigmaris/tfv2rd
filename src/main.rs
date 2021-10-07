use std::borrow::{Borrow, Cow, ToOwned};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use log::warn;
use path_absolutize::Absolutize;
use pathdiff::diff_paths;
use rd::Source;
use structopt::StructOpt;

mod reviewdog;
mod terraform;
use reviewdog as rd;
use terraform as tf;

#[derive(Debug)]
enum OutputFormat {
    RdJson,
    RdJsonL,
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "rdjson" => Ok(OutputFormat::RdJson),
            "rdjsonl" => Ok(OutputFormat::RdJsonL),
            _ => Err(format!("Unknown output format '{}'", s)),
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(
    name="tfv2rd",
    about="Converts terraform validate JSON output to Reviewdog Diagnostic Format.",
    version=env!("CARGO_PKG_VERSION")
)]
struct Opt {
    #[structopt(short, long, requires("workdir"), parse(from_os_str))]
    /// Converts paths to be relative to this base directory. Requires --workdir.
    basedir: Option<PathBuf>,

    #[structopt(short, long, parse(from_os_str))]
    /// Working directory terraform validate was run in, for path conversion.
    workdir: Option<PathBuf>,

    #[structopt(short, long)]
    /// Omit diagnostics in the output if errors are encountered converting them to Reviewdog format, instead of exiting with an error.
    skip_errors: bool,

    #[structopt(short, long, default_value = "rdjsonl")]
    /// Format for output, either RdJSONL (one JSON Diagnostic object per line, default) or RdJSON (a single RdJSON object).
    format: OutputFormat,

    #[structopt(short, long, default_value = "terraform validate")]
    /// Value for "source" of the diagnostics to report in the output.
    source: String,
}

fn convert<'a>(
    tf_result: &'a tf::ValidateResult,
    path_converter: &dyn Fn(&str) -> io::Result<String>,
    skip_errors: bool,
    source: &'a str,
) -> io::Result<Vec<reviewdog::Diagnostic<'a>>> {
    let rd_diags_iter = tf_result
        .diagnostics
        .iter()
        .filter(|diag| {
            if diag.range.is_none() {
                warn!("The TF {} {} has no source file location and cannot be reported as RdJSON, it will be ignored.", diag.severity, diag.summary);
                false
            } else {
                true
            }
        })
        .map(|diag| convert_one_diag(diag, path_converter, source));

    if skip_errors {
        rd_diags_iter
            .filter(|r| {
                if let Err(e) = r {
                    warn!(
                        "A TF diagnostic could not be converted and will be ignored: {}",
                        e
                    );
                    false
                } else {
                    true
                }
            })
            .collect::<io::Result<_>>()
    } else {
        rd_diags_iter.collect::<io::Result<_>>()
    }
}

fn convert_one_diag<'a>(
    diag: &'a tf::Diagnostic,
    path_converter: &dyn Fn(&str) -> Result<String, io::Error>,
    source: &'a str,
) -> Result<rd::Diagnostic<'a>, io::Error> {
    let has_range = diag.range.as_ref().unwrap();
    Ok(rd::Diagnostic {
        message: diag.summary,
        location: rd::Location {
            path: path_converter(has_range.filename.as_ref())?,
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
            "error" => rd::Severity::Error,
            "warning" => rd::Severity::Warning,
            "info" => rd::Severity::Info,
            _ => rd::Severity::UnknownSeverity,
        }),
        source: Some(rd::Source {
            name: source,
            url: None,
        }),
        code: None,
        suggestions: Vec::new(),
        original_output: diag.detail,
    })
}

fn path_to_string(pb: PathBuf) -> io::Result<String> {
    pb.into_os_string().into_string().map_err(|bad_path| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Can't encode path {:?} as UTF-8", bad_path),
        )
    })
}

fn path_fn<F>(f: F) -> F
where
    F: for<'a> Fn(&'a str) -> io::Result<Cow<'a, Path>>,
{
    f
}

fn main() -> io::Result<()> {
    pretty_env_logger::init();
    let opt = Opt::from_args();
    let path_converter: PathConverter = make_path_converter(&opt)?;

    let mut input = String::with_capacity(128);
    io::stdin().read_to_string(&mut input)?;
    let r: tf::ValidateResult = serde_json::from_str(input.as_str())?;
    let all_diags = convert(&r, &path_converter, opt.skip_errors, opt.source.as_str())?;

    match opt.format {
        OutputFormat::RdJson => {
            let overall_sev = if r.error_count > 0 {
                rd::Severity::Error
            } else if r.warning_count > 0 {
                rd::Severity::Warning
            } else {
                rd::Severity::Info
            };
            serde_json::to_writer(
                io::stdout(),
                &rd::DiagnosticResult {
                    diagnostics: all_diags,
                    severity: Some(overall_sev),
                    source: Some(Source {
                        name: opt.source.as_str(),
                        url: None,
                    }),
                },
            )?
        }
        OutputFormat::RdJsonL => {
            let mut stdout = io::stdout();
            for diag in all_diags {
                serde_json::to_writer(&stdout, &diag)?;
                stdout.write_all(b"\n")?;
            }
        }
    }
    Ok(())
}

type PathConverter = Box<dyn Fn(&str) -> Result<String, io::Error>>;

fn make_path_converter(opt: &Opt) -> Result<PathConverter, io::Error> {
    Ok(if let Some(workdir) = &opt.workdir {
        // If we have a workdir set we can convert relative paths in Terraform output to absolute paths
        let abs_work = workdir.absolutize()?.to_path_buf();
        let absolutize_path =
            path_fn(move |filename| Path::new(filename).absolutize_from(&abs_work));

        if let Some(basedir) = &opt.basedir {
            // If we also have a basedir we can further convert the absolute paths to be relative to the root of the project or repository
            let abs_base = basedir.absolutize()?.to_path_buf();
            let relativize_path = move |filename: &str| {
                let absolute = absolutize_path(filename)?;
                diff_paths(absolute, &abs_base)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::Other,
                            format!(
                                "Can't convert '{}' into a path relative to '{}'",
                                filename,
                                abs_base.to_string_lossy()
                            ),
                        )
                    })
                    .and_then(path_to_string)
            };
            Box::new(relativize_path)
        } else {
            // Otherwise just return the absolute paths
            Box::new(move |filename| {
                absolutize_path(filename)
                    .map(|p| p.to_path_buf())
                    .and_then(path_to_string)
            })
        }
    } else {
        // If we have no workdir we can only pass the paths straight through
        Box::new(|filename| Ok(filename.to_owned()))
    })
}

#[cfg(test)]
mod tests {
    use jsonschema::{Draft, JSONSchema};
    use serde_json::json;

    use super::*;

    static RD_SCHEMA: &str = include_str!("../testdata/DiagnosticResult.jsonschema");
    static TF_NO_RANGE: &str = include_str!("../testdata/no_range.json");
    static TF_MODS_IN_PARENT: &str = include_str!("../testdata/modules_parent_dir.json");
    static TF_QUOTING: &str = include_str!("../testdata/quoting.json");

    fn passthru_path(s: &str) -> Result<String, io::Error> {
        Ok(s.to_owned())
    }

    #[test]
    fn test_no_range() {
        let result: tf::ValidateResult =
            serde_json::from_str(TF_NO_RANGE).expect("Test data should be parsed");
        let all_diags = convert(&result, &Box::new(passthru_path), false, "test_no_range")
            .expect("Test data should be converted");
        assert_eq!(
            all_diags.len(),
            1,
            "Only one out of the two diagnostics should be included"
        );
        assert_eq!(
            serde_json::to_value(all_diags.iter().next().unwrap())
                .expect("Converted data should be serialized"),
            json!({
                "message": "Invalid quoted type constraints",
                "location": {
                    "path": "variables.tf",
                    "range": {
                        "start": {"line": 8,"column": 17},
                        "end": {"line": 8, "column": 25}
                    }
                },
                "severity": "ERROR",
                "source": {"name": "test_no_range"},
                "original_output": "Terraform 0.11 and earlier required type constraints to be given in quotes, but that form is now deprecated and will be removed in a future version of Terraform. Remove the quotes around \"string\"."
            })
        );
    }

    #[test]
    fn test_quoting_errors() {
        let result: tf::ValidateResult =
            serde_json::from_str(TF_QUOTING).expect("Test data should be parsed");
        let all_diags = convert(&result, &Box::new(passthru_path), false, "test_quoting")
            .expect("Test data should be converted");
        assert_eq!(all_diags.len(), 2, "Two diagnostics should be included");
        assert_eq!(
            serde_json::to_value(all_diags).expect("Converted data should be serialized"),
            json!([
                {
                    "message": "Invalid quoted type constraints",
                    "location": {
                        "path": "variables.tf",
                        "range": {
                            "start": {"line": 2,"column": 17},
                            "end": {"line": 2, "column": 25}
                        }
                    },
                    "severity": "ERROR",
                    "source": {"name": "test_quoting"},
                    "original_output": "Terraform 0.11 and earlier required type constraints to be given in quotes, but that form is now deprecated and will be removed in a future version of Terraform. Remove the quotes around \"string\"."
                },
                {
                    "message": "Invalid quoted type constraints",
                    "location": {
                        "path": "variables.tf",
                        "range": {
                            "start": {"line": 8,"column": 17},
                            "end": {"line": 8, "column": 25}
                        }
                    },
                    "severity": "ERROR",
                    "source": {"name": "test_quoting"},
                    "original_output": "Terraform 0.11 and earlier required type constraints to be given in quotes, but that form is now deprecated and will be removed in a future version of Terraform. Remove the quotes around \"string\"."
                },
            ])
        );
    }

    #[test]
    fn schema_validate_output() {
        let compiled_schema = JSONSchema::options()
            .with_draft(Draft::Draft4)
            .compile(&serde_json::from_str(RD_SCHEMA).expect("Schema should be parsed"))
            .expect("A valid schema");
        for input in [TF_MODS_IN_PARENT, TF_NO_RANGE, TF_QUOTING] {
            let tf_in: tf::ValidateResult =
                serde_json::from_str(input).expect("Test data can be parsed");
            let all_diags = convert(
                &tf_in,
                &Box::new(passthru_path),
                false,
                "schema_validate_output",
            )
            .expect("Diagnostics can be converted");
            let rd_diag = rd::DiagnosticResult {
                diagnostics: all_diags,
                severity: Some(rd::Severity::Error),
                source: Some(Source {
                    name: "schema_validate_output",
                    url: None,
                }),
            };
            compiled_schema
                .validate(
                    &serde_json::to_value(rd_diag).expect("DiagnosticResult can be serialized"),
                )
                .map_err(|d| d.collect::<Vec<_>>())
                .expect("DiagnosticResult conforms to RdJSON schema");
        }
    }
}
