//! B2/B3 tests: `abdur=<N>` grammar, loop validation, execution plan, set resolution.

use crate::analyze::ClauseId;
use crate::dti::{Dti, DtiKey};
use crate::error::{DiagnosticKind, ErrorCode, Limit};
use crate::parser::ParserConfig;
use crate::printer::render_period;
use crate::session::{PeriodId, Session};

use super::{build_execution_plan, ExecutionPlan, PlanError, QuantifierMode};

type TestResult = Result<(), String>;

fn key(tag: &str) -> Result<DtiKey, String> {
    let mut body = [b'0'; 31];
    for (slot, byte) in body.iter_mut().zip(tag.bytes()) {
        *slot = byte;
    }
    DtiKey::from_random_body(body).map_err(|e| e.to_string())
}

fn k(tag: &str) -> Result<String, String> {
    key(tag).map(|k| k.as_str().to_owned())
}

fn check<T: PartialEq + core::fmt::Debug>(what: &str, got: T, want: T) -> TestResult {
    if got == want {
        Ok(())
    } else {
        Err(format!("{what}: got {got:?}, want {want:?}"))
    }
}

fn ingest(session: &mut Session, src: &str) -> Result<PeriodId, String> {
    let ids = session
        .ingest(src)
        .map_err(|d| format!("rejected {src:?}: {d:#?}"))?;
    ids.last().copied().ok_or_else(|| "no period".to_owned())
}

fn expect(src: &str, kind: DiagnosticKind, code: ErrorCode) -> TestResult {
    match Session::new().ingest(src) {
        Ok(_) => Err(format!("accepted, expected {kind:?}")),
        Err(d) if d.iter().any(|x| x.kind == kind && x.code() == code) => Ok(()),
        Err(d) => Err(format!("{src:?}: expected {kind:?}, got {d:#?}")),
    }
}

/// Small taxonomy: sensore → sensore§ottico → sensore§ottico§ir, sensore → sensore§termico.
/// DTI forms are atoms (§16.3): the compound concepts are records with `base:` and
/// `param:`, reached from the segmented operands `<K1>§<K2>` (§14).
fn dti() -> Result<Dti, String> {
    let universal = format!(
        "#awdti 1\n#role universal\n#version 3\n#lang en\n\
rec {sensor} sensor morph:+UNION\n\
rec {opt} optic morph:+UNION\n\
rec {infra} infrared morph:+UNION\n\
rec {therm} thermic morph:+UNION\n\
rec {optical} opticalsensor base:{sensor} param:{opt}\n\
rec {ir} irsensor base:{optical} param:{infra}\n\
rec {thermal} thermalsensor base:{sensor} param:{therm}\n\
rec {system} system\n\
rec {activate} activate\n\
rec {x} x\n",
        sensor = k("sensor")?,
        opt = k("opt")?,
        infra = k("infra")?,
        therm = k("therm")?,
        optical = k("optical")?,
        ir = k("ir")?,
        thermal = k("thermal")?,
        system = k("system")?,
        activate = k("activate")?,
        x = k("x")?,
    );
    let local = format!(
        "#awdti 1\n#role local\n#universal 3\n#lang it\n\
use {} form:sensore\nuse {} form:ottico\nuse {} form:ir\nuse {} form:termico\n\
use {}\nuse {}\nuse {}\nuse {} form:sistema\nuse {} form:attivare\nuse {}\n",
        k("sensor")?,
        k("opt")?,
        k("infra")?,
        k("therm")?,
        k("optical")?,
        k("ir")?,
        k("thermal")?,
        k("system")?,
        k("activate")?,
        k("x")?,
    );
    Dti::load(&universal, &local).map_err(|d| format!("{d:#?}"))
}

fn plan_of(src: &str, dti: &Dti) -> Result<ExecutionPlan, String> {
    let mut session = Session::new();
    // Synchronized opens need an antecedent (Q23).
    ingest(&mut session, "/^ ta x ^/")?;
    let id = ingest(&mut session, src)?;
    build_execution_plan(&session, id, dti).map_err(|e| e.to_string())
}

fn first_targets(plan: &ExecutionPlan) -> Result<(QuantifierMode, Vec<DtiKey>), String> {
    let targets = plan
        .steps
        .first()
        .and_then(|s| s.targets.as_ref())
        .ok_or("no targets")?;
    Ok((targets.mode, targets.keys.to_vec()))
}

fn sorted(keys: &[&str]) -> Result<Vec<DtiKey>, String> {
    let mut out = keys.iter().map(|t| key(t)).collect::<Result<Vec<_>, _>>()?;
    out.sort();
    Ok(out)
}

// ═══════════════════════════ `abdur=<N>` grammar (L2, L5) ═══════════════════════════

#[test]
fn abdur_tiles_round_trip() -> TestResult {
    for src in [
        "/^ ta x abdur=5 ~ ^/",
        "/^ ta x abduru=12 ~ ^/",
        "/^ ta x abdurm=3 y ^/",
        "/^ ta x abdurnum=1 ~ ^/",
        "/^ ta x abdu notte ^/",
        "/^ ta x : tazato c abdur ~ ^/",
        "/^ ta x : tachuzato c a 100 abduru ~ ^/",
    ] {
        let mut session = Session::new();
        let id = ingest(&mut session, src)?;
        let period = session.period(id).ok_or("no period")?;
        check(
            "round trip",
            render_period(period, session.interner()).map_err(|e| e.to_string())?,
            src.to_owned(),
        )?;
    }
    Ok(())
}

#[test]
fn abdur_validation() -> TestResult {
    expect(
        "/^ ta x abdur ~ ^/",
        DiagnosticKind::LoopMarkerOutsideConditional,
        ErrorCode::Scope,
    )?;
    expect(
        "/^ ta x : ta c abdur ~ ^/",
        DiagnosticKind::LoopMarkerOutsideConditional,
        ErrorCode::Scope,
    )?;
    expect(
        "/^ ta x abdur=1001 ~ ^/",
        DiagnosticKind::LimitExceeded {
            limit: Limit::LoopIterations,
        },
        ErrorCode::Scope,
    )?;
    expect(
        "/^ ta x abdur=0 ~ ^/",
        DiagnosticKind::UnknownCaseTile,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x abdur=05 ~ ^/",
        DiagnosticKind::UnknownCaseTile,
        ErrorCode::Syntax,
    )?;
    expect(
        "/^ ta x abdu=5 ~ ^/",
        DiagnosticKind::UnknownCaseTile,
        ErrorCode::Syntax,
    )?;
    // v4.1: several loops in one block are allowed (nested in written order).
    ingest(&mut Session::new(), "/^ ta x abdur=3 ~ abdur=5 ~ ^/")?;
    // v4.1: the bare marker accepts an inherited conditional (I13).
    ingest(&mut Session::new(), "/^ tazato x nu y : ta c abdur ~ ^/")?;
    // An explicit indicative override removes the conditional.
    expect(
        "/^ tazato x nu y : taato c abdur ~ ^/",
        DiagnosticKind::LoopMarkerOutsideConditional,
        ErrorCode::Scope,
    )?;
    expect(
        "/^ defu v abdur=3 x ^/",
        DiagnosticKind::DefOperationNotAllowed,
        ErrorCode::Syntax,
    )?;
    // `~` after `abdur` is a marker: no antecedent needed even in the head.
    let mut session = Session::new();
    ingest(&mut session, "/^ ta x abdur=2 ~ ^/")?;
    Ok(())
}

// ═══════════════════════════ Execution plan (L1, L3, L5) ═══════════════════════════

fn shape(plan: &ExecutionPlan) -> Vec<(u32, Vec<u32>)> {
    plan.steps
        .iter()
        .map(|s| (s.clause.clause.index(), s.iteration.to_vec()))
        .collect()
}

#[test]
fn loops_repeat_clause_and_subtree() -> TestResult {
    let dti = dti()?;
    let plan = plan_of("/^ ta x abdur=3 ~ : ta y ^/", &dti)?;
    check(
        "simple loop",
        shape(&plan),
        vec![
            (0, vec![0]),
            (1, vec![0]),
            (0, vec![1]),
            (1, vec![1]),
            (0, vec![2]),
            (1, vec![2]),
        ],
    )?;
    let plan = plan_nested(&dti)?;
    check(
        "nested loops",
        shape(&plan),
        vec![
            (0, vec![0]),
            (1, vec![0, 0]),
            (1, vec![0, 1]),
            (0, vec![1]),
            (1, vec![1, 0]),
            (1, vec![1, 1]),
        ],
    )?;
    // Loop-control subordinate (bare marker) emitted once, before the body.
    let plan = plan_of("/^ ta x abdur=2 ~ : tazato c abdur ~ ^/", &dti)?;
    check(
        "control",
        shape(&plan),
        vec![(1, vec![]), (0, vec![0]), (0, vec![1])],
    )?;
    // v4.1: a control subordinate with an inherited conditional.
    let plan = plan_of("/^ tazato x abdur=2 ~ : ta c abdur ~ ^/", &dti)?;
    check(
        "inherited control",
        shape(&plan),
        vec![(1, vec![]), (0, vec![0]), (0, vec![1])],
    )?;
    // v4.1: two loops in one clause nest in written order (N1 × N2).
    let plan = plan_of("/^ ta x abdur=2 ~ abdur=3 ~ : ta y ^/", &dti)?;
    check(
        "nested loops of one clause",
        shape(&plan),
        vec![
            (0, vec![0, 0]),
            (1, vec![0, 0]),
            (0, vec![0, 1]),
            (1, vec![0, 1]),
            (0, vec![0, 2]),
            (1, vec![0, 2]),
            (0, vec![1, 0]),
            (1, vec![1, 0]),
            (0, vec![1, 1]),
            (1, vec![1, 1]),
            (0, vec![1, 2]),
            (1, vec![1, 2]),
        ],
    )?;
    // No loops: plain pre-order, def and portability blocks excluded.
    let plan = plan_of("/^ defu v a x y ta a : ta b : anchor p : end ~ ^/", &dti)?;
    check("no loops", shape(&plan), vec![(1, vec![]), (2, vec![])])
}

fn plan_nested(dti: &Dti) -> Result<ExecutionPlan, String> {
    plan_of("/^ ta x abdur=2 ~ : ta y abdur=2 ~ ^/", dti)
}

#[test]
fn loop_limit_on_generated_steps_l5() -> TestResult {
    let dti = dti()?;
    let config = ParserConfig {
        max_loop_iterations: core::num::NonZeroU32::new(10).ok_or("zero")?,
        ..ParserConfig::default()
    };
    let mut session = Session::with_config(config);
    let id = ingest(&mut session, "/^ ta x abdur=4 ~ : ta y abdur=4 ~ ^/")?;
    check(
        "nested product beyond the limit",
        build_execution_plan(&session, id, &dti),
        Err(PlanError::LoopIterations),
    )?;
    let id = ingest(&mut session, "/^ ta x abdur=3 ~ : ta y ^/")?;
    check(
        "within the limit",
        build_execution_plan(&session, id, &dti).map(|p| p.steps.len()),
        Ok(6),
    )
}

// ═══════════════════════════ Sets (S1–S3) ═══════════════════════════

#[test]
fn canonical_set_examples_s3() -> TestResult {
    let dti = dti()?;
    check(
        "all minus abse",
        first_targets(&plan_of(
            "/^≈ deepall attivare nu sistema am sensore abse sensore§ottico ^/",
            &dti,
        )?)?,
        (QuantifierMode::All, sorted(&["sensor", "thermal"])?),
    )?;
    check(
        "all filtered by abpa",
        first_targets(&plan_of(
            "/^≈ deepall attivare nu sistema am sensore abpa sensore§ottico ^/",
            &dti,
        )?)?,
        (QuantifierMode::All, sorted(&["optical", "ir"])?),
    )?;
    check(
        "cuo subtracts absi operands",
        first_targets(&plan_of(
            "/^≈ deepallcuo attivare nu sistema a sensore absi sensore§ottico ^/",
            &dti,
        )?)?,
        (QuantifierMode::All, sorted(&["sensor", "thermal"])?),
    )?;
    check(
        "cup unites absi operands",
        first_targets(&plan_of(
            "/^≈ deepallcup attivare nu sistema a sensore§ottico absi sensore§termico ^/",
            &dti,
        )?)?,
        (QuantifierMode::All, sorted(&["optical", "ir", "thermal"])?),
    )
}

#[test]
fn nominative_is_the_base_without_accusative() -> TestResult {
    let dti = dti()?;
    check(
        "nominative base minus abse",
        first_targets(&plan_of(
            "/^≈ deepall attivare nm sensore abse sensore§ottico ^/",
            &dti,
        )?)?,
        (QuantifierMode::All, sorted(&["sensor", "thermal"])?),
    )?;
    // With an Accusative present, the Nominative is not an operand.
    check(
        "accusative prevails",
        first_targets(&plan_of(
            "/^≈ deepall attivare nm sistema am sensore§ottico ^/",
            &dti,
        )?)?,
        (QuantifierMode::All, sorted(&["optical", "ir"])?),
    )
}

#[test]
fn quantifier_modes_and_variables_s2() -> TestResult {
    let dti = dti()?;
    check(
        "some",
        first_targets(&plan_of(
            "/^ tasome attivare nu sistema a sensore§ottico ^/",
            &dti,
        )?)?,
        (QuantifierMode::AtLeastOne, sorted(&["optical", "ir"])?),
    )?;
    // A `def` variable with several values contributes all of them.
    let mut session = Session::new();
    let id = ingest(
        &mut session,
        "/^ def grp a sensore§ottico y def grp absi sensore§termico y tacup attivare nu sistema a grp ^/",
    )?;
    let plan = build_execution_plan(&session, id, &dti).map_err(|e| e.to_string())?;
    check(
        "variable operands",
        first_targets(&plan)?,
        (QuantifierMode::All, sorted(&["optical", "thermal"])?),
    )?;
    // No quantifier and no set comparative: no targets.
    let plan = plan_plain(&dti)?;
    check(
        "no set semantics",
        plan.steps.first().map(|s| s.targets.is_none()),
        Some(true),
    )?;
    let mut session = Session::new();
    let id = ingest(&mut session, "/^ taall attivare nu sistema a blu ^/")?;
    match build_execution_plan(&session, id, &dti) {
        Err(PlanError::Resolve { clause, .. }) if clause == ClauseId::HEAD => Ok(()),
        other => Err(format!("expected Resolve error, got {other:?}")),
    }
}

fn plan_plain(dti: &Dti) -> Result<ExecutionPlan, String> {
    plan_of("/^ ta attivare nu sistema a sensore ^/", dti)
}

#[test]
fn segmented_operands_need_exactly_one_compound_record() -> TestResult {
    let dti = dti()?;
    // `sensore§ottico§ir` folds from the left: (sensor + optic) + infrared.
    check(
        "chain",
        first_targets(&plan_of(
            "/^≈ deepall attivare nu sistema am sensore§ottico§ir ^/",
            &dti,
        )?)?,
        (QuantifierMode::All, sorted(&["ir"])?),
    )?;
    // No enabled record with base:sensor and param:infrared.
    let mut session = Session::new();
    let id = ingest(
        &mut session,
        "/^ deepall attivare nu sistema am sensore§ir ^/",
    )?;
    check(
        "missing compound",
        build_execution_plan(&session, id, &dti).map(|_| ()),
        Err(PlanError::Composite {
            clause: ClauseId::new(0),
            base: key("sensor")?,
            param: key("infra")?,
            found: Vec::new(),
        }),
    )
}
