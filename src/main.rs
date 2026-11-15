//! Minimal CLI: `apriwave-cli [--ast] [--json] [--warn=<CODE>]… "<canonical text>"`.
//!
//! Prints the canonical form of every accepted period; with `--ast` also the AST and
//! the stage 2 analysis (Debug), with `--json` the deterministic structured export.
//! `--warn=<CODE>` (v4.1, repeatable) downgrades an error code to a warning
//! (structural errors stay blocking); warnings are printed as `WARN`.
//! Exit code: 0 = ok, 1 = diagnostic or error, 2 = usage.

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::indexing_slicing
)]

use std::process::ExitCode;

use apriwave::export::export_analyzed_json;
use apriwave::printer::render_period;
use apriwave::{ErrorCode, ParserConfig, Session};

const USAGE: &str =
    "usage: apriwave-cli [--ast] [--json] [--warn=ERR_SEMANTIC]... \"/^ ta vedere nu io ^/\"";

fn main() -> ExitCode {
    let mut show_ast = false;
    let mut show_json = false;
    let mut source: Option<String> = None;
    let mut config = ParserConfig::default();
    for arg in std::env::args().skip(1) {
        if let Some(name) = arg.strip_prefix("--warn=") {
            let Some(code) = ErrorCode::from_name(name) else {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            };
            config.warnings = config.warnings.with(code);
        } else if arg == "--ast" && !show_ast {
            show_ast = true;
        } else if arg == "--json" && !show_json {
            show_json = true;
        } else if source.is_none() {
            source = Some(arg);
        } else {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    }
    let Some(source) = source else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    let mut session = Session::with_config(config);
    let ids = match session.ingest(&source) {
        Ok(ids) => ids,
        Err(diagnostics) => {
            for diagnostic in &diagnostics {
                eprintln!("{diagnostic}");
            }
            return ExitCode::FAILURE;
        }
    };

    for id in ids {
        let Some(period) = session.period(id) else {
            eprintln!("period {} not found in the session", id.index());
            return ExitCode::FAILURE;
        };
        match render_period(period, session.interner()) {
            Ok(text) => println!("OK  {text}"),
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::FAILURE;
            }
        }
        for warning in session.analysis(id).map_or(&[][..], |a| a.warnings()) {
            println!("WARN  {warning}");
        }
        if show_ast {
            println!("{period:#?}");
            if let Some(analysis) = session.analysis(id) {
                println!("{analysis:#?}");
            }
        }
        if show_json {
            match export_analyzed_json(&session, id) {
                Ok(json) => println!("{json}"),
                Err(err) => {
                    eprintln!("{err}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    ExitCode::SUCCESS
}
