//! Stage 2 tests (D8, R1–R14, 3-level GC, atomicity).
//! No `.unwrap()`: every test returns `Result` and uses `?`.

use crate::analyze::{
    AnalyzedPeriod, ClauseAnalysis, ClauseId, CoordId, CoordNode, DspRole, GlobalClause,
    Resolution, SlotAnalysis, SlotIndex, SlotRef, SlotValue,
};
use crate::ast::{Comparative, Conditional, Quantifier, QuantumNumber, Temporal, TemporalMode};
use crate::error::{Diagnostic, DiagnosticKind, ErrorCode};
use crate::printer::render_glyph_slot;
use crate::session::{PeriodId, Session};

type TestResult = Result<(), String>;

const PRIMER: &str = "/^ ta inizio nu sistema ^/";

// ═══════════════════════════ Helper ═══════════════════════════

fn accept(session: &mut Session, src: &str) -> Result<Vec<PeriodId>, String> {
    session
        .ingest(src)
        .map_err(|d| format!("rejected {src:?}: {d:#?}"))
}

/// Fresh session with `src` accepted; returns the last period.
fn one(src: &str) -> Result<(Session, PeriodId), String> {
    let mut session = Session::new();
    let ids = accept(&mut session, src)?;
    let last = ids.last().copied().ok_or("no period")?;
    Ok((session, last))
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
    if diagnostics.iter().any(|d| d.kind == kind) {
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

fn analysis(session: &Session, id: PeriodId) -> Result<&AnalyzedPeriod, String> {
    session
        .analysis(id)
        .ok_or_else(|| format!("missing analysis for {id:?}"))
}

fn clause(session: &Session, id: PeriodId, index: u32) -> Result<&ClauseAnalysis, String> {
    analysis(session, id)?
        .clause(ClauseId::new(index))
        .ok_or_else(|| format!("clause {index} missing"))
}

/// Analysis of the `[Tile] [Glyph]` pair at position `position` (`0` = signal
/// pair) of clause `index`.
fn pair(
    session: &Session,
    id: PeriodId,
    index: u32,
    position: u32,
) -> Result<SlotAnalysis, String> {
    clause(session, id, index)?
        .slot(SlotIndex::new(position))
        .copied()
        .ok_or_else(|| format!("pair {position} of clause {index} missing"))
}

/// Human-readable text of a resolved value.
fn value_text(session: &Session, value: SlotValue) -> Result<String, String> {
    match value {
        SlotValue::Glyph(slot) | SlotValue::Raw(slot) => {
            let content = session.glyph_slot(slot).ok_or("slot missing")?;
            render_glyph_slot(content, session.interner()).map_err(|e| e.to_string())
        }
        SlotValue::Variable(binding) => {
            let binding = session.binding(binding).ok_or("binding missing")?;
            let name = session
                .interner()
                .resolve(binding.name())
                .ok_or("name missing")?;
            Ok(format!("${name}"))
        }
        SlotValue::Sema(clause) => Ok(format!(
            "~~{}.{}",
            clause.period.index(),
            clause.clause.index()
        )),
    }
}

fn check<T: PartialEq + core::fmt::Debug>(what: &str, got: T, want: T) -> TestResult {
    if got == want {
        Ok(())
    } else {
        Err(format!("{what}: got {got:?}, expected {want:?}"))
    }
}

fn check_pair_value(
    session: &Session,
    id: PeriodId,
    index: u32,
    position: u32,
    want: &str,
) -> TestResult {
    let slot = pair(session, id, index, position)?;
    check(
        &format!("value clause {index} pair {position}"),
        value_text(session, slot.value())?.as_str(),
        want,
    )
}

fn coord_text(period: &AnalyzedPeriod, id: CoordId) -> Result<String, String> {
    match period.coord(id).ok_or("coordination node missing")? {
        CoordNode::Clause(clause) => Ok(clause.index().to_string()),
        CoordNode::Group { conj, members } => {
            let parts = members
                .iter()
                .map(|m| coord_text(period, *m))
                .collect::<Result<Vec<String>, String>>()?;
            Ok(format!("{}({})", conj.surface(), parts.join(",")))
        }
    }
}

fn groups_text(period: &AnalyzedPeriod, groups: &[CoordId]) -> Result<Vec<String>, String> {
    groups.iter().map(|g| coord_text(period, *g)).collect()
}

fn child_groups(src: &str, parent: u32) -> Result<Vec<String>, String> {
    let (session, id) = one(src)?;
    let period = analysis(&session, id)?;
    let groups = clause(&session, id, parent)?.groups();
    groups_text(period, groups)
}

fn placement(session: &Session, id: PeriodId, index: u32) -> Result<(u8, Option<u32>), String> {
    let c = clause(session, id, index)?;
    Ok((c.depth(), c.parent().map(ClauseId::index)))
}

fn slot_ref(period: PeriodId, clause: u32, slot: SlotIndex) -> SlotRef {
    SlotRef {
        clause: GlobalClause {
            period,
            clause: ClauseId::new(clause),
        },
        slot,
    }
}

// ═══════════════════════════ Manual examples ═══════════════════════════

#[test]
fn manual_example_5_anaphora_and_sed() -> TestResult {
    let (s, id) = one(
        "/^ ta vedere nu io au humanum : tazato venire n ~ sed to vedere nu io au humanum : tozato venire n ~ ^/",
    )?;
    check("venire 1", placement(&s, id, 1)?, (1, Some(0)))?;
    check("to vedere", placement(&s, id, 2)?, (0, None))?;
    check("venire 2", placement(&s, id, 3)?, (1, Some(2)))?;
    check_pair_value(&s, id, 1, 1, "humanum")?;
    check_pair_value(&s, id, 3, 1, "humanum")?;
    // Immediate antecedent: the `au humanum` pair of the respective parent.
    check(
        "antecedent 1",
        pair(&s, id, 1, 1)?.resolution(),
        Resolution::GlyphAnaphora {
            antecedent: slot_ref(id, 0, SlotIndex::new(2)),
            value: SlotValue::Glyph(slot_ref(id, 0, SlotIndex::new(2))),
        },
    )?;
    check(
        "antecedent 2 (branch pruned by sed)",
        pair(&s, id, 3, 1)?.resolution(),
        Resolution::GlyphAnaphora {
            antecedent: slot_ref(id, 2, SlotIndex::new(2)),
            value: SlotValue::Glyph(slot_ref(id, 2, SlotIndex::new(2))),
        },
    )?;
    // Q18: `n ~` inherits the Singular of `au humanum`.
    check(
        "number",
        pair(&s, id, 1, 1)?.number(),
        QuantumNumber::Singular,
    )?;
    let period = analysis(&s, id)?;
    check(
        "roots",
        period.roots(),
        &[ClauseId::new(0), ClauseId::new(2)][..],
    )?;
    check(
        "root groups",
        groups_text(period, period.root_groups())?,
        vec!["sed(0,2)".to_owned()],
    )
}

#[test]
fn manual_example_6_dsp_tree_and_anaphora_chain() -> TestResult {
    let (s, id) = one(
        "/^ ta vedere nu io au canis : ta abbaiare nu ~ :: tazato fame nu ~ : ta osservare nu gatto ^/",
    )?;
    check("abbaiare", placement(&s, id, 1)?, (1, Some(0)))?;
    check("fame", placement(&s, id, 2)?, (2, Some(1)))?;
    check("osservare", placement(&s, id, 3)?, (1, Some(0)))?;
    check_pair_value(&s, id, 1, 1, "canis")?;
    // R9.1: the chain `nu ~` → `nu ~` → `au canis` is followed.
    check(
        "chain",
        pair(&s, id, 2, 1)?.resolution(),
        Resolution::GlyphAnaphora {
            antecedent: slot_ref(id, 1, SlotIndex::new(1)),
            value: SlotValue::Glyph(slot_ref(id, 0, SlotIndex::new(2))),
        },
    )?;
    check(
        "children of vedere",
        clause(&s, id, 0)?.children(),
        &[ClauseId::new(1), ClauseId::new(3)][..],
    )?;
    let period = analysis(&s, id)?;
    // R4b: siblings without a conjunction = distinct branches.
    check(
        "branches of vedere",
        groups_text(period, clause(&s, id, 0)?.groups())?,
        vec!["1".to_owned(), "3".to_owned()],
    )
}

#[test]
fn manual_example_7_after_primer() -> TestResult {
    let mut s = Session::new();
    accept(&mut s, PRIMER)?;
    let ids = accept(
        &mut s,
        "/^≈ deep registrazione nu blink au video§rosso : tachazato temperatura§celsius nu tapo§porta a 30 ^/",
    )?;
    let id = ids.first().copied().ok_or("no period")?;
    check("deep", clause(&s, id, 0)?.role(), DspRole::ForcedRoot)?;
    check("sub", placement(&s, id, 1)?, (1, Some(0)))?;
    let signal = clause(&s, id, 1)?
        .signal()
        .copied()
        .ok_or("signal missing")?;
    check("comparative", signal.comparative, Some(Comparative::Cha))?;
    check(
        "tense",
        signal.tense,
        Some(TemporalMode::Conditional(Conditional(Temporal::Ato))),
    )
}

// ═══════════════════════════ DSP (Q14, Q15, Q16) ═══════════════════════════

#[test]
fn conjunction_depth_rules_q15() -> TestResult {
    // `y` without `:` keeps the current depth; `sed` without `:` returns to 0.
    let (s, id) = one("/^ ta x nu a : ta y nu b :: ta w nu c y ta z au ~ ^/")?;
    check("y", placement(&s, id, 3)?, (2, Some(1)))?;
    check_pair_value(&s, id, 3, 1, "b")?;
    let (s, id) = one("/^ ta x nu a : ta y nu b :: ta w nu c sed ta z au ~ ^/")?;
    check("sed", placement(&s, id, 3)?, (0, None))?;
    // Stack pruning: the branches of `x` are no longer visible; the root `x` remains.
    check_pair_value(&s, id, 3, 1, "a")?;
    let (s, id) = one("/^ ta x nu a : ta y nu b :: ta w nu c y : ta z ^/")?;
    check("y :", placement(&s, id, 3)?, (1, Some(0)))
}

#[test]
fn level_skip_q14() -> TestResult {
    expect(
        "/^ ta a ::: ta b ^/",
        DiagnosticKind::DspLevelSkip {
            max_allowed: 1,
            requested: 3,
        },
        ErrorCode::Scope,
    )?;
    expect(
        "/^ ta a : ta b :::: ta c ^/",
        DiagnosticKind::DspLevelSkip {
            max_allowed: 2,
            requested: 4,
        },
        ErrorCode::Scope,
    )
}

#[test]
fn forced_roles_q16() -> TestResult {
    // `oh` without `:` = sibling of the last active block (even after `sed`).
    let (s, id) = one("/^ ta p : ta a :: ta b sed oh c ^/")?;
    check("oh", placement(&s, id, 3)?, (2, Some(1)))?;
    check("oh role", clause(&s, id, 3)?.role(), DspRole::ForcedSibling)?;
    let (s, id) = one("/^ oh marco ^/")?;
    check("oh head", placement(&s, id, 0)?, (0, None))?;
    // `deep` without `:` = level 0; with explicit `:` it honours the colons.
    let (s, id) = one("/^ ta p : ta a y deep d ^/")?;
    check("deep after y", placement(&s, id, 2)?, (0, None))?;
    let (s, id) = one("/^ ta p : deep d ^/")?;
    check(": deep", placement(&s, id, 1)?, (1, Some(0)))?;
    // Transparent `def`: checks the `:` but does not modify the stack.
    let (s, id) = one("/^ ta p : ta a :: defu v a x : ta b nu v ^/")?;
    check("def", clause(&s, id, 2)?.role(), DspRole::Transparent)?;
    check("def depth", clause(&s, id, 2)?.depth(), 2)?;
    check("after def", placement(&s, id, 3)?, (1, Some(0)))?;
    expect(
        "/^ ta p ::: defu v a x ^/",
        DiagnosticKind::DspLevelSkip {
            max_allowed: 1,
            requested: 3,
        },
        ErrorCode::Scope,
    )
}

#[test]
fn canonical_order_q21() -> TestResult {
    // Signal pair (0), then the case pairs n → a → d → g → ab.
    let (s, id) = one("/^ ta x ab a n b d c g e a f ^/")?;
    check(
        "order",
        clause(&s, id, 0)?.canonical_order(),
        &[0_u32, 2, 5, 3, 4, 1][..],
    )
}

// ═══════════════════════════ Coordination (Q15, R4) ═══════════════════════════

#[test]
fn coordination_groups() -> TestResult {
    check(
        "IF A AND B",
        child_groups("/^ ta c : tazato a y : tazato b ^/", 0)?,
        vec!["y(1,2)".to_owned()],
    )?;
    check(
        "R4b",
        child_groups("/^ ta p : ta a : ta b y : ta c ^/", 0)?,
        vec!["1".to_owned(), "y(2,3)".to_owned()],
    )?;
    check(
        "homogeneous chain flattened",
        child_groups("/^ ta p : ta a y : ta b y : ta c ^/", 0)?,
        vec!["y(1,2,3)".to_owned()],
    )?;
    check(
        "mixed chain, left-associative",
        child_groups("/^ ta p : ta a y : ta b vel : ta c ^/", 0)?,
        vec!["vel(y(1,2),3)".to_owned()],
    )?;
    check(
        "(a y b) sed c",
        child_groups("/^ ta p : ta a y : ta b sed : ta c ^/", 0)?,
        vec!["sed(y(1,2),3)".to_owned()],
    )?;
    check(
        "immediate sibling past a deeper branch",
        child_groups("/^ ta p : ta a :: ta x y : ta b ^/", 0)?,
        vec!["y(1,3)".to_owned()],
    )?;
    // R4a: `y` after a transparent `def` has no relational sibling on its left.
    let (s, id) = one("/^ defu dev a sensore§ottico y ta attivare nu dev : ta leggere nu dev ^/")?;
    let period = analysis(&s, id)?;
    check("roots", period.roots(), &[ClauseId::new(1)][..])?;
    check(
        "R4a",
        groups_text(period, period.root_groups())?,
        vec!["1".to_owned()],
    )
}

// ═══════════════════════════ Inheritance (Q17, R5, D8) ═══════════════════════════

#[test]
fn inheritance_is_transitive_r5() -> TestResult {
    let (s, id) = one("/^ taallalt x nu y : ta z nu w :: ta k nu q ^/")?;
    for index in [1, 2] {
        let signal = clause(&s, id, index)?
            .signal()
            .copied()
            .ok_or("signal missing")?;
        check("quantifier", signal.quantifier, Some(Quantifier::Universal))?;
        check("inherited quantifier", signal.quantifier_inherited, true)?;
        check(
            "tense",
            signal.tense,
            Some(TemporalMode::Indicative(Temporal::Alt)),
        )?;
        check("inherited tense", signal.tense_inherited, true)?;
    }
    Ok(())
}

#[test]
fn inheritance_overrides_and_exclusions_q17() -> TestResult {
    let (s, id) = one("/^ taalt x : taato y ^/")?;
    let signal = clause(&s, id, 1)?
        .signal()
        .copied()
        .ok_or("signal missing")?;
    check(
        "explicit ato",
        signal.tense,
        Some(TemporalMode::Indicative(Temporal::Ato)),
    )?;
    check("not inherited", signal.tense_inherited, false)?;

    let (s, id) = one("/^ taalt x : tazato y ^/")?;
    let signal = clause(&s, id, 1)?
        .signal()
        .copied()
        .ok_or("signal missing")?;
    check(
        "conditional",
        signal.tense,
        Some(TemporalMode::Conditional(Conditional(Temporal::Ato))),
    )?;

    // The comparative is not inherited and does not impose the Accusative on children.
    let (s, id) = one("/^ tacha x nu y a z : ta k nu w ^/")?;
    let signal = clause(&s, id, 1)?
        .signal()
        .copied()
        .ok_or("signal missing")?;
    check("comparative", signal.comparative, None)?;

    // `deep` does not inherit.
    let (s, id) = one("/^ taalt x : deep y ^/")?;
    let signal = clause(&s, id, 1)?
        .signal()
        .copied()
        .ok_or("signal missing")?;
    check("deep", signal.tense, None)?;
    check(
        "present default",
        signal.tense_or_present(),
        TemporalMode::Indicative(Temporal::Ato),
    )
}

#[test]
fn inherited_reflexive_forbids_accusative_d8() -> TestResult {
    expect(
        "/^ taref x nu io : ta y au z ^/",
        DiagnosticKind::AccusativeInReflexive,
        ErrorCode::Syntax,
    )?;
    let (s, id) = one("/^ taref x nu io : ta y nu z ^/")?;
    let signal = clause(&s, id, 1)?
        .signal()
        .copied()
        .ok_or("signal missing")?;
    check(
        "inherited ref",
        (signal.reflexive, signal.reflexive_inherited),
        (true, true),
    )
}

#[test]
fn quantifier_misuse_q20() -> TestResult {
    expect(
        "/^ ta p nu q : taall x ^/",
        DiagnosticKind::QuantifierMisuse,
        ErrorCode::Scope,
    )?;
    expect(
        "/^ taall p nu q : ta x ^/",
        DiagnosticKind::QuantifierMisuse,
        ErrorCode::Scope,
    )?;
    one("/^ taallref x ^/")?;
    one("/^ taallref p : ta x ^/")?;
    Ok(())
}

#[test]
fn secondary_signal_at_root_q22() -> TestResult {
    expect(
        "/^ quata x ^/",
        DiagnosticKind::SecondarySignalAtRoot,
        ErrorCode::Scope,
    )?;
    expect(
        "/^ ta x y quata z ^/",
        DiagnosticKind::SecondarySignalAtRoot,
        ErrorCode::Scope,
    )?;
    one("/^ ta dicere nu io : quata venire n humanum ^/")?;
    Ok(())
}

// ═══════════════════════════ Anchors (Q6, R1–R3) ═══════════════════════════

#[test]
fn anchorrel_relative_offsets() -> TestResult {
    let (s, id) = one(
        "/^ ta radice nu x : ta padre nu a :: anchorrel port1 : ta figlio nu b :: ta nipote nu c : reloff port1 : ta fratello§esterno nu d : end ~ ^/",
    )?;
    check("anchorrel", clause(&s, id, 2)?.role(), DspRole::Portability)?;
    check("anchorrel depth", clause(&s, id, 2)?.depth(), 2)?;
    check("child (K + 0)", placement(&s, id, 3)?, (2, Some(1)))?;
    check("grandchild (K + 1)", placement(&s, id, 4)?, (3, Some(3)))?;
    check("sibling after reloff", placement(&s, id, 6)?, (1, Some(0)))
}

#[test]
fn nested_anchor_uses_effective_depth_r1() -> TestResult {
    let (s, id) =
        one("/^ ta r : ta p :: anchorrel a1 : ta f :: anchorrel a2 : ta g : end ~ : end ~ ^/")?;
    check("a2 = K + (C - 1)", clause(&s, id, 4)?.depth(), 3)?;
    check("g", placement(&s, id, 5)?, (3, Some(3)))
}

#[test]
fn rel_reactivates_relative_mode() -> TestResult {
    let (s, id) = one("/^ ta r : ta p :: anchor a1 :: ta x : rel a1 : ta y : end ~ ^/")?;
    check("x (absolute)", placement(&s, id, 3)?, (2, Some(1)))?;
    check("y (relative)", placement(&s, id, 5)?, (2, Some(1)))?;
    one("/^ ta r : anchor blocco§1 : rel blocco§1 : end ~ ^/")?;
    Ok(())
}

#[test]
fn anchor_errors_r2_r3() -> TestResult {
    expect(
        "/^ ta r : rel zz ^/",
        DiagnosticKind::UnknownAnchor,
        ErrorCode::Reference,
    )?;
    expect(
        "/^ ta r : reloff zz ^/",
        DiagnosticKind::UnknownAnchor,
        ErrorCode::Reference,
    )?;
    expect(
        "/^ ta r : anchor blocco§1 : rel blocco§2 : end ~ ^/",
        DiagnosticKind::UnknownAnchor,
        ErrorCode::Reference,
    )?;
    expect(
        "/^ ta r : end ~ ^/",
        DiagnosticKind::EndWithoutAnchor,
        ErrorCode::Scope,
    )?;
    expect(
        "/^ ta r : anchor a1 ^/",
        DiagnosticKind::UnclosedAnchor,
        ErrorCode::Scope,
    )?;
    expect(
        "/^ ta r ::: anchor a1 : end ~ ^/",
        DiagnosticKind::DspLevelSkip {
            max_allowed: 1,
            requested: 3,
        },
        ErrorCode::Scope,
    )?;
    // R2: the `:` of `end` are checked against the stack before the pop.
    expect(
        "/^ ta r : anchor a1 ::: end ~ ^/",
        DiagnosticKind::DspLevelSkip {
            max_allowed: 1,
            requested: 3,
        },
        ErrorCode::Scope,
    )?;
    // R3: the anchor stack is local to the period.
    expect(
        "/^ ta r : anchor a1 : end ~ ^/ /^ ta s : rel a1 ^/",
        DiagnosticKind::UnknownAnchor,
        ErrorCode::Reference,
    )
}

// ═══════════════════════════ Glyph anaphora (Q18, R6, R7, R9, R10) ═══════════════════════════

#[test]
fn anaphora_visibility_r6() -> TestResult {
    // R6.1: `~` does not see its own block.
    expect(
        "/^ ta x nu a au ~ ^/",
        DiagnosticKind::UnresolvedGlyphAnaphora,
        ErrorCode::Reference,
    )?;
    // R6.2: preceding roots, most recent first.
    let (s, id) = one("/^ ta x nu a y ta z nu b y ta w au ~ ^/")?;
    check_pair_value(&s, id, 2, 1, "b")?;
    // Pruned branches are invisible: of the root `x`, which has no case pairs, only
    // the signal pair remains.
    let (s, id) = one("/^ ta x : ta y nu b sed ta z au ~ ^/")?;
    check_pair_value(&s, id, 2, 1, "x")
}

#[test]
fn soft_close_carries_level_zero_r6() -> TestResult {
    let (s, id) = one("/^ ta x nu a /^/ /^ ta z au ~ ^/")?;
    check_pair_value(&s, id, 0, 1, "a")?;
    check(
        "period of the antecedent",
        match pair(&s, id, 0, 1)?.resolution() {
            Resolution::GlyphAnaphora { antecedent, .. } => Some(antecedent.clause.period.index()),
            _ => None,
        },
        Some(0),
    )?;
    // Accumulation across multiple `/^/`, most recent first.
    let (s, id) = one("/^ ta x nu a /^/ /^ ta y nu b /^/ /^ ta z au ~ ^/")?;
    check_pair_value(&s, id, 0, 1, "b")?;
    let (s, id) = one("/^ ta x au video§rosso /^/ /^ ta y nu b /^/ /^ ta z au video~ ^/")?;
    check_pair_value(&s, id, 0, 1, "video§rosso")?;
    // Also across successive ingests.
    let mut session = Session::new();
    accept(&mut session, "/^ ta x nu a /^/")?;
    check(
        "carried antecedents",
        session.carried_antecedents().count(),
        1,
    )?;
    let ids = accept(&mut session, "/^ ta z au ~ ^/")?;
    let id = ids.first().copied().ok_or("no period")?;
    check_pair_value(&session, id, 0, 1, "a")?;
    check("cleared by ^/", session.carried_antecedents().count(), 0)
}

#[test]
fn standard_and_hard_close_clear_anaphora() -> TestResult {
    expect(
        "/^ ta x nu a ^/ /^ ta z au ~ ^/",
        DiagnosticKind::UnresolvedGlyphAnaphora,
        ErrorCode::Reference,
    )?;
    expect(
        "/^ ta x nu a ^^/ /^ ta z au ~ ^/",
        DiagnosticKind::UnresolvedGlyphAnaphora,
        ErrorCode::Reference,
    )
}

#[test]
fn prefix_and_inverse_match_r7() -> TestResult {
    expect(
        "/^ ta r nu sonda au video : ta s au video~ ^/",
        DiagnosticKind::UnresolvedGlyphAnaphora,
        ErrorCode::Reference,
    )?;
    let (s, id) = one("/^ ta r au video§rosso : ta s nu io :: ta t au video~ ^/")?;
    check_pair_value(&s, id, 2, 1, "video§rosso")?;
    let (s, id) = one("/^ ta r nu video au video§rosso : ta s au video§rosso§scuro~ ^/")?;
    check_pair_value(&s, id, 1, 1, "video§rosso")?;
    let (s, id) = one("/^ ta r au video : ta s au video§rosso~ ^/")?;
    check_pair_value(&s, id, 1, 1, "video")?;
    expect(
        "/^ ta r au video§rosso : ta s au video§rosso~ ^/",
        DiagnosticKind::UnresolvedGlyphAnaphora,
        ErrorCode::Reference,
    )
}

#[test]
fn candidate_rules_r9() -> TestResult {
    // R9.2: pairs with a sema anaphora are skipped; the search continues with the
    // preceding canonical pair of the same block (here the signal pair `x`).
    let (s, id) = one("/^ ta p nu q : ta x nu a au ~~ :: ta y n ~ ^/")?;
    check_pair_value(&s, id, 2, 1, "a")?;
    let (s, id) = one("/^ ta p nu q : ta x au ~~ :: ta y n ~ ^/")?;
    check_pair_value(&s, id, 2, 1, "x")?;
    // R9.3: a raw string is valid only for Exact.
    let (s, id) = one("/^ ta x au \"Raw\" : ta y nu ~ ^/")?;
    check_pair_value(&s, id, 1, 1, "\"Raw\"")?;
    expect(
        "/^ ta x au \"video\" : ta y au video~ ^/",
        DiagnosticKind::UnresolvedGlyphAnaphora,
        ErrorCode::Reference,
    )?;
    // R9.4: a live variable as antecedent, with its number.
    let (s, id) = one("/^ defu dev a sensore§ottico y ta x nu dev : ta y n ~ ^/")?;
    check_pair_value(&s, id, 2, 1, "$dev")?;
    check(
        "variable number",
        pair(&s, id, 2, 1)?.number(),
        QuantumNumber::Singular,
    )?;
    let (s, id) = one("/^ defu dev a sensore§ottico y ta x au dev : ta y au sensore~ ^/")?;
    check_pair_value(&s, id, 2, 1, "$dev")
}

#[test]
fn numbers_and_signal_pair_q18_r10() -> TestResult {
    // The root has no case pairs: the only candidate is the signal pair, whose
    // number is not written, so it is the Dual.
    let (s, id) = one("/^ ta x : ta y n ~ ^/")?;
    check_pair_value(&s, id, 1, 1, "x")?;
    check(
        "omitted number → Dual",
        pair(&s, id, 1, 1)?.number(),
        QuantumNumber::Dual,
    )?;
    let (s, id) = one("/^ ta x : ta y nm ~ ^/")?;
    check(
        "explicit wins",
        pair(&s, id, 1, 1)?.number(),
        QuantumNumber::Plural,
    )?;
    let (s, id) = one("/^ ta x am a : ta y n ~ ^/")?;
    check(
        "inherited",
        pair(&s, id, 1, 1)?.number(),
        QuantumNumber::Plural,
    )?;
    let (s, id) = one("/^ ta x am a : ta y nu ~ ^/")?;
    check(
        "explicit",
        pair(&s, id, 1, 1)?.number(),
        QuantumNumber::Singular,
    )?;
    // Every pair has an effective number: without anaphora the signal pair is Dual.
    let (s, id) = one("/^ ta vedere nu io ^/")?;
    check(
        "signal pair",
        pair(&s, id, 0, 0)?.number(),
        QuantumNumber::Dual,
    )?;
    // Same antecedent rule in every pair: `~` in the signal pair binds to the last
    // canonical pair of the antecedent (`nu io`). Rule 1B: the signal pair never
    // inherits a number, it is Dual unless the signal tile writes one.
    let (s, id) = one("/^ ta vedere nu io : ta ~ nu marco ^/")?;
    let signal_pair = pair(&s, id, 1, 0)?;
    check("value", value_text(&s, signal_pair.value())?.as_str(), "io")?;
    check(
        "signal pair stays Dual",
        signal_pair.number(),
        QuantumNumber::Dual,
    )?;
    let (s, id) = one("/^ ta vedere nu io : tam ~ nu marco ^/")?;
    check(
        "number written in the signal tile",
        pair(&s, id, 1, 0)?.number(),
        QuantumNumber::Plural,
    )?;
    // A variable in the signal pair: Dual as well; in a case pair: its own number.
    let (s, id) = one("/^ defm gruppo a utenti y ta gruppo a gruppo ^/")?;
    check(
        "variable in the signal pair",
        pair(&s, id, 1, 0)?.number(),
        QuantumNumber::Dual,
    )?;
    check(
        "variable in a case pair",
        pair(&s, id, 1, 1)?.number(),
        QuantumNumber::Plural,
    )?;
    let (s, id) = one("/^ defm gruppo a utenti y ta x au gruppo ^/")?;
    check(
        "explicit prevails on the variable",
        pair(&s, id, 1, 1)?.number(),
        QuantumNumber::Singular,
    )?;
    // The anaphora inherits the effective number of the antecedent pair.
    let (s, id) = one("/^ defm gruppo a utenti y ta x a gruppo : ta y a ~ ^/")?;
    check(
        "anaphora on a variable pair",
        pair(&s, id, 2, 1)?.number(),
        QuantumNumber::Plural,
    )
}

#[test]
fn signal_tile_number_rule_1b() -> TestResult {
    for src in [
        "/^ tau x ^/",
        "/^ tam x ^/",
        "/^ tanum x ^/",
        "/^ deepallu x a y ^/",
        "/^ tachazatou x a y ^/",
        "/^ ta r : quatazatom x ^/",
    ] {
        let (s, id) = one(src)?;
        let period = s.period(id).ok_or("no period")?;
        check(
            "round trip",
            crate::printer::render_period(period, s.interner()).map_err(|e| e.to_string())?,
            src.to_owned(),
        )?;
    }
    let (s, id) = one("/^ deepallu x a y ^/")?;
    check(
        "deepallu",
        pair(&s, id, 0, 0)?.number(),
        QuantumNumber::Singular,
    )?;
    let (s, id) = one("/^ tanum x ^/")?;
    check(
        "tanum",
        pair(&s, id, 0, 0)?.number(),
        QuantumNumber::NonDual,
    )?;
    Ok(())
}

#[test]
fn anaphora_rules_are_identical_in_every_pair() -> TestResult {
    // `~` in the signal pair and in a case pair: same antecedent.
    let (s, id) = one("/^ ta x nu a : ta ~ nu ~ ^/")?;
    check(
        "same antecedent",
        pair(&s, id, 1, 0)?.resolution(),
        pair(&s, id, 1, 1)?.resolution(),
    )?;
    check_pair_value(&s, id, 1, 0, "a")?;
    // The glyph of the signal pair is a candidate like any other.
    let (s, id) = one("/^ ta video§rosso nu io : ta s au video~ ^/")?;
    check_pair_value(&s, id, 1, 1, "video§rosso")?;
    // A label in the signal pair is a valid antecedent for `~` (R9.3).
    let (s, id) = one("/^ ta \"Xcvb124\" : ta y au ~ ^/")?;
    check_pair_value(&s, id, 1, 1, "\"Xcvb124\"")?;
    // `glyph~~` finds the glyph in any pair of the antecedent block.
    let (s, id) = one("/^ ta x nu canis : ta y :: ta z au canis~~ ^/")?;
    check_pair_value(&s, id, 2, 1, "~~0.0")
}

// ═══════════════════════════ Sema anaphora (R8) ═══════════════════════════

#[test]
fn sema_anaphora_r8() -> TestResult {
    let (s, id) = one("/^ ta x nu a : tozato ~~ ^/")?;
    check(
        "~~ parent",
        value_text(&s, pair(&s, id, 1, 0)?.value())?.as_str(),
        "~~0.0",
    )?;
    let (s, id) = one("/^ ta x nu a y ta z au ~~ ^/")?;
    check_pair_value(&s, id, 1, 1, "~~0.0")?;
    expect(
        "/^ ta x au ~~ ^/",
        DiagnosticKind::UnresolvedSemaAnaphora,
        ErrorCode::Reference,
    )?;
    let (s, id) = one("/^ ta vedere nu io : ta abbaiare nu ~ :: ta vedere~~ ^/")?;
    check(
        "by glyph",
        value_text(&s, pair(&s, id, 2, 0)?.value())?.as_str(),
        "~~0.0",
    )?;
    // The comparison uses resolved values: `nu ~` → canis makes `canis~~` bindable
    // to block 1, the most recent one containing it.
    let (s, id) = one("/^ ta x au canis : ta y nu ~ :: ta k au canis~~ ^/")?;
    check_pair_value(&s, id, 2, 1, "~~0.1")?;
    expect(
        "/^ ta x : ta y nu zzz~~ ^/",
        DiagnosticKind::UnresolvedSemaAnaphora,
        ErrorCode::Reference,
    )
}

// ═══════════════════════════ def variables (R11, R13, R14) ═══════════════════════════

#[test]
fn def_lifecycle_example_3() -> TestResult {
    let (s, id) = one(
        "/^ defu dev a sensore§ottico y defu dev abfi sensore§termico y ta leggere nu dev y defu dev abse ~ ^/",
    )?;
    let slot = pair(&s, id, 2, 1)?;
    let binding_id = match slot.resolution() {
        Resolution::Variable { binding } => binding,
        other => return Err(format!("expected variable, found {other:?}")),
    };
    let binding = s.binding(binding_id).ok_or("binding missing")?;
    check("values", binding.values().len(), 1)?;
    let value = binding.values().first().copied().ok_or("value missing")?;
    check(
        "current value",
        value_text(&s, value)?.as_str(),
        "sensore§termico",
    )?;
    check("number", binding.quantum_number(), QuantumNumber::Singular)?;
    check("deleted", s.live_variable("dev").is_none(), true)?;
    check(
        "abse without value (pair 1 of the def block)",
        clause(&s, id, 3)?.slots().get(1).copied(),
        Some(None),
    )
}

#[test]
fn def_union_and_cross_period_use() -> TestResult {
    let (s, _) = one("/^ defu dev a x y defu dev absi z y ta k nu dev ^/")?;
    let (_, binding) = s.live_variable("dev").ok_or("variable missing")?;
    let texts = binding
        .values()
        .iter()
        .map(|v| value_text(&s, *v))
        .collect::<Result<Vec<String>, String>>()?;
    check("merged values", texts, vec!["x".to_owned(), "z".to_owned()])?;

    let mut session = Session::new();
    accept(&mut session, "/^ defu soglia a 30§celsius ^/")?;
    let ids = accept(
        &mut session,
        "/^ tachazato temperatura nu sensore a soglia ^/",
    )?;
    let id = ids.first().copied().ok_or("no period")?;
    check_pair_value(&session, id, 0, 2, "$soglia")
}

#[test]
fn def_errors_r11() -> TestResult {
    expect(
        "/^ defu dev abfi x ^/",
        DiagnosticKind::UndefinedVariable,
        ErrorCode::Reference,
    )?;
    expect(
        "/^ defu dev absi x ^/",
        DiagnosticKind::UndefinedVariable,
        ErrorCode::Reference,
    )?;
    expect(
        "/^ defu dev abse ~ ^/",
        DiagnosticKind::UndefinedVariable,
        ErrorCode::Reference,
    )?;
    expect(
        "/^ defu dev a x y defu dev a z ^/",
        DiagnosticKind::AssignToLiveVariable,
        ErrorCode::Semantic,
    )?;
    expect(
        "/^ defu dev a x y defm dev abfi z ^/",
        DiagnosticKind::VariableNumberMismatch,
        ErrorCode::Semantic,
    )?;
    expect(
        "/^ defu dev a x y def dev abse ~ ^/",
        DiagnosticKind::VariableNumberMismatch,
        ErrorCode::Semantic,
    )?;
    expect(
        "/^ defu x am y ^/",
        DiagnosticKind::VariableNumberMismatch,
        ErrorCode::Semantic,
    )?;
    expect(
        "/^ def x au y ^/",
        DiagnosticKind::VariableNumberMismatch,
        ErrorCode::Semantic,
    )?;
    one("/^ def x a y ^/")?;
    one("/^ defu x au y ^/")?;
    // After `abse ~` the name becomes a literal glyph again.
    let (s, id) = one("/^ defu dev a x y defu dev abse ~ y ta k nu dev ^/")?;
    check(
        "literal",
        pair(&s, id, 2, 1)?.resolution(),
        Resolution::Literal,
    )
}

#[test]
fn variable_name_resolution_r13() -> TestResult {
    let (s, id) = one("/^ defu v a x y ta v nu q ^/")?;
    check(
        "variable in the signal pair",
        matches!(
            pair(&s, id, 1, 0)?.resolution(),
            Resolution::Variable { .. }
        ),
        true,
    )?;
    let (s, id) = one("/^ defu v a x y ta v§1 nu q ^/")?;
    check(
        "qualified",
        pair(&s, id, 1, 0)?.resolution(),
        Resolution::Literal,
    )?;
    // `def` values are not resolved as variables (R13: relational blocks only).
    let (s, id) = one("/^ defu v a x y defu w a v ^/")?;
    let value = pair(&s, id, 1, 1)?;
    check("def value", value.resolution(), Resolution::Literal)?;
    let (s, id) = one("/^ defu v a x ^^/ /^ ta k nu v ^/")?;
    check(
        "after ^^/",
        pair(&s, id, 0, 1)?.resolution(),
        Resolution::Literal,
    )?;
    check("not live", s.live_variable("v").is_none(), true)
}

#[test]
fn unbind_tilde_is_not_anaphora_r14() -> TestResult {
    let mut s = Session::new();
    accept(&mut s, "/^ defu x a y ^/")?;
    accept(&mut s, "/^ defu x abse ~ ^/")?;
    check("deleted", s.live_variable("x").is_none(), true)
}

// ═══════════════════════════ 3-level GC and R12 ═══════════════════════════

#[test]
fn garbage_collector_variables() -> TestResult {
    let mut s = Session::new();
    accept(&mut s, "/^ defu x a y /^/")?;
    check("after /^/", s.live_variable("x").is_some(), true)?;
    accept(&mut s, "/^ ta k nu x ^/")?;
    check("after ^/", s.live_variable("x").is_some(), true)?;
    accept(&mut s, "/^ ta k nu x ^^/")?;
    check("after ^^/", s.live_variable("x").is_none(), true)?;
    expect_in(
        &mut s,
        "/^ defu x abfi z ^/",
        DiagnosticKind::UndefinedVariable,
        ErrorCode::Reference,
    )?;
    expect(
        "/^ defu x a y ^^/ /^ defu x abfi z ^/",
        DiagnosticKind::UndefinedVariable,
        ErrorCode::Reference,
    )
}

#[test]
fn hard_close_removes_sync_antecedent_r12() -> TestResult {
    expect(
        "/^ ta x ^^/ /^≈ ta y ^/",
        DiagnosticKind::SynchronizedOpenWithoutAntecedent,
        ErrorCode::Scope,
    )?;
    let mut s = Session::new();
    accept(&mut s, "/^ ta x ^^/")?;
    check("antecedent", s.has_sync_antecedent(), false)?;
    expect_in(
        &mut s,
        "/^≃ ta y ^/",
        DiagnosticKind::SynchronizedOpenWithoutAntecedent,
        ErrorCode::Scope,
    )?;
    accept(&mut s, "/^ ta x /^/ /^≈ ta y ^/ /^≃ ta z ^/")?;
    check("antecedent restored", s.has_sync_antecedent(), true)
}

// ═══════════════════════════ Atomicity and diagnostics ═══════════════════════════

#[test]
fn stage_two_failure_leaves_session_untouched() -> TestResult {
    let mut s = Session::new();
    accept(&mut s, "/^ defu keep a valore ^/ /^ ta x nu a /^/")?;
    let periods = s.periods().len();
    let symbols = s.interner().len();
    let carried = s.carried_antecedents().count();
    reject_in(
        &mut s,
        "/^ defu nuova a altro y defu keep abse ~ ^/ /^ ta y au ~ ^/",
    )?;
    check("periods", s.periods().len(), periods)?;
    check("symbols", s.interner().len(), symbols)?;
    check("antecedents", s.carried_antecedents().count(), carried)?;
    check(
        "variable nuova missing",
        s.live_variable("nuova").is_none(),
        true,
    )?;
    check(
        "variable keep intact",
        s.live_variable("keep").is_some(),
        true,
    )?;
    check("synchronization", s.has_sync_antecedent(), true)
}

#[test]
fn stage_two_collects_sorted_diagnostics() -> TestResult {
    let diagnostics = reject_in(&mut Session::new(), "/^ ta x au ~ : rel zz ^/")?;
    let kinds: Vec<DiagnosticKind> = diagnostics.iter().map(|d| d.kind).collect();
    check(
        "diagnostics",
        kinds,
        vec![
            DiagnosticKind::UnresolvedGlyphAnaphora,
            DiagnosticKind::UnknownAnchor,
        ],
    )?;
    let sorted = diagnostics.windows(2).all(|w| match w {
        [a, b] => a.span.start <= b.span.start,
        _ => true,
    });
    check("sorted", sorted, true)
}

#[test]
fn sema_anaphora_resolves_identically_in_every_slot() -> TestResult {
    // `~~` in the signal glyph slot and in a case slot: the same antecedent, no
    // pair of the recalled sema is merged into the block.
    let (s, id) = one("/^ ta x nu a : tozato ~~ nu ~~ ^/")?;
    check(
        "same resolution",
        pair(&s, id, 1, 0)?.resolution(),
        pair(&s, id, 1, 1)?.resolution(),
    )?;
    // Each block is validated on its own written pairs only.
    one("/^ ta x nu a : tozato ~~ nu b ^/")?;
    one("/^ ta x a y : taref ~~ ^/")?;
    one("/^ ta r : tazato x abdur ~ :: ta ~~ ^/")?;
    Ok(())
}
