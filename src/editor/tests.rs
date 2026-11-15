//! Phase D tests: assisted input, settings and shortcuts. No `.unwrap()`:
//! every test returns `Result` and uses `?`.

use std::fmt::Debug;

use super::{
    apply_label_choices, copy_subordinates, parse_settings, preprocess_assisted_input,
    preprocess_with_choices, suggestions, ActionShortcuts, AssistDiagnosticKind, AssistNote,
    AssistOutput, AssistSettings, Autocomplete, Chord, ChordError, ComparativeShortcuts,
    CopiedBlock, CopyError, CopyMode, EditorSettings, Key, KeyPress, SettingsErrorKind,
    ShortcutAction, ShortcutContext, ShortcutOutcome, ShortcutResolver, SlotChoice, Suggestion,
    SuggestionKind,
};
use crate::analyze::{ClauseId, GlobalClause, SlotIndex, SlotRef};
use crate::ast::Comparative;
use crate::dti::{Dti, DtiKey};
use crate::error::{ErrorCode, WarningCodes};
use crate::session::{Session, SlotKind};
use crate::span::Span;
use crate::stage3::MorphFlag;

type TestResult = Result<(), String>;

fn check<T: PartialEq + Debug>(what: &str, got: T, want: T) -> TestResult {
    if got == want {
        Ok(())
    } else {
        Err(format!("{what}: got {got:?}, want {want:?}"))
    }
}

fn canon_with(input: &str, settings: &AssistSettings, dti: Option<&Dti>) -> Result<String, String> {
    preprocess_assisted_input(input, settings, dti)
        .map(|out| out.text)
        .map_err(|d| format!("rejected {input:?}: {d:#?}"))
}

fn canon(input: &str) -> Result<String, String> {
    canon_with(input, &AssistSettings::default(), None)
}

/// Preprocesses with default settings and checks the canonical text.
fn expect_canon(input: &str, want: &str) -> TestResult {
    check(input, canon(input)?.as_str(), want)
}

/// Canonical text accepted by a fresh session.
fn expect_ingest(text: &str) -> TestResult {
    Session::new()
        .ingest(text)
        .map(|_| ())
        .map_err(|d| format!("session rejected {text:?}: {d:#?}"))
}

fn diagnostics_with(
    input: &str,
    settings: &AssistSettings,
    dti: Option<&Dti>,
) -> Result<Vec<AssistDiagnosticKind>, String> {
    match preprocess_assisted_input(input, settings, dti) {
        Ok(out) => Err(format!("accepted {input:?} as {:?}", out.text)),
        Err(diagnostics) => Ok(diagnostics.into_iter().map(|d| d.kind).collect()),
    }
}

fn expect_errors(input: &str, want: &[AssistDiagnosticKind]) -> TestResult {
    let got = diagnostics_with(input, &AssistSettings::default(), None)?;
    check(input, got.as_slice(), want)
}

// ═══════════════════════════ Whitespace and comments ═══════════════════════════

#[test]
fn whitespace_runs_become_single_spaces() -> TestResult {
    expect_canon("  /^\tta x\r\n\r\n    nu   y ^/  \n", "/^ ta x nu y ^/")?;
    expect_canon("/^ ta x\n\tnu y\u{3000}^/", "/^ ta x nu y ^/")?;
    expect_canon("", "")
}

#[test]
fn comments_start_at_a_token_boundary() -> TestResult {
    expect_canon(
        "/^ ta x // comment ^/ not a close\nnu y ^/",
        "/^ ta x nu y ^/",
    )?;
    // v4.1: `//` inside a token belongs to it (glyphs may contain any visible
    // character); a comment starts at the beginning of a token.
    expect_canon("/^ ta x nu y// kept\n^/ ^/", "/^ ta x nu y// kept ^/ ^/")?;
    expect_canon("/^ ta http://x nu y // note\n^/", "/^ ta http://x nu y ^/")?;
    expect_canon(
        "/^ ta \"http://x\" nu y // note\n^/",
        "/^ ta \"http://x\" nu y ^/",
    )?;
    // Period delimiters are never comments, even when followed by one.
    expect_canon("/^ ta x /^/ /^ ta y ^/", "/^ ta x /^/ /^ ta y ^/")?;
    expect_canon(
        "/^ ta x /^/// soft close\n/^ ta y ^^/// hard",
        "/^ ta x /^/ /^ ta y ^^/",
    )?;
    expect_canon("/^ ta x ^/// end", "/^ ta x ^/")?;
    // Comment on the last line without a newline.
    expect_canon("/^ ta x ^/ // done", "/^ ta x ^/")
}

#[test]
fn comments_can_be_disabled() -> TestResult {
    let settings = AssistSettings {
        comments: false,
        ..AssistSettings::default()
    };
    check(
        "comments off",
        canon_with("/^ ta x // y ^/", &settings, None)?.as_str(),
        "/^ ta x // y ^/",
    )
}

// ═══════════════════════════ Braces ═══════════════════════════

#[test]
fn braces_prefix_every_clause_start() -> TestResult {
    // Legacy bug: only the first clause after `{` received colons.
    let text = canon("/^ ta c { tazato a y tazato b } ^/")?;
    check(
        "conjunction",
        text.as_str(),
        "/^ ta c : tazato a y : tazato b ^/",
    )?;
    expect_ingest(&text)?;

    let text = canon("/^ ta c { tazato a tazato b tazato e } ^/")?;
    check(
        "siblings without conjunction",
        text.as_str(),
        "/^ ta c : tazato a : tazato b : tazato e ^/",
    )?;
    expect_ingest(&text)?;

    let text = canon("/^ ta c { tazato a { tazato b vel tazato f } tazato d } ^/")?;
    check(
        "nested",
        text.as_str(),
        "/^ ta c : tazato a :: tazato b vel :: tazato f : tazato d ^/",
    )?;
    expect_ingest(&text)
}

#[test]
fn braces_after_explicit_colons_open_children_of_that_clause() -> TestResult {
    let text = canon("/^ ta c : tazato a { tazato b } ^/")?;
    check(
        "base depth",
        text.as_str(),
        "/^ ta c : tazato a :: tazato b ^/",
    )?;
    expect_ingest(&text)?;
    // After `}` at depth 0, explicit connectors are allowed again.
    let text = canon("/^ ta c { tazato a } : tazato b y ta d ^/")?;
    check(
        "connectors after braces",
        text.as_str(),
        "/^ ta c : tazato a : tazato b y ta d ^/",
    )?;
    expect_ingest(&text)
}

#[test]
fn braces_with_case_pairs_and_def_blocks() -> TestResult {
    let text = canon("/^ ta vedere nu io au canis { ta abbaiare nu ~ { tazato fame nu ~ } } ^/")?;
    check(
        "case pairs",
        text.as_str(),
        "/^ ta vedere nu io au canis : ta abbaiare nu ~ :: tazato fame nu ~ ^/",
    )?;
    expect_ingest(&text)
}

#[test]
fn brace_errors() -> TestResult {
    use AssistDiagnosticKind as K;
    expect_errors("/^ { ta c } ^/", &[K::BraceAfterPeriodOpen])?;
    expect_errors("/^ ta c { tazato a ^/", &[K::UnclosedBrace])?;
    expect_errors("/^ ta c { tazato a", &[K::UnclosedBrace])?;
    expect_errors("/^ ta c } ^/", &[K::UnexpectedCloseBrace])?;
    expect_errors("/^ ta c { } ^/", &[K::EmptyBraces])?;
    // v4.1 Slot Locking: a brace in an even slot is a glyph.
    expect_errors("/^ ta { tazato a } ^/", &[K::UnexpectedCloseBrace])?;
    expect_errors("/^ ta c { tazato } ^/", &[K::UnclosedBrace])?;
    expect_errors("/^ ta c { tazato a } { tazato b } ^/", &[K::MisplacedBrace])?;
    expect_errors(
        "/^ ta c { tazato a : tazato b } ^/",
        &[K::MixedBracesAndColons],
    )?;
    expect_errors(
        "/^ ta c { tazato a y : tazato b } ^/",
        &[K::MixedBracesAndColons],
    )?;
    expect_errors("/^ ta c { anchor p } ^/", &[K::BracesWithPortability])?;
    expect_errors("/^ ta c { tazato a end ~ } ^/", &[K::BracesWithPortability])?;
    expect_errors(
        "/^ ta c : anchor p { tazato a } ^/",
        &[K::BracesWithPortability],
    )?;
    expect_errors("/^ ta c { tazato a } ta d ^/", &[K::MissingConnector])?;
    expect_errors("/^ ta c { tazato a } nu d ^/", &[K::CaseTileAfterBrace])?;
    // Several errors are all collected.
    expect_errors(
        "/^ ta c } { ^/ /^ ta d { tazato x ^/",
        &[K::UnexpectedCloseBrace, K::UnclosedBrace, K::UnclosedBrace],
    )
}

#[test]
fn brace_diagnostics_carry_input_spans() -> TestResult {
    let input = "/^ ta c\n  { tazato a ^/";
    let diagnostics = preprocess_assisted_input(input, &AssistSettings::default(), None)
        .err()
        .ok_or("accepted")?;
    let first = diagnostics.first().ok_or("no diagnostic")?;
    check("span start", first.span.start, 10)?;
    check("span end", first.span.end, 11)
}

#[test]
fn braces_can_be_disabled() -> TestResult {
    let settings = AssistSettings {
        braces: false,
        ..AssistSettings::default()
    };
    check(
        "braces off",
        canon_with("/^ ta c { tazato a } ^/", &settings, None)?.as_str(),
        "/^ ta c { tazato a } ^/",
    )
}

// ═══════════════════════════ Signal trains and aliases ═══════════════════════════

#[test]
fn underscore_trains_are_joined_and_reordered() -> TestResult {
    for (input, want) in [
        ("/^ ta_cha_zato x nu y a z ^/", "/^ tachazato x nu y a z ^/"),
        ("/^ ta_zato_cha x nu y a z ^/", "/^ tachazato x nu y a z ^/"),
        (
            "/^ ta_ato_cha_all x nu y a z ^/",
            "/^ taallchaato x nu y a z ^/",
        ),
        ("/^ ta_ato_ref_all x nu y ^/", "/^ taallrefato x nu y ^/"),
        ("/^ tazato_all x nu y ^/", "/^ taallzato x nu y ^/"),
        ("/^ deep_all x nu y ^/", "/^ deepall x nu y ^/"),
        ("/^ ta x : qua_ta_zato y ^/", "/^ ta x : quatazato y ^/"),
        ("/^ ta x : quata_zato y ^/", "/^ ta x : quatazato y ^/"),
        ("/^ def_u x au y ^/", "/^ defu x au y ^/"),
        ("/^ ta_ref observe nu io ^/", "/^ taref observe nu io ^/"),
        // Rule 1B: the number of the signal pair goes last.
        ("/^ ta_u_zato x nu y ^/", "/^ tazatou x nu y ^/"),
        ("/^ deep_num_all x a y ^/", "/^ deepallnum x a y ^/"),
    ] {
        let text = canon(input)?;
        check(input, text.as_str(), want)?;
        expect_ingest(&text)?;
    }
    Ok(())
}

#[test]
fn underscore_trains_keep_duplicates_for_the_parser() -> TestResult {
    let text = canon("/^ ta_zato_cha_zato x ^/")?;
    check("duplicate kept", text.as_str(), "/^ tachazatozato x ^/")?;
    let diagnostics = Session::new()
        .ingest(&text)
        .err()
        .ok_or("duplicate accepted")?;
    check(
        "ERR_ORDER",
        diagnostics.iter().any(|d| d.code() == ErrorCode::Order),
        true,
    )
}

#[test]
fn underscores_outside_signal_slots_are_left_alone() -> TestResult {
    // Even slot: `_` stays (the parser reports it).
    expect_canon("/^ ta x_y nu z ^/", "/^ ta x_y nu z ^/")?;
    // Malformed trains are left unchanged.
    expect_canon("/^ ta__zato x ^/", "/^ ta__zato x ^/")?;
    expect_canon("/^ ta_foo x ^/", "/^ ta_foo x ^/")?;
    expect_canon("/^ qua_zato x ^/", "/^ qua_zato x ^/")?;
    // Case slots are never joined.
    expect_canon("/^ ta x n_u y ^/", "/^ ta x n_u y ^/")?;
    let settings = AssistSettings {
        signal_underscore: false,
        ..AssistSettings::default()
    };
    check(
        "trains off",
        canon_with("/^ ta_zato x ^/", &settings, None)?.as_str(),
        "/^ ta_zato x ^/",
    )
}

#[test]
fn symbol_aliases_inside_trains() -> TestResult {
    for (input, want) in [
        ("/^ ta_> x nu y a z ^/", "/^ tacha x nu y a z ^/"),
        ("/^ ta_< x nu y a z ^/", "/^ tachu x nu y a z ^/"),
        ("/^ ta_= x nu y a z ^/", "/^ tacol x nu y a z ^/"),
        ("/^ ta_!= x nu y a z ^/", "/^ tacla x nu y a z ^/"),
        ("/^ ta_zato_> x nu y a z ^/", "/^ tachazato x nu y a z ^/"),
    ] {
        let text = canon(input)?;
        check(input, text.as_str(), want)?;
        expect_ingest(&text)?;
    }
    // Aliases are never rewritten outside trains.
    expect_canon("/^ ta > ^/", "/^ ta > ^/")?;
    let settings = AssistSettings {
        symbol_aliases: false,
        ..AssistSettings::default()
    };
    check(
        "aliases off",
        canon_with("/^ ta_> x ^/", &settings, None)?.as_str(),
        "/^ ta_> x ^/",
    )
}

#[test]
fn removed_aliases_are_diagnosed() -> TestResult {
    for alias in ["~=", "+", "-"] {
        let input = format!("/^ ta_{alias} x nu y a z ^/");
        let diagnostics = preprocess_assisted_input(&input, &AssistSettings::default(), None)
            .err()
            .ok_or_else(|| format!("{alias} accepted"))?;
        let first = diagnostics.first().ok_or("no diagnostic")?;
        check(
            alias,
            &first.kind,
            &AssistDiagnosticKind::RemovedAlias {
                alias: alias.to_owned(),
            },
        )?;
        check("alias span start", first.span.start, 6)?;
        check(
            "alias span end",
            first.span.end,
            6 + u32::try_from(alias.len()).map_err(|e| e.to_string())?,
        )?;
    }
    Ok(())
}

#[test]
fn trains_and_braces_combine() -> TestResult {
    let text = canon("/^ ta c { ta_zato a ta_> b nu d a e } ^/")?;
    check(
        "train as sibling start",
        text.as_str(),
        "/^ ta c : tazato a : tacha b nu d a e ^/",
    )?;
    expect_ingest(&text)
}

// ═══════════════════════════ Full example ═══════════════════════════

#[test]
fn multi_line_commented_input_is_accepted_by_the_session() -> TestResult {
    let input = "\
// Assisted input: comments, braces, trains, aliases and a label.\r\n\
/^ ta_zato observe nu io {             // root clause\r\n\
\tta_zato_> grandis n canis a \"http://x\"  // reordered to tachazato\r\n\
\ty ta_zato venire n humanum {\r\n\
\t\tta_zato abbaiare nu ~             // grandchild\r\n\
\t}\r\n\
}\r\n\
y ta_ref inizio nu sistema\r\n\
^/   // end\r\n";
    let text = canon(input)?;
    check(
        "canonical",
        text.as_str(),
        "/^ tazato observe nu io : tachazato grandis n canis a \"http://x\" \
y : tazato venire n humanum :: tazato abbaiare nu ~ y taref inizio nu sistema ^/",
    )?;
    expect_ingest(&text)
}

// ═══════════════════════════ DTI: multiword and unknown glyphs ═══════════════════════════

fn key(tag: &str) -> Result<DtiKey, String> {
    let mut body = [b'0'; 31];
    for (slot, byte) in body.iter_mut().zip(tag.bytes()) {
        *slot = byte;
    }
    DtiKey::from_random_body(body).map_err(|e| e.to_string())
}

/// Editor DTI: every lexical form is an atom (no `§`); `display:` may contain
/// spaces. `greenemerald` and `house` carry `+UNION`; `offgrid` is not enabled;
/// `portal` (synonym) and `front door` (display) are shared by two concepts.
fn fixture() -> Result<Dti, String> {
    let (green, emerald, light, door, gate, offgrid, house, dog) = (
        key("green")?,
        key("greenemerald")?,
        key("greenemeraldlight")?,
        key("door")?,
        key("gate")?,
        key("offgrid")?,
        key("house")?,
        key("dog")?,
    );
    let universal = format!(
        "#awdti 1\n#role universal\n#version 1\n#lang en\n\
rec {green} green\n\
rec {emerald} greenemerald display:\"emerald green\" morph:+UNION\n\
rec {light} greenemeraldlight display:\"light emerald green\"\n\
rec {door} door display:\"front door\" syn:portal\n\
rec {gate} gate display:\"front door\" syn:portal\n\
rec {offgrid} offgrid display:\"off grid\" syn:unplugged\n\
rec {house} house syn:home morph:+UNION\n\
rec {dog} dog display:\"doggy\"\n"
    );
    let local = format!(
        "#awdti 1\n#role local\n#universal 1\n#lang it\n\
use {green} form:verde\n\
use {emerald} form:verdesmeraldo display:\"verde smeraldo\"\n\
use {light} form:verdesmeraldochiaro display:\"verde smeraldo chiaro\"\n\
use {door}\n\
use {gate}\n\
use {house} form:casa syn:dimora\n\
use {dog}\n"
    );
    Dti::load(&universal, &local).map_err(|d| format!("{d:#?}"))
}

fn assist(input: &str, dti: &Dti) -> Result<AssistOutput, String> {
    preprocess_assisted_input(input, &AssistSettings::default(), Some(dti))
        .map_err(|d| format!("rejected {input:?}: {d:#?}"))
}

fn normalized(written: &str, primary: &str, start: u32, end: u32) -> AssistNote {
    AssistNote::NormalizedForm {
        written: written.to_owned(),
        primary: primary.to_owned(),
        span: Span::new(start, end),
    }
}

/// `NormalizedForm` notes only, in input order.
fn normalized_notes(out: &AssistOutput) -> Vec<&AssistNote> {
    out.notes
        .iter()
        .filter(|n| matches!(n, AssistNote::NormalizedForm { .. }))
        .collect()
}

#[test]
fn multiword_display_forms_use_longest_match() -> TestResult {
    let dti = fixture()?;
    let settings = AssistSettings::default();
    let out = preprocess_assisted_input(
        "/^ ta verde smeraldo nu verde smeraldo chiaro a emerald green ^/",
        &settings,
        Some(&dti),
    )
    .map_err(|d| format!("{d:#?}"))?;
    check(
        "text",
        out.text.as_str(),
        "/^ ta verdesmeraldo nu verdesmeraldochiaro a verdesmeraldo ^/",
    )?;
    check("notes", out.notes.len(), 3)?;
    expect_ingest(&out.text)?;
    let first = out.notes.first().ok_or("no note")?;
    check(
        "multiword note",
        first,
        &AssistNote::MultiwordGlyph {
            phrase: "verde smeraldo".to_owned(),
            form: "verdesmeraldo".to_owned(),
            span: Span::new(6, 20),
        },
    )
}

#[test]
fn multiword_never_crosses_tiles_and_never_rewrites_single_tokens() -> TestResult {
    let dti = fixture()?;
    let settings = AssistSettings::default();
    // `nu` is a case tile: `verde nu smeraldo` is not a phrase.
    check(
        "case tile blocks",
        canon_with("/^ ta verde nu smeraldo ^/", &settings, Some(&dti))?.as_str(),
        "/^ ta verde nu smeraldo ^/",
    )?;
    // Conjunctions, colon runs and braces block too.
    check(
        "conjunction blocks",
        canon_with("/^ ta verde y ta smeraldo ^/", &settings, Some(&dti))?.as_str(),
        "/^ ta verde y ta smeraldo ^/",
    )?;
    check(
        "brace blocks",
        canon_with("/^ ta c { tazato verde } ^/", &settings, Some(&dti))?.as_str(),
        "/^ ta c : tazato verde ^/",
    )?;
    // Multiword only in even slots and only when enabled.
    let off = AssistSettings {
        multiword: false,
        ..AssistSettings::default()
    };
    check(
        "multiword off",
        canon_with("/^ ta verde smeraldo ^/", &off, Some(&dti))?.as_str(),
        "/^ ta verde smeraldo ^/",
    )?;
    check(
        "no dti",
        canon_with("/^ ta verde smeraldo ^/", &settings, None)?.as_str(),
        "/^ ta verde smeraldo ^/",
    )
}

#[test]
fn ambiguous_and_disabled_multiword_entries() -> TestResult {
    let dti = fixture()?;
    let settings = AssistSettings::default();
    let mut both = vec![key("door")?, key("gate")?];
    both.sort();
    check(
        "ambiguous",
        diagnostics_with("/^ ta front door ^/", &settings, Some(&dti))?,
        vec![AssistDiagnosticKind::AmbiguousForm {
            phrase: "front door".to_owned(),
            keys: both,
        }],
    )?;
    let offgrid = key("offgrid")?;
    check(
        "disabled",
        diagnostics_with("/^ ta off grid ^/", &settings, Some(&dti))?,
        vec![AssistDiagnosticKind::DisabledEntry {
            key: offgrid,
            suggestion: format!("use {offgrid}"),
        }],
    )
}

#[test]
fn unknown_glyphs_produce_notes() -> TestResult {
    let dti = fixture()?;
    let out = preprocess_assisted_input(
        "/^ ta blu nu verde a 42 dad \"x\" gpa verde~ ^/ /^ defu dev a verde ^/",
        &AssistSettings::default(),
        Some(&dti),
    )
    .map_err(|d| format!("{d:#?}"))?;
    // v4.1: `"x"` and `verde~` are ordinary glyphs, unknown to the DTI.
    check(
        "notes",
        out.notes,
        vec![
            AssistNote::UnknownGlyph {
                token: "blu".to_owned(),
                span: Span::new(6, 9),
                structural: false,
            },
            AssistNote::UnknownGlyph {
                token: "\"x\"".to_owned(),
                span: Span::new(28, 31),
                structural: false,
            },
            AssistNote::UnknownGlyph {
                token: "verde~".to_owned(),
                span: Span::new(36, 42),
                structural: false,
            },
        ],
    )
}

#[test]
fn morphology_violations_are_reported_b9() -> TestResult {
    let dti = fixture()?;
    let settings = AssistSettings::default();
    // `verde§verde` is a union: `verde` would need +UNION.
    check(
        "union",
        diagnostics_with(
            "/^ ta verdesmeraldo nu verde§verde ^/",
            &settings,
            Some(&dti),
        )?,
        vec![AssistDiagnosticKind::MorphViolation {
            form: "verde§verde".to_owned(),
            key: key("green")?,
            flag: MorphFlag::Union,
        }],
    )?;
    // Whole records need no flag.
    let out = preprocess_assisted_input("/^ ta verdesmeraldochiaro ^/", &settings, Some(&dti))
        .map_err(|d| format!("{d:#?}"))?;
    check("record", out.text.as_str(), "/^ ta verdesmeraldochiaro ^/")?;
    // Concepts carrying +UNION join freely; the check runs on primary forms.
    check(
        "union allowed",
        assist("/^ ta home§verdesmeraldo ^/", &dti)?.text.as_str(),
        "/^ ta casa§verdesmeraldo ^/",
    )?;
    check(
        "violation on the normalized text",
        diagnostics_with("/^ ta dimora§verde ^/", &settings, Some(&dti))?,
        vec![AssistDiagnosticKind::MorphViolation {
            form: "casa§verde".to_owned(),
            key: key("green")?,
            flag: MorphFlag::Union,
        }],
    )
}

// ═══════════════════════════ DTI: primary-form normalization (decision E) ═══════════════════════════

#[test]
fn synonyms_and_displays_are_normalized_in_relational_slots() -> TestResult {
    let dti = fixture()?;
    // Signal glyph slot (exchange synonym → local primary), case slot (local
    // synonym), case slot (single-token display → exchange primary).
    let out = assist("/^ ta home nu dimora a doggy ^/", &dti)?;
    check("text", out.text.as_str(), "/^ ta casa nu casa a dog ^/")?;
    check(
        "notes",
        out.notes,
        vec![
            normalized("home", "casa", 6, 10),
            normalized("dimora", "casa", 14, 20),
            normalized("doggy", "dog", 23, 28),
        ],
    )?;
    expect_ingest("/^ ta casa nu casa a dog ^/")?;
    // Primary forms of both languages are unchanged.
    let out = assist("/^ ta house nu casa a dog ^/", &dti)?;
    check(
        "primaries",
        out.text.as_str(),
        "/^ ta house nu casa a dog ^/",
    )?;
    check("no notes", out.notes, Vec::new())
}

#[test]
fn synonym_def_name_and_its_references_are_normalized_consistently() -> TestResult {
    let dti = fixture()?;
    let out = assist("/^ defu home au verde y ta leggere nu home ^/", &dti)?;
    check(
        "text",
        out.text.as_str(),
        "/^ defu casa au verde y ta leggere nu casa ^/",
    )?;
    check(
        "notes",
        normalized_notes(&out),
        vec![
            &normalized("home", "casa", 8, 12),
            &normalized("home", "casa", 38, 42),
        ],
    )?;
    expect_ingest(&out.text)
}

#[test]
fn synonyms_and_multiword_displays_are_normalized_in_def_values() -> TestResult {
    let dti = fixture()?;
    let out = assist("/^ defu dev au dimora ^/", &dti)?;
    check("synonym", out.text.as_str(), "/^ defu dev au casa ^/")?;
    check(
        "synonym note",
        out.notes,
        vec![normalized("dimora", "casa", 15, 21)],
    )?;
    expect_ingest("/^ defu dev au casa ^/")?;
    let out = assist("/^ defu dev au verde smeraldo ^/", &dti)?;
    check(
        "multiword",
        out.text.as_str(),
        "/^ defu dev au verdesmeraldo ^/",
    )?;
    check(
        "multiword note",
        out.notes,
        vec![AssistNote::MultiwordGlyph {
            phrase: "verde smeraldo".to_owned(),
            form: "verdesmeraldo".to_owned(),
            span: Span::new(15, 29),
        }],
    )
}

#[test]
fn synonyms_are_normalized_in_portability_ids() -> TestResult {
    let dti = fixture()?;
    let out = assist("/^ ta x : anchor home§1 : rel dimora§1 : end ~ ^/", &dti)?;
    check(
        "text",
        out.text.as_str(),
        "/^ ta x : anchor casa§1 : rel casa§1 : end ~ ^/",
    )?;
    check(
        "notes",
        normalized_notes(&out),
        vec![
            &normalized("home", "casa", 17, 21),
            &normalized("dimora", "casa", 31, 37),
        ],
    )?;
    expect_ingest(&out.text)
}

#[test]
fn suffixed_tokens_are_ordinary_glyphs() -> TestResult {
    let dti = fixture()?;
    // v4.1: `home~` and `doggy~~` are glyph tokens (no anaphora bases): they are
    // not forms of the DTI and stay unchanged.
    let out = assist("/^ ta home~ nu dimora a doggy~~ ^/", &dti)?;
    check(
        "text",
        out.text.as_str(),
        "/^ ta home~ nu casa a doggy~~ ^/",
    )?;
    check(
        "notes",
        out.notes,
        vec![
            AssistNote::UnknownGlyph {
                token: "home~".to_owned(),
                span: Span::new(6, 11),
                structural: false,
            },
            normalized("dimora", "casa", 15, 21),
            AssistNote::UnknownGlyph {
                token: "doggy~~".to_owned(),
                span: Span::new(24, 31),
                structural: false,
            },
        ],
    )?;
    // The bare marks are never touched.
    let out = assist("/^ ta x nu ~ a ~~ ^/", &dti)?;
    check("marks", out.text.as_str(), "/^ ta x nu ~ a ~~ ^/")
}

#[test]
fn every_segment_of_a_qualified_path_is_normalized() -> TestResult {
    let dti = fixture()?;
    // `§` is two bytes long in UTF-8.
    let out = assist("/^ ta home§verdesmeraldo§dimora ^/", &dti)?;
    check(
        "text",
        out.text.as_str(),
        "/^ ta casa§verdesmeraldo§casa ^/",
    )?;
    check(
        "notes",
        out.notes,
        vec![
            normalized("home", "casa", 6, 10),
            normalized("dimora", "casa", 27, 33),
        ],
    )?;
    expect_ingest("/^ ta casa§verdesmeraldo§casa ^/")?;
    // A free atom inside a path stays and the glyph is reported as unknown.
    let out = assist("/^ ta home§blu ^/", &dti)?;
    check("free segment", out.text.as_str(), "/^ ta casa§blu ^/")?;
    check(
        "free segment notes",
        out.notes,
        vec![
            normalized("home", "casa", 6, 10),
            AssistNote::UnknownGlyph {
                token: "home§blu".to_owned(),
                span: Span::new(6, 15),
                structural: false,
            },
        ],
    )
}

#[test]
fn ambiguous_synonyms_are_rejected_in_every_position() -> TestResult {
    let dti = fixture()?;
    let settings = AssistSettings::default();
    let ambiguous = || -> Result<AssistDiagnosticKind, String> {
        let mut keys = vec![key("door")?, key("gate")?];
        keys.sort();
        Ok(AssistDiagnosticKind::AmbiguousForm {
            phrase: "portal".to_owned(),
            keys,
        })
    };
    for input in [
        "/^ ta portal ^/",
        "/^ ta x nu verde§portal ^/",
        "/^ defu portal au verde ^/",
        "/^ ta x : anchor portal ^/",
    ] {
        check(
            input,
            diagnostics_with(input, &settings, Some(&dti))?,
            vec![ambiguous()?],
        )?;
    }
    // The diagnostic carries the span of the atom.
    let diagnostics =
        preprocess_assisted_input("/^ ta x nu verde§portal ^/", &settings, Some(&dti))
            .err()
            .ok_or("accepted")?;
    let first = diagnostics.first().ok_or("no diagnostic")?;
    check("ambiguous span", first.span, Span::new(18, 24))
}

#[test]
fn synonyms_of_disabled_concepts_are_rejected() -> TestResult {
    let dti = fixture()?;
    let settings = AssistSettings::default();
    let offgrid = key("offgrid")?;
    for input in [
        "/^ ta unplugged ^/",
        "/^ defu unplugged au verde ^/",
        "/^ ta x : anchor unplugged ^/",
    ] {
        check(
            input,
            diagnostics_with(input, &settings, Some(&dti))?,
            vec![AssistDiagnosticKind::DisabledEntry {
                key: offgrid,
                suggestion: format!("use {offgrid}"),
            }],
        )?;
    }
    Ok(())
}

#[test]
fn primary_forms_of_disabled_concepts_are_legitimate_session_symbols() -> TestResult {
    let dti = fixture()?;
    let out = assist("/^ defu offgrid au verde ^/", &dti)?;
    check("def name", out.text.as_str(), "/^ defu offgrid au verde ^/")?;
    check("def name notes", out.notes, Vec::new())?;
    expect_ingest(&out.text)?;
    let out = assist("/^ ta x : anchor offgrid : end ~ ^/", &dti)?;
    check(
        "anchor id",
        out.text.as_str(),
        "/^ ta x : anchor offgrid : end ~ ^/",
    )?;
    check(
        "anchor id notes",
        normalized_notes(&out),
        Vec::<&AssistNote>::new(),
    )
}

#[test]
fn free_identifiers_are_unchanged() -> TestResult {
    let dti = fixture()?;
    let input = "/^ defu dev au blu y ta leggere nu dev : anchor p : end ~ ^/";
    let out = assist(input, &dti)?;
    check("text", out.text.as_str(), input)?;
    check(
        "no normalization",
        normalized_notes(&out),
        Vec::<&AssistNote>::new(),
    )?;
    check(
        "unknown glyph note",
        out.notes.first(),
        Some(&AssistNote::UnknownGlyph {
            token: "blu".to_owned(),
            span: Span::new(15, 18),
            structural: false,
        }),
    )?;
    // Numerals are known; v4.1: `"home"` is an ordinary (unknown) glyph.
    let out = assist("/^ ta \"home\" nu 42 ^/", &dti)?;
    check("quoted", out.text.as_str(), "/^ ta \"home\" nu 42 ^/")?;
    check(
        "quoted notes",
        out.notes,
        vec![AssistNote::UnknownGlyph {
            token: "\"home\"".to_owned(),
            span: Span::new(6, 12),
            structural: false,
        }],
    )?;
    // Without a DTI nothing is normalized.
    expect_canon(
        "/^ ta home nu dimora§doggy ^/",
        "/^ ta home nu dimora§doggy ^/",
    )
}

/// Editor fixture plus local labels: `Roma` (label only), `casa` (also a
/// glyph) and `dimora` (also a synonym of `casa`).
fn fixture_with_labels() -> Result<Dti, String> {
    let (green, house) = (key("green")?, key("house")?);
    let universal = format!(
        "#awdti 1\n#role universal\n#version 1\n#lang en\n\
rec {green} green\n\
rec {house} house syn:home morph:+UNION\n"
    );
    let local = format!(
        "#awdti 1\n#role local\n#universal 1\n#lang it\n\
use {green} form:verde\n\
use {house} form:casa syn:dimora\n\
label {} Roma\n\
label {} casa\n\
label {} dimora\n",
        key("labelroma")?,
        key("labelcasa")?,
        key("labeldimora")?
    );
    Dti::load(&universal, &local).map_err(|d| format!("{d:#?}"))
}

#[test]
fn unmarked_labels_ask_for_the_mandatory_choice_v4_1() -> TestResult {
    let dti = fixture_with_labels()?;
    // Without a selection every slot is a glyph: `dimora` (a label, and a
    // synonym of `casa`) is normalized; the notes ask for the selection.
    let out = assist("/^ ta Roma nu casa a dimora ^/", &dti)?;
    check("text", out.text.as_str(), "/^ ta Roma nu casa a casa ^/")?;
    check("no label spans", out.labels, Vec::new())?;
    check(
        "notes",
        out.notes,
        vec![
            AssistNote::Label {
                token: "Roma".to_owned(),
                span: Span::new(6, 10),
            },
            AssistNote::GlyphOrLabel {
                token: "casa".to_owned(),
                span: Span::new(14, 18),
            },
            normalized("dimora", "casa", 21, 27),
            AssistNote::GlyphOrLabel {
                token: "dimora".to_owned(),
                span: Span::new(21, 27),
            },
        ],
    )
}

#[test]
fn selected_labels_are_inert_and_travel_as_labels_v4_1() -> TestResult {
    let dti = fixture_with_labels()?;
    let input = "/^ ta  Roma nu casa a dimora ^/";
    let choices = [
        SlotChoice {
            span: Span::new(7, 11),
            kind: SlotKind::Label,
        },
        SlotChoice {
            span: Span::new(15, 19),
            kind: SlotKind::Glyph,
        },
        SlotChoice {
            span: Span::new(22, 28),
            kind: SlotKind::Label,
        },
    ];
    let out = preprocess_with_choices(input, &AssistSettings::default(), Some(&dti), &choices)
        .map_err(|d| format!("{d:#?}"))?;
    // A label is an inert string: `dimora` is not normalized.
    check("text", out.text.as_str(), "/^ ta Roma nu casa a dimora ^/")?;
    check("notes", out.notes, Vec::new())?;
    check(
        "label spans in the output",
        out.labels.as_slice(),
        &[Span::new(6, 10), Span::new(21, 27)][..],
    )?;
    let mut session = Session::new();
    let ids = session.ingest(&out.text).map_err(|d| format!("{d:#?}"))?;
    check(
        "marked slots",
        apply_label_choices(&mut session, &ids, &out.labels),
        2,
    )?;
    let id = ids.first().copied().ok_or("no period")?;
    check(
        "indexed",
        crate::stage3::render_indexed_period(&session, id, &dti).map_err(|e| e.to_string())?,
        format!("/^ ta LVAAAARoma nu {} a LVAAAAdimora ^/", key("house")?),
    )?;
    // A label choice on a form that is not censused as a label.
    let out = preprocess_with_choices(
        "/^ ta Milano ^/",
        &AssistSettings::default(),
        Some(&dti),
        &[SlotChoice {
            span: Span::new(6, 12),
            kind: SlotKind::Label,
        }],
    )
    .map_err(|d| format!("{d:#?}"))?;
    check(
        "uncensused label",
        out.notes,
        vec![AssistNote::UncensusedLabel {
            token: "Milano".to_owned(),
            span: Span::new(6, 12),
        }],
    )
}

#[test]
fn delimiters_in_even_slots_are_checked_against_the_dti_v4_1() -> TestResult {
    let dti = fixture()?;
    let out = assist("/^ ta verde nu ^/", &dti)?;
    check("text", out.text.as_str(), "/^ ta verde nu ^/")?;
    check(
        "not censused, structural",
        out.notes,
        vec![AssistNote::UnknownGlyph {
            token: "^/".to_owned(),
            span: Span::new(15, 17),
            structural: true,
        }],
    )?;
    // The session reports the missing period close as well.
    match Session::new().ingest(&out.text) {
        Err(diagnostics)
            if diagnostics
                .iter()
                .any(|d| d.kind == crate::DiagnosticKind::MissingPeriodClose) =>
        {
            Ok(())
        }
        other => Err(format!("expected MissingPeriodClose, got {other:?}")),
    }
}

#[test]
fn even_slots_follow_slot_locking_v4_1() -> TestResult {
    // Period delimiters, colon runs, braces and quotes in an even slot are glyphs.
    for text in [
        "/^ ta ^/ nu : a { ^/",
        "/^ ta /^/ nu \" a } ^/",
        "/^ ta x nu ~ a ~~ ^/",
    ] {
        expect_canon(text, text)?;
    }
    expect_ingest("/^ ta ^/ nu : a { ^/")
}

// ═══════════════════════════ Settings ═══════════════════════════

fn parse(src: &str) -> Result<EditorSettings, String> {
    parse_settings(src).map_err(|e| format!("{e:#?}"))
}

fn expect_settings_error(src: &str, line: usize, kind: SettingsErrorKind) -> TestResult {
    match parse_settings(src) {
        Ok(_) => Err(format!("accepted {src:?}, expected {kind:?}")),
        Err(errors) => {
            if errors.iter().any(|e| e.line == line && e.kind == kind) {
                Ok(())
            } else {
                Err(format!("expected {kind:?} at line {line}, got {errors:#?}"))
            }
        }
    }
}

#[test]
fn settings_defaults() -> TestResult {
    let defaults = EditorSettings::default();
    check("empty file", parse("")?, EditorSettings::default())?;
    check(
        "comments only",
        parse("# nothing\n\n")?,
        EditorSettings::default(),
    )?;
    check("assist", defaults.assist, AssistSettings::default())?;
    check("prefix", defaults.comparative.prefix, Chord::alt('c'))?;
    check(
        "mapping",
        defaults.comparative.bindings.as_slice(),
        &[
            ('>', Comparative::Cha),
            ('<', Comparative::Chu),
            ('=', Comparative::Col),
            ('!', Comparative::Cla),
            ('~', Comparative::Cal),
            ('[', Comparative::Cua),
            ('-', Comparative::Cuo),
            ('+', Comparative::Cup),
        ][..],
    )?;
    check("custom", defaults.custom.is_empty(), true)
}

const FULL_SETTINGS: &str = r#"
# Editor settings
[editor]
braces = false          # no braces
multiword = false

[ shortcuts . comparative ]
prefix = "ctrl+alt+k"
">" = "cha"
"\"" = "cup"
"\\" = "cuo"
c = "col"

[custom_shortcuts]
"Alt+V" = "@live_variables"
"F5" = "/^ "
"shift+ctrl+x" = "say \"hi\" # not a comment"
"#;

#[test]
fn settings_full_file() -> TestResult {
    let settings = parse(FULL_SETTINGS)?;
    check(
        "assist",
        settings.assist,
        AssistSettings {
            braces: false,
            multiword: false,
            ..AssistSettings::default()
        },
    )?;
    check(
        "comparative",
        &settings.comparative,
        &ComparativeShortcuts {
            prefix: Chord::parse("Ctrl+Alt+K").map_err(|e| e.to_string())?,
            bindings: vec![
                ('>', Comparative::Cha),
                ('"', Comparative::Cup),
                ('\\', Comparative::Cuo),
                ('c', Comparative::Col),
            ],
        },
    )?;
    check(
        "custom",
        settings.custom.as_slice(),
        &[
            (Chord::alt('v'), ShortcutAction::LiveVariables),
            (
                Chord::parse("F5").map_err(|e| e.to_string())?,
                ShortcutAction::Insert("/^ ".to_owned()),
            ),
            (
                Chord::parse("Ctrl+Shift+X").map_err(|e| e.to_string())?,
                ShortcutAction::Insert("say \"hi\" # not a comment".to_owned()),
            ),
        ][..],
    )
}

#[test]
fn settings_round_trip() -> TestResult {
    let defaults = EditorSettings::default();
    check(
        "defaults",
        parse(&defaults.to_toml())?,
        EditorSettings::default(),
    )?;
    let full = parse(FULL_SETTINGS)?;
    let rendered = full.to_toml();
    check("full", parse(&rendered)?, parse(FULL_SETTINGS)?)?;
    check("stable", parse(&rendered)?.to_toml(), rendered)
}

#[test]
fn settings_errors() -> TestResult {
    use SettingsErrorKind as E;
    expect_settings_error(
        "[editor]\nbraces = true\n[colors]\n",
        3,
        E::UnknownTable {
            name: "colors".to_owned(),
        },
    )?;
    expect_settings_error(
        "[editor]\nbracez = true\n",
        2,
        E::UnknownKey {
            table: "editor".to_owned(),
            key: "bracez".to_owned(),
        },
    )?;
    expect_settings_error(
        "braces = true\n",
        1,
        E::KeyOutsideTable {
            key: "braces".to_owned(),
        },
    )?;
    expect_settings_error(
        "[editor]\nbraces = \"yes\"\n",
        2,
        E::ExpectedBoolean {
            key: "braces".to_owned(),
        },
    )?;
    expect_settings_error(
        "[editor]\nbraces = 1\n",
        2,
        E::ExpectedBoolean {
            key: "braces".to_owned(),
        },
    )?;
    expect_settings_error(
        "[editor]\nbraces = true\nbraces = false\n",
        3,
        E::DuplicateKey {
            key: "braces".to_owned(),
        },
    )?;
    expect_settings_error(
        "[editor]\n[editor]\n",
        2,
        E::DuplicateTable {
            name: "editor".to_owned(),
        },
    )?;
    expect_settings_error(
        "[shortcuts.comparative]\n\">>\" = \"cha\"\n",
        2,
        E::InvalidComparativeKey {
            key: ">>".to_owned(),
        },
    )?;
    expect_settings_error(
        "[shortcuts.comparative]\n\">\" = \"zato\"\n",
        2,
        E::InvalidComparativeValue {
            value: "zato".to_owned(),
        },
    )?;
    expect_settings_error(
        "[shortcuts.comparative]\nc = \"cha\"\nC = \"chu\"\n",
        3,
        E::DuplicateKey {
            key: "C".to_owned(),
        },
    )?;
    expect_settings_error(
        "[custom_shortcuts]\n\"Hyper+X\" = \"x\"\n",
        2,
        E::InvalidChord {
            chord: "Hyper+X".to_owned(),
            error: ChordError::UnknownModifier {
                modifier: "Hyper".to_owned(),
            },
        },
    )?;
    expect_settings_error(
        "[custom_shortcuts]\n\"Alt+V\" = \"a\"\n\"alt+v\" = \"b\"\n",
        3,
        E::DuplicateChord {
            chord: "Alt+V".to_owned(),
        },
    )?;
    expect_settings_error(
        "[custom_shortcuts]\n\"Alt+C\" = \"x\"\n",
        2,
        E::ChordConflict {
            chord: "Alt+C".to_owned(),
        },
    )?;
    expect_settings_error(
        "[custom_shortcuts]\n\"Alt+T\" = \"a\tb\"\n",
        2,
        E::ControlCharacterInInsertion { ch: '\t' },
    )?;
    expect_settings_error(
        "[custom_shortcuts]\n\"Alt+T\" = \"\"\n",
        2,
        E::EmptyInsertion,
    )?;
    // Syntax errors.
    expect_settings_error("[editor\n", 1, E::InvalidTableHeader)?;
    expect_settings_error("[[editor]]\n", 1, E::InvalidTableHeader)?;
    expect_settings_error("[editor]\nbraces true\n", 2, E::ExpectedEquals)?;
    expect_settings_error("[editor]\nbraces = yes\n", 2, E::InvalidValue)?;
    expect_settings_error("[editor]\nbraces = true false\n", 2, E::TrailingCharacters)?;
    expect_settings_error(
        "[custom_shortcuts]\n\"F5\" = \"open\n",
        2,
        E::UnterminatedString,
    )?;
    expect_settings_error(
        "[custom_shortcuts]\n\"F5\" = \"a\\nb\"\n",
        2,
        E::InvalidEscape,
    )?;
    expect_settings_error("[editor]\na.b = true\n", 2, E::InvalidKey)?;
    // All errors are collected, sorted by line.
    let errors = parse_settings("x = 1\n[editor]\nfoo = true\n[bar]\n")
        .err()
        .ok_or("accepted")?;
    check(
        "lines",
        errors.iter().map(|e| e.line).collect::<Vec<_>>(),
        vec![1, 3, 4],
    )
}

#[test]
fn chord_parsing_and_normalization() -> TestResult {
    let parse = |text: &str| Chord::parse(text);
    check("case insensitive", parse("alt+c"), Ok(Chord::alt('C')))?;
    check("display", Chord::alt('c').to_string(), "Alt+C".to_owned())?;
    let chord = parse("Shift+Ctrl+x").map_err(|e| e.to_string())?;
    check("order", chord.to_string(), "Ctrl+Shift+X".to_owned())?;
    check(
        "all modifiers",
        parse("meta+SHIFT+alt+CTRL+F1").map(|c| c.to_string()),
        Ok("Ctrl+Alt+Shift+Meta+F1".to_owned()),
    )?;
    check(
        "function key",
        parse("F5").map(|c| c.key),
        Ok(Key::Function(5)),
    )?;
    check("f12", parse("f12").map(|c| c.key), Ok(Key::Function(12)))?;
    check(
        "plus key",
        parse("Ctrl++").map(|c| c.key),
        Ok(Key::Char('+')),
    )?;
    check(
        "plus round trip",
        parse("Ctrl++").map(|c| c.to_string()),
        Ok("Ctrl++".to_owned()),
    )?;
    check(
        "F13",
        parse("F13"),
        Err(ChordError::InvalidKey {
            key: "F13".to_owned(),
        }),
    )?;
    check(
        "F0",
        parse("F0"),
        Err(ChordError::InvalidKey {
            key: "F0".to_owned(),
        }),
    )?;
    check(
        "word key",
        parse("Alt+Tab"),
        Err(ChordError::InvalidKey {
            key: "Tab".to_owned(),
        }),
    )?;
    check("no modifier", parse("x"), Err(ChordError::MissingModifier))?;
    check(
        "shift only",
        parse("Shift+x"),
        Err(ChordError::MissingModifier),
    )?;
    check(
        "duplicate",
        parse("Alt+alt+x"),
        Err(ChordError::DuplicateModifier {
            modifier: "alt".to_owned(),
        }),
    )?;
    check("empty", parse(""), Err(ChordError::Empty))
}

// ═══════════════════════════ Shortcuts ═══════════════════════════

#[test]
fn comparative_prefix_shortcuts() -> TestResult {
    let settings = EditorSettings::default();
    let resolver = ShortcutResolver::new(&settings);
    let session = Session::new();
    let prefix: KeyPress = Chord::alt('c');
    check(
        "pending",
        resolver.resolve(&session, &[prefix]),
        ShortcutOutcome::Pending,
    )?;
    for (ch, surface) in [
        ('>', "cha"),
        ('<', "chu"),
        ('=', "col"),
        ('!', "cla"),
        ('~', "cal"),
        ('[', "cua"),
        ('-', "cuo"),
        ('+', "cup"),
    ] {
        check(
            surface,
            resolver.resolve(&session, &[prefix, Chord::plain(ch)]),
            ShortcutOutcome::Insert(surface.to_owned()),
        )?;
    }
    // Shift is tolerated on the second key (`>` needs it on many layouts).
    let shifted = Chord {
        shift: true,
        ..Chord::plain('>')
    };
    check(
        "shifted",
        resolver.resolve(&session, &[prefix, shifted]),
        ShortcutOutcome::Insert("cha".to_owned()),
    )?;
    check(
        "unbound",
        resolver.resolve(&session, &[prefix, Chord::plain('?')]),
        ShortcutOutcome::NoMatch,
    )?;
    check(
        "modified second key",
        resolver.resolve(&session, &[prefix, Chord::alt('>')]),
        ShortcutOutcome::NoMatch,
    )?;
    check(
        "too long",
        resolver.resolve(&session, &[prefix, Chord::plain('>'), Chord::plain('>')]),
        ShortcutOutcome::NoMatch,
    )?;
    check(
        "empty",
        resolver.resolve(&session, &[]),
        ShortcutOutcome::NoMatch,
    )?;
    check(
        "unknown chord",
        resolver.resolve(&session, &[Chord::alt('z')]),
        ShortcutOutcome::NoMatch,
    )
}

#[test]
fn custom_shortcuts_and_live_variables() -> TestResult {
    let settings = parse(FULL_SETTINGS)?;
    let resolver = ShortcutResolver::new(&settings);
    let mut session = Session::new();
    check(
        "no variables yet",
        resolver.resolve(&session, &[Chord::alt('v')]),
        ShortcutOutcome::LiveVariables(Vec::new()),
    )?;
    session
        .ingest("/^ defu dev a x y defu soglia a y y ta k nu dev ^/")
        .map_err(|d| format!("{d:#?}"))?;
    check(
        "live variables",
        resolver.resolve(&session, &[Chord::alt('V')]),
        ShortcutOutcome::LiveVariables(vec!["dev".to_owned(), "soglia".to_owned()]),
    )?;
    check(
        "insert",
        resolver.resolve(&session, &[Chord::parse("F5").map_err(|e| e.to_string())?]),
        ShortcutOutcome::Insert("/^ ".to_owned()),
    )?;
    // Custom prefix from the file; the default prefix no longer applies.
    let prefix = Chord::parse("Ctrl+Alt+K").map_err(|e| e.to_string())?;
    check(
        "custom prefix",
        resolver.resolve(&session, &[prefix, Chord::plain('C')]),
        ShortcutOutcome::Insert("col".to_owned()),
    )?;
    check(
        "old prefix",
        resolver.resolve(&session, &[Chord::alt('c')]),
        ShortcutOutcome::NoMatch,
    )?;
    check(
        "custom chord is not a prefix",
        resolver.resolve(&session, &[Chord::alt('v'), Chord::plain('x')]),
        ShortcutOutcome::NoMatch,
    )
}

// ═══════════════════════════ v4.1: settings, actions, suggestions, copy ═══════════════════════════

const V41_SETTINGS: &str = r#"
[editor]
autocomplete = "manual"

[shortcuts.actions]
suggest = "Ctrl+Down"
copy_absolute = "Ctrl+Shift+C"

[warnings]
ERR_SEMANTIC = "warning"
ERR_SCOPE = "warning"
ERR_REFERENCE = "error"
"#;

#[test]
fn settings_v4_1_sections() -> TestResult {
    let defaults = EditorSettings::default();
    check(
        "autocomplete default",
        defaults.autocomplete,
        Autocomplete::Auto,
    )?;
    check(
        "actions default",
        defaults.actions,
        ActionShortcuts {
            suggest: Chord::alt_down(),
            copy_absolute: Chord::ctrl('c'),
            copy_relative: Chord::alt('c'),
        },
    )?;
    check("warnings default", defaults.warnings, WarningCodes::NONE)?;
    let settings = parse(V41_SETTINGS)?;
    check("manual", settings.autocomplete, Autocomplete::Manual)?;
    check(
        "suggest",
        settings.actions.suggest,
        Chord::parse("Ctrl+Down").map_err(|e| e.to_string())?,
    )?;
    check(
        "copy_relative default",
        settings.actions.copy_relative,
        Chord::alt('c'),
    )?;
    check(
        "warnings",
        settings.warnings,
        WarningCodes::NONE
            .with(ErrorCode::Semantic)
            .with(ErrorCode::Scope),
    )?;
    let rendered = settings.to_toml();
    check("round trip", parse(&rendered)?, parse(V41_SETTINGS)?)?;
    check(
        "Alt+Down key",
        Chord::parse("Alt+Down").map(|c| c.key),
        Ok(Key::Down),
    )?;
    check(
        "Down display",
        Chord::alt_down().to_string(),
        "Alt+Down".to_owned(),
    )?;
    // The warnings feed the parser configuration.
    let config = crate::ParserConfig {
        warnings: settings.warnings,
        ..crate::ParserConfig::default()
    };
    let mut session = Session::with_config(config);
    session
        .ingest("/^ quata x ^/")
        .map_err(|d| format!("{d:#?}"))?;
    Ok(())
}

#[test]
fn settings_v4_1_errors() -> TestResult {
    use SettingsErrorKind as E;
    expect_settings_error(
        "[editor]\nautocomplete = \"sometimes\"\n",
        2,
        E::InvalidAutocomplete {
            value: "sometimes".to_owned(),
        },
    )?;
    expect_settings_error(
        "[editor]\nautocomplete = true\n",
        2,
        E::ExpectedString {
            key: "autocomplete".to_owned(),
        },
    )?;
    expect_settings_error(
        "[warnings]\nERR_COLOR = \"warning\"\n",
        2,
        E::UnknownErrorCode {
            code: "ERR_COLOR".to_owned(),
        },
    )?;
    expect_settings_error(
        "[warnings]\nERR_SCOPE = \"maybe\"\n",
        2,
        E::InvalidSeverity {
            value: "maybe".to_owned(),
        },
    )?;
    expect_settings_error(
        "[shortcuts.actions]\npaste = \"Ctrl+V\"\n",
        2,
        E::UnknownKey {
            table: "shortcuts.actions".to_owned(),
            key: "paste".to_owned(),
        },
    )?;
    expect_settings_error(
        "[custom_shortcuts]\n\"Ctrl+C\" = \"x\"\n",
        2,
        E::ChordConflict {
            chord: "Ctrl+C".to_owned(),
        },
    )?;
    expect_settings_error(
        "[custom_shortcuts]\n\"Alt+Down\" = \"x\"\n",
        2,
        E::ChordConflict {
            chord: "Alt+Down".to_owned(),
        },
    )
}

#[test]
fn alt_c_ctrl_c_and_alt_down_are_contextual_v4_1() -> TestResult {
    let settings = EditorSettings::default();
    let resolver = ShortcutResolver::new(&settings);
    let session = Session::new();
    let selected = ShortcutContext { selection: true };
    let typing = ShortcutContext::default();
    check(
        "Alt+C with a selection",
        resolver.resolve_in(&session, selected, &[Chord::alt('c')]),
        ShortcutOutcome::CopyRelative,
    )?;
    check(
        "Alt+C while typing",
        resolver.resolve_in(&session, typing, &[Chord::alt('c')]),
        ShortcutOutcome::Pending,
    )?;
    check(
        "Alt+C then > while typing",
        resolver.resolve_in(&session, typing, &[Chord::alt('c'), Chord::plain('>')]),
        ShortcutOutcome::Insert("cha".to_owned()),
    )?;
    check(
        "Ctrl+C with a selection",
        resolver.resolve_in(&session, selected, &[Chord::ctrl('c')]),
        ShortcutOutcome::CopyAbsolute,
    )?;
    check(
        "Ctrl+C without a selection",
        resolver.resolve_in(&session, typing, &[Chord::ctrl('c')]),
        ShortcutOutcome::NoMatch,
    )?;
    for context in [typing, selected] {
        check(
            "Alt+Down",
            resolver.resolve_in(&session, context, &[Chord::alt_down()]),
            ShortcutOutcome::Suggest,
        )?;
    }
    Ok(())
}

#[test]
fn suggestions_recent_glyphs_semas_and_labels_v4_1() -> TestResult {
    let dti = fixture_with_labels()?;
    let mut session = Session::new();
    session
        .ingest("/^ ta verde nu casa : ta vedere nu Roma ^/ /^ ta valere nu cane ^/")
        .map_err(|d| format!("{d:#?}"))?;
    let glyph = |text: &str| Suggestion {
        kind: SuggestionKind::Glyph,
        text: text.to_owned(),
    };
    let label = |text: &str| Suggestion {
        kind: SuggestionKind::Label,
        text: text.to_owned(),
    };
    let sema = |text: &str| Suggestion {
        kind: SuggestionKind::Sema,
        text: text.to_owned(),
    };
    // Most recent first, then semas whose text or signal glyph match.
    check(
        "prefix v",
        suggestions("v", &session, Some(&dti)),
        vec![
            glyph("valere"),
            glyph("vedere"),
            glyph("verde"),
            sema("ta valere nu cane"),
            sema("ta vedere nu Roma"),
            sema("ta verde nu casa"),
        ],
    )?;
    // Glyph / Label disambiguation for a homonym.
    check(
        "homonym",
        suggestions("cas", &session, Some(&dti)),
        vec![glyph("casa"), label("casa")],
    )?;
    // Labels of the local DTI, also when not written yet.
    check(
        "labels",
        suggestions("R", &session, Some(&dti)),
        vec![label("Roma")],
    )?;
    check(
        "dimora",
        suggestions("dim", &session, Some(&dti)),
        vec![label("dimora")],
    )?;
    // The user's choice for a slot wins.
    let ids = session
        .ingest("/^ ta x nu casa ^/")
        .map_err(|d| format!("{d:#?}"))?;
    let id = ids.first().copied().ok_or("no period")?;
    let slot = SlotRef {
        clause: GlobalClause {
            period: id,
            clause: ClauseId::new(0),
        },
        slot: SlotIndex::new(1),
    };
    check("choice", session.set_slot_kind(slot, SlotKind::Label), true)?;
    check(
        "chosen kind first",
        suggestions("cas", &session, Some(&dti)),
        vec![label("casa"), glyph("casa")],
    )?;
    // Without a DTI every token is a glyph.
    check(
        "no dti",
        suggestions("R", &session, None),
        vec![glyph("Roma")],
    )
}

#[test]
fn copy_subordinates_absolute_and_relative_v4_1() -> TestResult {
    let text = "/^ ta p nu q :: ta a nu b ::: ta c nu d y :: ta e nu f ^/";
    let start = text.find(":: ta a").ok_or("no selection")?;
    let end = text.find(" ^/").ok_or("no end")?;
    check(
        "absolute",
        copy_subordinates(text, start..end, CopyMode::Absolute, Some("blocco")),
        Ok(CopiedBlock {
            text: ":: anchor blocco :: ta a nu b ::: ta c nu d y :: ta e nu f :: end ~".to_owned(),
            id: "blocco".to_owned(),
        }),
    )?;
    check(
        "relative with generated id",
        copy_subordinates(text, start..end, CopyMode::Relative, None),
        Ok(CopiedBlock {
            text: ":: anchorrel copy1 : ta a nu b :: ta c nu d y : ta e nu f : end ~".to_owned(),
            id: "copy1".to_owned(),
        }),
    )?;
    // The generated id avoids the tokens of the text.
    let taken = "/^ ta copy1 nu q : ta a nu b ^/";
    let start = taken.find(": ta a").ok_or("no selection")?;
    let end = taken.find(" ^/").ok_or("no end")?;
    check(
        "fresh id",
        copy_subordinates(taken, start..end, CopyMode::Relative, None).map(|c| c.id),
        Ok("copy2".to_owned()),
    )?;
    // A colon run in an even slot is a glyph (Slot Locking).
    let glyphs = "/^ ta p nu q : ta a nu : ^/";
    let start = glyphs.find(": ta a").ok_or("no selection")?;
    let end = glyphs.find(" ^/").ok_or("no end")?;
    check(
        "glyph colon",
        copy_subordinates(glyphs, start..end, CopyMode::Absolute, Some("k")).map(|c| c.text),
        Ok(": anchor k : ta a nu : : end ~".to_owned()),
    )?;
    // The pasted block is valid where it was copied from.
    let pasted = format!(
        "/^ ta p nu q {} ^/",
        copy_subordinates(
            text,
            text.find(":: ta a").ok_or("x")?..text.find(" ^/").ok_or("x")?,
            CopyMode::Relative,
            Some("k")
        )
        .map_err(|e| e.to_string())?
        .text
        .replacen("::", ":", 1)
    );
    expect_ingest(&pasted)?;
    // Errors.
    check(
        "empty",
        copy_subordinates(text, 3..3, CopyMode::Absolute, None),
        Err(CopyError::EmptySelection),
    )?;
    check(
        "not a subordinate",
        copy_subordinates(text, 3..12, CopyMode::Absolute, None),
        Err(CopyError::NotSubordinate),
    )?;
    let start = text.find(":: ta a").ok_or("no selection")?;
    check(
        "incomplete",
        copy_subordinates(
            text,
            start..start.saturating_add(10),
            CopyMode::Absolute,
            None,
        ),
        Err(CopyError::IncompleteClause),
    )?;
    check(
        "out of range",
        copy_subordinates(text, 0..10_000, CopyMode::Absolute, None),
        Err(CopyError::InvalidSelection),
    )?;
    check(
        "invalid id",
        copy_subordinates(text, start..end_of(text)?, CopyMode::Absolute, Some("~")),
        Err(CopyError::InvalidId { id: "~".to_owned() }),
    )
}

fn end_of(text: &str) -> Result<usize, String> {
    text.find(" ^/").ok_or_else(|| "no end".to_owned())
}
