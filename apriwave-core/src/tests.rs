//! Stage 1 tests. No `.unwrap()`: every test returns `Result` and uses `?`.

use crate::ast::{
    Case, ClauseBlock, LinkedClause, PortabilityKeyword, PrimarySignal, SecondarySignal,
};
use crate::error::{DeterminerClass, Diagnostic, DiagnosticKind, ErrorCode, GlyphFault, Limit};
use crate::parser::ParserConfig;
use crate::printer::render_period;
use crate::session::{PeriodId, Session};
use crate::tiles;
use core::num::NonZeroU8;

type TestResult = Result<(), String>;

/// Neutral period used to give `/^≈` and `/^≃` an antecedent (Q23).
const PRIMER: &str = "/^ ta inizio nu sistema ^/";

fn accept(session: &mut Session, src: &str) -> Result<Vec<PeriodId>, String> {
    session
        .ingest(src)
        .map_err(|d| format!("rejected {src:?}: {d:#?}"))
}

fn render_all(session: &Session, ids: &[PeriodId]) -> Result<String, String> {
    let mut parts = Vec::new();
    for id in ids {
        let period = session
            .period(*id)
            .ok_or_else(|| format!("period {id:?} missing"))?;
        parts.push(render_period(period, session.interner()).map_err(|e| e.to_string())?);
    }
    Ok(parts.join(" "))
}

/// Accepts `src` and verifies the exact round-trip `parse → render == src`.
fn roundtrip_in(session: &mut Session, src: &str) -> TestResult {
    let ids = accept(session, src)?;
    let rendered = render_all(session, &ids)?;
    if rendered == src {
        Ok(())
    } else {
        Err(format!(
            "round-trip mismatch:\n  in:  {src}\n  out: {rendered}"
        ))
    }
}

fn roundtrip(src: &str) -> TestResult {
    roundtrip_in(&mut Session::new(), src)
}

fn roundtrip_after_primer(src: &str) -> TestResult {
    let mut session = Session::new();
    accept(&mut session, PRIMER)?;
    roundtrip_in(&mut session, src)
}

fn reject_in(session: &mut Session, src: &str) -> Result<Vec<Diagnostic>, String> {
    match session.ingest(src) {
        Ok(_) => Err(format!("accepted but should have failed: {src:?}")),
        Err(diagnostics) => Ok(diagnostics),
    }
}

fn expect_in(
    session: &mut Session,
    src: &str,
    kind: DiagnosticKind,
    code: ErrorCode,
) -> TestResult {
    let diagnostics = reject_in(session, src)?;
    if kind.code() != code {
        return Err(format!("{kind:?} does not match {code}"));
    }
    if diagnostics
        .iter()
        .any(|d| d.kind == kind && d.code() == code)
    {
        Ok(())
    } else {
        Err(format!(
            "{src:?}: expected {kind:?} [{code}], got {diagnostics:#?}"
        ))
    }
}

fn expect(src: &str, kind: DiagnosticKind, code: ErrorCode) -> TestResult {
    expect_in(&mut Session::new(), src, kind, code)
}

// ═══════════════════ Lexical properties (design §1) ═══════════════════

fn assert_prefix_free(name: &str, items: &[String]) -> TestResult {
    for (i, a) in items.iter().enumerate() {
        for (j, b) in items.iter().enumerate() {
            if i != j && b.starts_with(a.as_str()) {
                return Err(format!("{name}: '{a}' is a prefix of '{b}'"));
            }
        }
    }
    Ok(())
}

#[test]
fn determiners_are_prefix_free() -> TestResult {
    assert_prefix_free("determiners", &tiles::determiner_surfaces())
}

#[test]
fn signal_heads_are_prefix_free() -> TestResult {
    let mut heads: Vec<String> = PrimarySignal::ALL
        .iter()
        .map(|p| p.surface().to_owned())
        .collect();
    heads.extend(SecondarySignal::ALL.iter().map(|s| s.surface().to_owned()));
    heads.push(crate::ast::DEEP_SURFACE.to_owned());
    heads.push(crate::ast::DefSignal::SURFACE.to_owned());
    assert_prefix_free("signal heads", &heads)
}

#[test]
fn every_case_tile_has_a_unique_reading() -> TestResult {
    let numbers = ["", "u", "m", "num"];
    for case in Case::ALL {
        for number in numbers {
            let text = format!("{}{number}", case.stem());
            let readings = Case::ALL
                .iter()
                .filter(|other| {
                    text.strip_prefix(other.stem())
                        .is_some_and(|rest| numbers.contains(&rest))
                })
                .count();
            if readings != 1 {
                return Err(format!("'{text}' has {readings} readings"));
            }
            let parsed =
                tiles::parse_case(&text).ok_or_else(|| format!("'{text}' not recognized"))?;
            if parsed.case != case {
                return Err(format!("'{text}' read as {:?}", parsed.case));
            }
        }
    }
    Ok(())
}

#[test]
fn du_is_dative_singular_and_abdu_is_ablative_temporal() -> TestResult {
    let du = tiles::parse_case("du").ok_or("du not recognized")?;
    if du.case != Case::Dative(None) || du.number != Some(crate::ast::ExplicitNumber::Singular) {
        return Err(format!("du read as {du:?}"));
    }
    let abdu = tiles::parse_case("abdu").ok_or("abdu not recognized")?;
    if abdu.case != Case::Ablative(Some(crate::ast::AblativeDeterminer::Du))
        || abdu.number.is_some()
    {
        return Err(format!("abdu read as {abdu:?}"));
    }
    Ok(())
}

// ═══════════════════ Manual examples (round-trip) ═══════════════════

#[test]
fn manual_examples_roundtrip() -> TestResult {
    roundtrip("/^ taref observe nu io ^/")?;
    roundtrip("/^ tacha grandis n canis a gatto ^/")?;
    roundtrip("/^ tachazato grandis nu canis a gatto ^/")?;
    roundtrip("/^ ta dicere nu io : quata venire n humanum ^/")?;
    roundtrip(
        "/^ ta vedere nu io au humanum : tazato venire n ~ sed to vedere nu io au humanum : tozato venire n ~ ^/",
    )?;
    roundtrip(
        "/^ ta vedere nu io au canis : ta abbaiare nu ~ :: tazato fame nu ~ : ta osservare nu gatto ^/",
    )?;
    roundtrip_after_primer(
        "/^≈ deep registrazione nu blink au video§rosso : tachazato temperatura§celsius nu tapo§porta a 30 ^/",
    )
}

#[test]
fn comparative_table_roundtrip() -> TestResult {
    for c in [
        "tacha", "tachu", "tacla", "tacol", "tacal", "tacua", "tacuo", "tacup",
    ] {
        roundtrip(&format!("/^ {c} grandis n canis a gatto ^/"))?;
    }
    Ok(())
}

#[test]
fn if_patterns_roundtrip() -> TestResult {
    for src in [
        "/^ ta b : tazato a ^/",
        "/^ ta c : tazato a :: tazato b ^/",
        "/^ ta c : tazato a y : tazato b ^/",
        "/^ ta c : tazato a vel : tazato b ^/",
        "/^ ta c : tazato a aut : tazato b ^/",
        "/^ ta b : tazato a sed ta c : tozato a ^/",
        "/^ ta b : tazalt a ^/",
        "/^ ta b : tazatv a ^/",
        "/^ ta b : tazasu a ^/",
    ] {
        roundtrip(src)?;
    }
    Ok(())
}

#[test]
fn all_cases_numbers_and_closes_roundtrip() -> TestResult {
    roundtrip("/^ ta x nnum a anum b gnum c dnum d abnum e ^/")?;
    roundtrip("/^ ta x nu a am b gu c dm d abu e ^/")?;
    roundtrip("/^ ta x dad a gpa b abef c aben d abel e abse f abfi g abcu h ^/")?;
    roundtrip("/^ ta x absi a absi b abdu c abpa d ^/")?;
    roundtrip("/^ ta x dadu e gpam f ^/")?;
    roundtrip("/^ ta x dadnum e ^/")?;
    roundtrip("/^ ta x /^/")?;
    roundtrip("/^ ta x ^^/")?;
    roundtrip("/^ ta a ^/ /^ ta b /^/ /^≃ ta c ^^/")
}

#[test]
fn signal_variants_roundtrip() -> TestResult {
    for sig in [
        "ta",
        "to",
        "ka",
        "ko",
        "ot",
        "oh",
        "taall",
        "tasome",
        "taallrefato",
        "tosomezasu",
        "quata",
        "queto",
        "quota",
        "quuka",
        "quatazato",
        "deep",
        "deepall",
        "deepzato",
        "deepalt",
        "taato",
        "deepsomeref",
    ] {
        roundtrip(&format!("/^ ta p : {sig} x nu y ^/"))?;
    }
    // A comparative requires an Accusative (Q19); ref and a comparative together are
    // therefore always incompatible (ref forbids the Accusative).
    for sig in ["taallchaato", "tosomecuozasu", "quatacolalt", "deepcup"] {
        roundtrip(&format!("/^ ta p : {sig} x nu y a z ^/"))?;
    }
    expect(
        "/^ taallrefchaato x nu y a z ^/",
        DiagnosticKind::AccusativeInReflexive,
        ErrorCode::Syntax,
    )?;
    roundtrip("/^ def x a y ^/")?;
    roundtrip("/^ defu x au y ^/")?;
    roundtrip("/^ defm x am y ^/")?;
    roundtrip("/^ defnum x anum y ^/")
}

#[test]
fn glyph_slot_variants_roundtrip() -> TestResult {
    roundtrip("/^ ta rilevare nu sonda au video§rosso : ta archiviare nu io au video~ ^/")?;
    roundtrip("/^ ta rilevare nu sonda au video : ta allarme nu io au video§rosso~ ^/")?;
    roundtrip("/^ ta vedere nu io au canis : ta abbaiare nu ~ : ta vedere~~ ^/")?;
    roundtrip("/^ ta vedere nu io : tozato ~~ ^/")?;
    roundtrip("/^ ta vedere nu io : ta ~ nu marco ^/")?;
    roundtrip("/^ ta video§rosso nu y : ta z au video§rosso~~ ^/")?;
    roundtrip("/^ ta log nu sensore au \"Errore.42(A)\" ^/")?;
    roundtrip("/^ ta log nu \"\" ^/")?;
    roundtrip("/^ ta leggere nu registro§errori§priorita§alta ^/")?;
    roundtrip("/^ ta 観察 nu ñandú au verde§smeraldo§142 ^/")
}

#[test]
fn tile_names_are_valid_glyphs_in_even_slots() -> TestResult {
    // Q8: in even slots, tile names are glyphs.
    roundtrip("/^ ta a nu y ^/")?;
    roundtrip("/^ ta sed nu ta au u ^/")?;
    roundtrip("/^ ta vel n aut ^/")
}

#[test]
fn dative_singular_du_in_sema_anaphora_log() -> TestResult {
    roundtrip("/^ ta rottura nu valvola : deep invio nu sistema au ~~ du centrale ^/")
}

#[test]
fn canonical_def_examples_q11() -> TestResult {
    roundtrip("/^ defu soglia a 30§celsius ^/ /^ tachazato temperatura nu sensore a soglia ^/")?;
    roundtrip("/^ defu dev a sensore§ottico y ta attivare nu dev : ta leggere nu dev ^/")?;
    roundtrip(
        "/^ defu dev a sensore§ottico y defu dev abfi sensore§termico y ta leggere nu dev y defu dev abse ~ ^/",
    )
}

#[test]
fn def_block_is_typed() -> TestResult {
    let mut session = Session::new();
    let ids = accept(
        &mut session,
        "/^ defu dev a sensore§ottico absi x absi y ^/",
    )?;
    let id = ids.first().ok_or("no period")?;
    let period = session.period(*id).ok_or("period missing")?;
    match &period.head {
        ClauseBlock::Definition(def) => {
            if def.operations.iter().count() == 3 {
                Ok(())
            } else {
                Err("expected 3 operations".to_owned())
            }
        }
        ClauseBlock::Relational(_) => Err("expected a DefinitionBlock".to_owned()),
    }
}

#[test]
fn portability_blocks_roundtrip() -> TestResult {
    roundtrip(
        "/^ ta radice nu x : ta padre nu a :: anchorrel port1 : ta figlio nu b :: ta nipote nu c : reloff port1 : ta fratello§esterno nu d : end ~ ^/",
    )?;
    roundtrip("/^ ta x : anchor blocco§1 : rel blocco§1 : end ~ ^/")?;
    let mut session = Session::new();
    let ids = accept(&mut session, "/^ ta x : anchor p : end ~ ^/")?;
    let id = ids.first().ok_or("no period")?;
    let period = session.period(*id).ok_or("period missing")?;
    match period.tail.first() {
        Some(LinkedClause::Portability { block, .. })
            if block.keyword() == PortabilityKeyword::Anchor =>
        {
            Ok(())
        }
        other => Err(format!("expected anchor, found {other:?}")),
    }
}

// ═══════════════════ ERR_ORDER ═══════════════════

#[test]
fn determiner_order_errors() -> TestResult {
    expect(
        "/^ tarefall x ^/",
        DiagnosticKind::DeterminerOutOfOrder {
            found: DeterminerClass::Quantifier,
            after: DeterminerClass::Reflexive,
        },
        ErrorCode::Order,
    )?;
    expect(
        "/^ tazatocha x a y ^/",
        DiagnosticKind::DeterminerOutOfOrder {
            found: DeterminerClass::Comparative,
            after: DeterminerClass::Conditional,
        },
        ErrorCode::Order,
    )?;
    expect(
        "/^ tarefref x ^/",
        DiagnosticKind::DuplicateDeterminer {
            first: DeterminerClass::Reflexive,
            second: DeterminerClass::Reflexive,
        },
        ErrorCode::Order,
    )?;
    expect(
        "/^ tachachu x a y ^/",
        DiagnosticKind::DuplicateDeterminer {
            first: DeterminerClass::Comparative,
            second: DeterminerClass::Comparative,
        },
        ErrorCode::Order,
    )?;
    // Q5: temporal and conditional are mutually exclusive.
    expect(
        "/^ taaltzato x ^/",
        DiagnosticKind::DuplicateDeterminer {
            first: DeterminerClass::Temporal,
            second: DeterminerClass::Conditional,
        },
        ErrorCode::Order,
    )?;
    expect(
        "/^ taallrefall x ^/",
        DiagnosticKind::DuplicateDeterminer {
            first: DeterminerClass::Quantifier,
            second: DeterminerClass::Quantifier,
        },
        ErrorCode::Order,
    )?;
    // Rule 1B: the number closes the signal tile.
    expect(
        "/^ tauzato x ^/",
        DiagnosticKind::DeterminerOutOfOrder {
            found: DeterminerClass::Conditional,
            after: DeterminerClass::Number,
        },
        ErrorCode::Order,
    )?;
    expect(
        "/^ tauu x ^/",
        DiagnosticKind::DuplicateDeterminer {
            first: DeterminerClass::Number,
            second: DeterminerClass::Number,
        },
        ErrorCode::Order,
    )?;
    expect(
        "/^ deepmu x ^/",
        DiagnosticKind::DuplicateDeterminer {
            first: DeterminerClass::Number,
            second: DeterminerClass::Number,
        },
        ErrorCode::Order,
    )
}

// ═══════════════════ ERR_SYNTAX ═══════════════════

#[test]
fn reflexive_forbids_accusative() -> TestResult {
    expect(
        "/^ taref observe nu io au te ^/",
        DiagnosticKind::AccusativeInReflexive,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ taref observe nu io anum te ^/",
        DiagnosticKind::AccusativeInReflexive,
        ErrorCode::Syntax,
    )
}

#[test]
fn signal_syntax_errors() -> TestResult {
    expect(
        "/^ deepta x ^/",
        DiagnosticKind::UnknownSignalTile,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta p : qua x ^/",
        DiagnosticKind::MissingPrimaryAfterSecondary,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ defzato x a y ^/",
        DiagnosticKind::DeterminerOnDef,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ defuref x a y ^/",
        DiagnosticKind::DeterminerOnDef,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ defta x a y ^/",
        DiagnosticKind::UnknownSignalTile,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ tx x ^/",
        DiagnosticKind::UnknownSignalTile,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ Ta x ^/",
        DiagnosticKind::UppercaseCharacter { ch: 'T' },
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta.zato x ^/",
        DiagnosticKind::ForbiddenSeparator { ch: '.' },
        ErrorCode::Syntax,
    )
}

#[test]
fn case_syntax_errors() -> TestResult {
    // Q2: only `dad`, never `dda`.
    expect(
        "/^ ta x dda y ^/",
        DiagnosticKind::UnknownCaseTile,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x num y ^/",
        DiagnosticKind::UnknownCaseTile,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x nef y ^/",
        DiagnosticKind::UnknownCaseTile,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x Nu y ^/",
        DiagnosticKind::UppercaseCharacter { ch: 'N' },
        ErrorCode::Syntax,
    )
}

#[test]
fn missing_glyph_and_structure() -> TestResult {
    expect(
        "/^ ta ^/",
        DiagnosticKind::SignalTileWithoutGlyph,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta : tazato a ^/",
        DiagnosticKind::SignalTileWithoutGlyph,
        ErrorCode::Syntax,
    )?;
    expect(
        "ta x ^/",
        DiagnosticKind::MissingPeriodOpen,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x",
        DiagnosticKind::MissingPeriodClose,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x /^ ta y ^/",
        DiagnosticKind::MissingPeriodClose,
        ErrorCode::Syntax,
    )?;
    expect(
        "",
        DiagnosticKind::UnexpectedEnd {
            expected: crate::error::Expected::PeriodOpen,
        },
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ^/",
        DiagnosticKind::UnexpectedToken {
            expected: crate::error::Expected::SignalTile,
        },
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x y ^/",
        DiagnosticKind::UnexpectedToken {
            expected: crate::error::Expected::SignalTile,
        },
        ErrorCode::Syntax,
    )
}

#[test]
fn whitespace_must_be_canonical() -> TestResult {
    expect(
        "/^  ta x ^/",
        DiagnosticKind::InvalidWhitespace,
        ErrorCode::Syntax,
    )?;
    expect(
        " /^ ta x ^/",
        DiagnosticKind::InvalidWhitespace,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x ^/ ",
        DiagnosticKind::InvalidWhitespace,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x ^/\n",
        DiagnosticKind::InvalidWhitespace,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^\tta x ^/",
        DiagnosticKind::InvalidWhitespace,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta\u{00A0}x ^/",
        DiagnosticKind::InvalidWhitespace,
        ErrorCode::Syntax,
    )
}

#[test]
fn glyph_syntax_errors() -> TestResult {
    let malformed = |fault| DiagnosticKind::MalformedGlyph { fault };
    expect(
        "/^ ta x§ ^/",
        malformed(GlyphFault::EmptySegment),
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta §x ^/",
        malformed(GlyphFault::EmptySegment),
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x§§y ^/",
        malformed(GlyphFault::EmptySegment),
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x-y ^/",
        malformed(GlyphFault::IllegalCharacter { ch: '-' }),
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x_y ^/",
        malformed(GlyphFault::IllegalCharacter { ch: '_' }),
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta a~b ^/",
        malformed(GlyphFault::IllegalCharacter { ch: '~' }),
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta ~~~ ^/",
        malformed(GlyphFault::IllegalCharacter { ch: '~' }),
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x.y ^/",
        DiagnosticKind::ForbiddenSeparator { ch: '.' },
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta Canis ^/",
        DiagnosticKind::UppercaseCharacter { ch: 'C' },
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta \"ab ^/",
        malformed(GlyphFault::UnterminatedRawString),
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta \" ^/",
        malformed(GlyphFault::UnterminatedRawString),
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta \"a\"b\" ^/",
        malformed(GlyphFault::QuoteInsideRawString),
        ErrorCode::Syntax,
    )
}

#[test]
fn def_constraints_q11() -> TestResult {
    expect(
        "/^ defu x§y a z ^/",
        DiagnosticKind::DefNameNotAtomic,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ defu ~ a z ^/",
        DiagnosticKind::DefNameNotAtomic,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ defu x n z ^/",
        DiagnosticKind::DefOperationNotAllowed,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ defu x abfiu z ^/",
        DiagnosticKind::DefOperationNotAllowed,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ defu x abse z ^/",
        DiagnosticKind::UnbindRequiresTilde,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ defu x ^/",
        DiagnosticKind::DefWithoutOperations,
        ErrorCode::Semantic,
    )?;
    // The old workaround (block after def without a connector) is forbidden.
    expect(
        "/^ defu dev a sensore§ottico ta attivare nu dev ^/",
        DiagnosticKind::UnknownCaseTile,
        ErrorCode::Syntax,
    )
}

#[test]
fn portability_constraints_q6() -> TestResult {
    expect(
        "/^ anchor p ^/",
        DiagnosticKind::PortabilityOutsideSubordination,
        ErrorCode::Scope,
    )?;
    expect(
        "/^ ta x y anchor p ^/",
        DiagnosticKind::PortabilityOutsideSubordination,
        ErrorCode::Scope,
    )?;
    expect(
        "/^ ta x y : anchor p ^/",
        DiagnosticKind::PortabilityOutsideSubordination,
        ErrorCode::Scope,
    )?;
    expect(
        "/^ ta x : end y ^/",
        DiagnosticKind::EndRequiresTilde,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x : anchor ~ ^/",
        DiagnosticKind::PortabilityOperandInvalid,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x : rel p~~ ^/",
        DiagnosticKind::PortabilityOperandInvalid,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x : anchor p nu z ^/",
        DiagnosticKind::PortabilityWithCasePairs,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x : anchorzato p ^/",
        DiagnosticKind::UnknownSignalTile,
        ErrorCode::Syntax,
    )
}

// ═══════════════════ ERR_SEMANTIC ═══════════════════

#[test]
fn comparative_requires_accusative_q19() -> TestResult {
    expect(
        "/^ tacha grandis n canis ^/",
        DiagnosticKind::ComparativeWithoutAccusative,
        ErrorCode::Semantic,
    )?;
    roundtrip("/^ tacha grandis n canis anum gatto ^/")?;
    roundtrip("/^ tacha grandis n canis am gatto ^/")
}

// ═══════════════════ ERR_SCOPE ═══════════════════

#[test]
fn synchronized_open_requires_antecedent_q23() -> TestResult {
    expect(
        "/^≈ ta x ^/",
        DiagnosticKind::SynchronizedOpenWithoutAntecedent,
        ErrorCode::Scope,
    )?;
    expect(
        "/^≃ ta x ^/",
        DiagnosticKind::SynchronizedOpenWithoutAntecedent,
        ErrorCode::Scope,
    )?;
    roundtrip("/^ ta x ^/ /^≈ ta y ^/")?;
    roundtrip_after_primer("/^≃ ta y ^/")
}

#[test]
fn dsp_depth_limit_q13() -> TestResult {
    let max = NonZeroU8::new(3).ok_or("3 is not NonZero")?;
    let mut session = Session::with_config(ParserConfig {
        max_dsp_depth: max,
        ..ParserConfig::default()
    });
    roundtrip_in(&mut session, "/^ ta a : ta b :: ta c ::: ta d ^/")?;
    expect_in(
        &mut session,
        "/^ ta a :::: ta b ^/",
        DiagnosticKind::LimitExceeded {
            limit: Limit::DspDepth,
        },
        ErrorCode::Scope,
    )?;
    let colons = ":".repeat(300);
    expect(
        &format!("/^ ta a {colons} ta b ^/"),
        DiagnosticKind::LimitExceeded {
            limit: Limit::DspDepth,
        },
        ErrorCode::Scope,
    )
}

// ═══════════════════ ERR_CASE_AMBIGUITY ═══════════════════

#[test]
fn case_without_glyph() -> TestResult {
    expect(
        "/^ ta vedere nu ^/",
        DiagnosticKind::CaseTileWithoutGlyph,
        ErrorCode::CaseAmbiguity,
    )?;
    expect(
        "/^ ta vedere nu : ta x ^/",
        DiagnosticKind::CaseTileWithoutGlyph,
        ErrorCode::CaseAmbiguity,
    )?;
    expect(
        "/^ ta vedere nu",
        DiagnosticKind::CaseTileWithoutGlyph,
        ErrorCode::CaseAmbiguity,
    )
}

#[test]
fn duplicate_cases_q21() -> TestResult {
    expect(
        "/^ ta x nu y n z ^/",
        DiagnosticKind::DuplicateCase,
        ErrorCode::CaseAmbiguity,
    )?;
    expect(
        "/^ ta x a y au z ^/",
        DiagnosticKind::DuplicateCase,
        ErrorCode::CaseAmbiguity,
    )?;
    expect(
        "/^ defu x a y au z ^/",
        DiagnosticKind::DuplicateCase,
        ErrorCode::CaseAmbiguity,
    )?;
    // Cases distinguished by determiner, and multiple `absi`, are allowed.
    roundtrip("/^ ta x ab y aben z abel w ^/")?;
    roundtrip("/^ ta x absi y absi z ^/")
}

// ═══════════════════ Session and recovery ═══════════════════

#[test]
fn rejected_input_leaves_session_untouched() -> TestResult {
    let mut session = Session::new();
    accept(&mut session, PRIMER)?;
    let symbols = session.interner().len();
    let periods = session.periods().len();
    reject_in(&mut session, "/^ ta nuovo§glifo nu altro ^/ /^ Ta x ^/")?;
    if session.interner().len() != symbols || session.periods().len() != periods {
        return Err("the rejected input modified the session".to_owned());
    }
    Ok(())
}

#[test]
fn recovery_collects_all_diagnostics_in_order_q25() -> TestResult {
    let diagnostics = reject_in(
        &mut Session::new(),
        "/^ tarefall x : ta y nu : Ta z ^/ /^ ta w x§ ^/",
    )?;
    let codes: Vec<ErrorCode> = diagnostics.iter().map(Diagnostic::code).collect();
    let expected = vec![
        ErrorCode::Order,
        ErrorCode::CaseAmbiguity,
        ErrorCode::Syntax,
        ErrorCode::Syntax,
    ];
    if codes != expected {
        return Err(format!(
            "codes {codes:?}, expected {expected:?}: {diagnostics:#?}"
        ));
    }
    let sorted = diagnostics.windows(2).all(|w| match w {
        [a, b] => a.span.start <= b.span.start,
        _ => true,
    });
    if sorted {
        Ok(())
    } else {
        Err("diagnostics not sorted".to_owned())
    }
}

// ═══════════════════ Interner and memory release ═══════════════════

#[test]
fn interner_single_copy_index_and_exact_rollback() -> TestResult {
    use crate::symbol::Interner;
    let mut interner = Interner::new();
    let words = ["zeta", "alfa", "mu", "beta", "alfa", "omega"];
    let mut symbols = Vec::new();
    for word in words {
        symbols.push(interner.intern(word).map_err(|_| "symbol space")?);
    }
    if interner.len() != 5 {
        return Err(format!("expected 5 distinct texts, got {}", interner.len()));
    }
    if symbols.get(1) != symbols.get(4) {
        return Err("the same text must give the same symbol".to_owned());
    }
    for (word, symbol) in words.iter().zip(&symbols) {
        if interner.lookup(word) != Some(*symbol) || interner.resolve(*symbol) != Some(*word) {
            return Err(format!("lookup/resolve mismatch for {word}"));
        }
    }
    // Rollback removes the texts and their index entries, nothing else.
    let mark = interner.mark();
    let late = interner.intern("aaa").map_err(|_| "symbol space")?;
    interner.intern("zzz").map_err(|_| "symbol space")?;
    interner.rollback(mark);
    if interner.len() != 5 || interner.lookup("aaa").is_some() || interner.lookup("zzz").is_some() {
        return Err("rollback left texts or index entries behind".to_owned());
    }
    if words.iter().any(|word| interner.lookup(word).is_none()) {
        return Err("rollback removed older symbols".to_owned());
    }
    // A text interned again after the rollback reuses the freed slot.
    if interner.intern("aaa").map_err(|_| "symbol space")? != late {
        return Err("the rolled-back slot must be reused".to_owned());
    }
    Ok(())
}

#[test]
fn session_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Session>();
}

#[test]
fn compact_releases_a_hard_closed_session() -> TestResult {
    use crate::session::CompactError;
    let mut session = Session::new();
    accept(&mut session, "/^ defu dev a x y ta k nu dev /^/")?;
    if session.compact() != Err(CompactError::NotHardClosed) {
        return Err("a soft-closed session must not be compacted".to_owned());
    }
    accept(&mut session, "/^ ta y ^^/")?;
    session.compact().map_err(|e| e.to_string())?;
    if !session.periods().is_empty() || !session.interner().is_empty() {
        return Err("compact must release periods and texts".to_owned());
    }
    // Future ingestion behaves as after `^^/`: no variable, no sync antecedent.
    if session.live_variable("dev").is_some() || session.has_sync_antecedent() {
        return Err("compact must not leave live state".to_owned());
    }
    reject_in(&mut session, "/^≈ ta z ^/")?;
    accept(&mut session, "/^ ta z ^/")?;
    // An empty session can always be compacted.
    Session::new().compact().map_err(|e| e.to_string())
}
