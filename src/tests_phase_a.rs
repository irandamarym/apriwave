//! Phase A tests: D9 (forbidden characters in raw strings), deterministic JSON
//! export, Functions contract (D12).

use crate::analyze::{ClauseId, GlobalClause, SlotIndex, SlotRef};
use crate::ast::{ClauseRef, GlyphSlot};
use crate::error::{DiagnosticKind, ErrorCode, GlyphFault};
use crate::export::export_analyzed_json;
use crate::functions::{FunctionError, FunctionEvaluator};
use crate::printer::render_glyph_slot;
use crate::session::{PeriodId, Session};

type TestResult = Result<(), String>;

fn accept(session: &mut Session, src: &str) -> Result<Vec<PeriodId>, String> {
    session
        .ingest(src)
        .map_err(|d| format!("rejected {src:?}: {d:#?}"))
}

fn first(ids: &[PeriodId]) -> Result<PeriodId, String> {
    ids.first().copied().ok_or_else(|| "no period".to_owned())
}

fn expect_kind(src: &str, kind: DiagnosticKind, code: ErrorCode) -> TestResult {
    match Session::new().ingest(src) {
        Ok(_) => Err(format!("accepted but should have failed: {src:?}")),
        Err(d) if d.iter().any(|x| x.kind == kind && x.code() == code) => Ok(()),
        Err(d) => Err(format!("{src:?}: expected {kind:?}, got {d:#?}")),
    }
}

fn json_of(src: &str) -> Result<String, String> {
    let mut session = Session::new();
    let ids = accept(&mut session, src)?;
    let id = ids.last().copied().ok_or("no period")?;
    export_analyzed_json(&session, id).map_err(|e| e.to_string())
}

fn contains(haystack: &str, needle: &str) -> TestResult {
    if haystack.contains(needle) {
        Ok(())
    } else {
        Err(format!("missing {needle:?} in:\n{haystack}"))
    }
}

/// Minimal structural check: balanced brackets outside strings,
/// closed strings, no raw control characters.
fn well_formed(json: &str) -> TestResult {
    let mut depth: i64 = 0;
    let mut in_string = false;
    let mut escaped = false;
    for ch in json.chars() {
        if ch.is_control() {
            return Err(format!("raw control character U+{:04X}", u32::from(ch)));
        }
        if in_string {
            match (escaped, ch) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return Err("unbalanced brackets".to_owned());
        }
    }
    if depth == 0 && !in_string {
        Ok(())
    } else {
        Err("unclosed JSON".to_owned())
    }
}

// ═══════════════════════════ D9 ═══════════════════════════

#[test]
fn raw_strings_reject_control_and_bidi_characters_d9() -> TestResult {
    for ch in [
        '\u{1B}', '\u{0}', '\u{7F}', '\u{90}', '\u{202E}', '\u{2066}', '\u{200B}', '\u{FEFF}',
    ] {
        expect_kind(
            &format!("/^ ta log nu \"ab{ch}cd\" ^/"),
            DiagnosticKind::MalformedGlyph {
                fault: GlyphFault::ForbiddenRawCharacter { ch },
            },
            ErrorCode::Syntax,
        )?;
    }
    // Uppercase letters, punctuation, backslashes and printable Unicode remain allowed.
    let mut session = Session::new();
    accept(
        &mut session,
        "/^ ta log nu \"Xcvb124\" au \"a\\b.(c)|ñ漢\" ^/",
    )?;
    Ok(())
}

// ═══════════════════════════ Export JSON ═══════════════════════════

const EXAMPLE_5: &str = "/^ ta vedere nu io au humanum : tazato venire n ~ sed to vedere nu io au humanum : tozato venire n ~ ^/";

#[test]
fn export_example_5_resolutions() -> TestResult {
    let json = json_of(EXAMPLE_5)?;
    well_formed(&json)?;
    contains(&json, "\"roots\":[0,2]")?;
    contains(&json, "{\"id\":2,\"conj\":\"sed\",\"members\":[0,1]}")?;
    contains(
        &json,
        "\"resolution\":{\"kind\":\"glyph_anaphora\",\"antecedent\":{\"period\":0,\"clause\":0,\"position\":2}}",
    )?;
    contains(
        &json,
        "\"tile_kind\":\"case\",\"tile\":\"n\",\"written\":\"~\"",
    )?;
    contains(&json, "\"text\":\"humanum\"")?;
    contains(&json, "\"number\":\"singular\"")?;
    contains(&json, "\"tense\":\"zato\"")
}

#[test]
fn export_is_deterministic() -> TestResult {
    let a = json_of(EXAMPLE_5)?;
    let b = json_of(EXAMPLE_5)?;
    if a == b {
        Ok(())
    } else {
        Err("export not deterministic".to_owned())
    }
}

#[test]
fn export_definition_portability_and_escaping() -> TestResult {
    let json = json_of("/^ defu dev a \"a\\b\" y ta leggere nu dev : anchor p§1 : end ~ ^/")?;
    well_formed(&json)?;
    contains(&json, "\"kind\":\"definition\"")?;
    contains(&json, "\"op\":\"assign\",\"tile\":\"a\"")?;
    contains(&json, "\"kind\":\"raw\",\"text\":\"a\\\\b\"")?;
    contains(
        &json,
        "\"kind\":\"variable\",\"binding\":0,\"name\":\"dev\",\"number\":\"singular\"",
    )?;
    contains(&json, "\"keyword\":\"anchor\",\"operand\":\"p§1\"")?;
    contains(&json, "\"keyword\":\"end\",\"operand\":\"~\"")?;
    let json = json_of("/^ ta x nu a : tozato ~~ ^/")?;
    contains(
        &json,
        "\"value\":{\"kind\":\"sema\",\"clause\":{\"period\":0,\"clause\":0}}",
    )
}

#[test]
fn export_unknown_period_is_an_error() -> TestResult {
    let mut session = Session::new();
    accept(&mut session, "/^ ta x ^/")?;
    // An id issued by a longer session does not exist in `session`.
    let mut longer = Session::new();
    let ids = accept(&mut longer, "/^ ta x ^/ /^ ta y ^/")?;
    let missing = ids.last().copied().ok_or("no period")?;
    match export_analyzed_json(&session, missing) {
        Err(crate::export::ExportError::PeriodNotFound(_)) => Ok(()),
        other => Err(format!("expected PeriodNotFound, got {other:?}")),
    }
}

// ═══════════════════════════ Functions (D12) ═══════════════════════════

/// Test evaluator: sums the numeric glyphs of the clause's `a` / `absi` pairs.
struct Sum;

impl FunctionEvaluator for Sum {
    type Error = String;

    fn evaluate(&self, session: &Session, clause: GlobalClause) -> Result<String, String> {
        let period = session.period(clause.period).ok_or("period missing")?;
        let Some(ClauseRef::Relational(block)) = period.clause(clause.clause.index()) else {
            return Err("non-relational clause".to_owned());
        };
        let mut total: u64 = 0;
        for (index, pair) in block.cases.iter().enumerate() {
            let stem = pair.tile.node.case.stem();
            if stem != "a" && stem != "absi" {
                continue;
            }
            let position = index
                .checked_add(1)
                .and_then(|p| u32::try_from(p).ok())
                .ok_or("index out of range")?;
            let slot = session
                .glyph_slot(SlotRef {
                    clause,
                    slot: SlotIndex::new(position),
                })
                .ok_or("slot missing")?;
            let GlyphSlot::Glyph(_) = slot else {
                return Err("operand is not a glyph".to_owned());
            };
            let text = render_glyph_slot(slot, session.interner()).map_err(|e| e.to_string())?;
            let value: u64 = text
                .parse()
                .map_err(|_| format!("non-numeric operand: {text}"))?;
            total = total.checked_add(value).ok_or("overflow")?;
        }
        Ok(format!("/^ ta risultato nu sistema a {total} ^/"))
    }
}

struct Broken;

impl FunctionEvaluator for Broken {
    type Error = String;

    fn evaluate(&self, _: &Session, _: GlobalClause) -> Result<String, String> {
        Ok("/^ ta risultato nu ^/".to_owned())
    }
}

#[test]
fn function_output_is_reingested() -> TestResult {
    let mut session = Session::new();
    let ids = accept(
        &mut session,
        "/^ ka somma nu sistema a 10 absi 20 absi 12 ^/",
    )?;
    let clause = GlobalClause {
        period: first(&ids)?,
        clause: ClauseId::HEAD,
    };
    let produced = session
        .evaluate_and_ingest(&Sum, clause)
        .map_err(|e| format!("{e:?}"))?;
    let id = first(&produced)?;
    let period = session.period(id).ok_or("period missing")?;
    let text =
        crate::printer::render_period(period, session.interner()).map_err(|e| e.to_string())?;
    if text == "/^ ta risultato nu sistema a 42 ^/" {
        Ok(())
    } else {
        Err(format!("unexpected output: {text}"))
    }
}

#[test]
fn invalid_function_output_leaves_session_untouched() -> TestResult {
    let mut session = Session::new();
    let ids = accept(&mut session, "/^ ka somma nu sistema a 1 ^/")?;
    let before = session.periods().len();
    let clause = GlobalClause {
        period: first(&ids)?,
        clause: ClauseId::HEAD,
    };
    match session.evaluate_and_ingest(&Broken, clause) {
        Err(FunctionError::InvalidOutput(diagnostics)) if !diagnostics.is_empty() => {}
        other => return Err(format!("expected InvalidOutput, got {other:?}")),
    }
    if session.periods().len() == before {
        Ok(())
    } else {
        Err("the session was modified".to_owned())
    }
}
