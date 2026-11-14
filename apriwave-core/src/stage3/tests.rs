//! Phase C tests: expanded rendering, indexed wire form, indexed input, views.

use crate::dti::{Dti, DtiKey, KeyResolution};
use crate::session::{PeriodId, Session};

use super::{
    render_expanded_period, render_indexed_period, render_view, validate_indexed_input,
    ExpandError, IndexError, IndexedInputError, Layout, MorphFlag, ResolveError, ViewDisplayMode,
};

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

fn one(src: &str) -> Result<(Session, PeriodId), String> {
    let mut session = Session::new();
    let ids = session
        .ingest(src)
        .map_err(|d| format!("rejected {src:?}: {d:#?}"))?;
    let id = ids.last().copied().ok_or("no period")?;
    Ok((session, id))
}

fn check<T: PartialEq + core::fmt::Debug>(what: &str, got: T, want: T) -> TestResult {
    if got == want {
        Ok(())
    } else {
        Err(format!("{what}: got {got:?}, want {want:?}"))
    }
}

fn expanded(src: &str) -> Result<String, String> {
    let (session, id) = one(src)?;
    render_expanded_period(&session, id).map_err(|e| e.to_string())
}

fn expand_error(src: &str) -> Result<ExpandError, String> {
    let (session, id) = one(src)?;
    match render_expanded_period(&session, id) {
        Ok(text) => Err(format!("expanded unexpectedly: {text}")),
        Err(err) => Ok(err),
    }
}

/// The expanded output must be valid ApriWave and expand to itself.
fn idempotent(src: &str) -> TestResult {
    let first = expanded(src)?;
    let second = expanded(&first)?;
    check("idempotent expansion", second, first)
}

// ═══════════════════════════ Expanded rendering (C.1, D8) ═══════════════════════════

#[test]
fn expands_manual_example_5_with_effective_numbers() -> TestResult {
    let src = "/^ ta vedere nu io au humanum : tazato venire n ~ sed to vedere nu io au humanum : tozato venire n ~ ^/";
    check(
        "example 5",
        expanded(src)?,
        "/^ ta vedere nu io au humanum : tazato venire nu humanum sed to vedere nu io au humanum : tozato venire nu humanum ^/".to_owned(),
    )?;
    idempotent(src)
}

#[test]
fn removes_def_blocks_and_substitutes_variables() -> TestResult {
    check(
        "example 2",
        expanded("/^ defu dev a sensore§ottico y ta attivare nu dev : ta leggere nu dev ^/")?,
        "/^ ta attivare nu sensore§ottico : ta leggere nu sensore§ottico ^/".to_owned(),
    )?;
    check(
        "example 3",
        expanded(
            "/^ defu dev a sensore§ottico y defu dev abfi sensore§termico y ta leggere nu dev y defu dev abse ~ ^/",
        )?,
        "/^ ta leggere nu sensore§termico ^/".to_owned(),
    )?;
    let mut session = Session::new();
    session
        .ingest("/^ defu soglia a 30§celsius ^/")
        .map_err(|d| format!("{d:#?}"))?;
    let ids = session
        .ingest("/^ tachazato temperatura nu sensore a soglia ^/")
        .map_err(|d| format!("{d:#?}"))?;
    let id = ids.first().copied().ok_or("no period")?;
    check(
        "variable across periods",
        render_expanded_period(&session, id).map_err(|e| e.to_string())?,
        // Rule 1B / Q18: the omitted number of the case pair is the variable's own.
        "/^ tachazato temperatura nu sensore au 30§celsius ^/".to_owned(),
    )
}

#[test]
fn case_pair_variable_inherits_its_number_rule_1b() -> TestResult {
    check(
        "omitted number inherited",
        expanded("/^ defm gruppo a utenti y ta avvisare a gruppo ^/")?,
        "/^ ta avvisare am utenti ^/".to_owned(),
    )?;
    check(
        "explicit number prevails",
        expanded("/^ defm gruppo a utenti y ta avvisare au gruppo ^/")?,
        "/^ ta avvisare au utenti ^/".to_owned(),
    )
}

#[test]
fn multi_valued_variable_in_case_pair_expands_with_absi() -> TestResult {
    let src = "/^ defu dev a x y defu dev absi z y ta k nu dev ^/";
    check(
        "absi expansion",
        expanded(src)?,
        "/^ ta k nu x absi z ^/".to_owned(),
    )?;
    idempotent(src)?;
    check(
        "three values, Dual variable",
        expanded("/^ def s a p y def s absi q y def s absi r y ta k a s ^/")?,
        "/^ ta k a p absi q absi r ^/".to_owned(),
    )
}

#[test]
fn renders_sema_anaphora_as_glyph_reference() -> TestResult {
    // `~~` in a case slot: the signal glyph of the antecedent.
    check(
        "latest in a case slot",
        expanded("/^ ta x nu a : ta b a ~~ ^/")?,
        "/^ ta x nu a : ta b a x~~ ^/".to_owned(),
    )?;
    check(
        "by glyph in a case slot",
        expanded("/^ ta vedere nu io : ta abbaiare nu ~ :: ta pensare a vedere~~ ^/")?,
        "/^ ta vedere nu io : ta abbaiare nu io :: ta pensare a vedere~~ ^/".to_owned(),
    )?;
    idempotent("/^ ta x nu a : ta b a ~~ ^/")
}

#[test]
fn sema_in_signal_slot_is_rendered_like_any_slot() -> TestResult {
    // Every even slot follows the same rule: `~~` → `<signal glyph>~~`.
    let src = "/^ ta x nu a : tozato ~~ ^/";
    check(
        "latest",
        expanded(src)?,
        "/^ ta x nu a : tozato x~~ ^/".to_owned(),
    )?;
    idempotent(src)?;
    check(
        "written glyph~~ stays as written",
        expanded("/^ ta vedere nu io : ta abbaiare nu canis :: tazato vedere~~ abfi ieri ^/")?,
        "/^ ta vedere nu io : ta abbaiare nu canis :: tazato vedere~~ abfi ieri ^/".to_owned(),
    )?;
    // The antecedent's signal glyph is itself a sema: first glyph path in
    // canonical order, `~~` when there is none.
    check(
        "sema of a sema",
        expanded("/^ ta x nu a : tozato ~~ :: tazato ~~ ^/")?,
        "/^ ta x nu a : tozato x~~ :: tazato ~~ ^/".to_owned(),
    )?;
    check(
        "case pairs are never copied",
        expanded("/^ ta x nu a : tozato ~~ nu b ^/")?,
        "/^ ta x nu a : tozato x~~ nu b ^/".to_owned(),
    )
}

#[test]
fn removes_portability_blocks_when_expressible() -> TestResult {
    let src = "/^ ta radice nu x : ta padre nu a :: anchorrel port1 : ta figlio nu b :: ta nipote nu c : reloff port1 : ta fratello§esterno nu d : end ~ ^/";
    check(
        "explicit colons",
        expanded(src)?,
        "/^ ta radice nu x : ta padre nu a :: ta figlio nu b ::: ta nipote nu c : ta fratello§esterno nu d ^/".to_owned(),
    )?;
    idempotent(src)?;
    // `y` at depth 0 right after a depth-2 clause needs the anchor: not expressible.
    check(
        "inexpressible depth",
        expand_error("/^ ta r : ta p : anchor a1 : ta q :: ta w : end ~ y ta s ^/")?,
        ExpandError::InexpressibleDepth {
            clause: crate::analyze::ClauseId::new(6),
        },
    )
}

#[test]
fn export_errors_instead_of_invalid_output_d8() -> TestResult {
    check(
        "multi-valued variable in the signal pair",
        expand_error("/^ defu dev a x y defu dev absi z y ta dev nu k ^/")?,
        ExpandError::MultiValuedVariable {
            clause: crate::analyze::ClauseId::new(2),
        },
    )?;
    // Rule 1B: the signal pair never inherits the Singular of `nu io` (Dual).
    check(
        "signal pair stays Dual",
        expanded("/^ ta vedere nu io : ta ~ nu marco ^/")?,
        "/^ ta vedere nu io : ta io nu marco ^/".to_owned(),
    )?;
    check(
        "explicit number on the signal tile",
        expanded("/^ ta vedere nu io : tau ~ nu marco ^/")?,
        "/^ ta vedere nu io : tau io nu marco ^/".to_owned(),
    )?;
    check(
        "dual antecedent is fine",
        expanded("/^ ta x : ta ~ nu marco ^/")?,
        "/^ ta x : ta x nu marco ^/".to_owned(),
    )?;
    check(
        "only definitions",
        expand_error("/^ defu soglia a 30 ^/")?,
        ExpandError::EmptyExpansion,
    )
}

// ═══════════════════════════ DTI fixture ═══════════════════════════

fn dti() -> Result<Dti, String> {
    let mut universal =
        String::from("#awdti 1\n#role universal\n#version 7\n#lang en\n#numeral base:10 digits:");
    let digits: Vec<String> = (0..10)
        .map(|d| k(&format!("digit{d}")))
        .collect::<Result<_, _>>()?;
    universal.push_str(&digits.join(","));
    universal.push('\n');
    for (d, key) in digits.iter().enumerate() {
        let flag = if d == 9 { "" } else { " morph:+AGGL_NUM" };
        universal.push_str(&format!("rec {key} {d}{flag}\n"));
    }
    for (tag, form) in [
        ("see", "see"),
        ("i", "i"),
        ("dog", "dog"),
        ("bark", "bark"),
        ("sensor", "sensor"),
        ("optical", "optical"),
        ("green", "green"),
        ("emerald", "emerald"),
        ("celsius", "celsius"),
        ("door", "door"),
        ("carry", "carry"),
        ("hidden", "hidden"),
    ] {
        // B9: only these concepts may be joined by `§`.
        let morph = if ["sensor", "optical", "celsius"].contains(&tag) {
            " morph:+UNION"
        } else {
            ""
        };
        universal.push_str(&format!("rec {} {form}{morph}\n", k(tag)?));
    }
    universal.push_str(&format!(
        "rec {} greenemerald display:\"emerald green\" base:{}\n",
        k("greenemerald")?,
        k("green")?
    ));
    let mut local = String::from("#awdti 1\n#role local\n#universal 7\n#lang it\n");
    for key in &digits {
        local.push_str(&format!("use {key}\n"));
    }
    for (tag, form) in [
        ("see", "vedere"),
        ("i", "io"),
        ("dog", "canis"),
        ("bark", "abbaiare"),
        ("sensor", "sensore"),
        ("optical", "ottico"),
        ("green", "verde"),
        ("celsius", "celsius"),
    ] {
        local.push_str(&format!("use {} form:{form}\n", k(tag)?));
    }
    local.push_str(&format!(
        "use {} form:verdesmeraldo display:\"verde smeraldo\" syn:smeraldino\n",
        k("greenemerald")?
    ));
    local.push_str(&format!("use {} form:porta syn:uscio\n", k("door")?));
    local.push_str(&format!("use {} form:portare syn:uscio\n", k("carry")?));
    Dti::load(&universal, &local).map_err(|d| format!("{d:#?}"))
}

fn indexed(src: &str, dti: &Dti) -> Result<Result<String, IndexError>, String> {
    let (session, id) = one(src)?;
    Ok(render_indexed_period(&session, id, dti))
}

// ═══════════════════════════ Indexed form (C.2, D5) ═══════════════════════════

#[test]
fn indexes_written_period() -> TestResult {
    let dti = dti()?;
    let want = format!(
        "/^ ta {} nu {} au {} : ta {} nu ~ ^/",
        k("see")?,
        k("i")?,
        k("dog")?,
        k("bark")?,
    );
    check(
        "indexed",
        indexed("/^ ta vedere nu io au canis : ta abbaiare nu ~ ^/", &dti)?,
        Ok(want),
    )
}

#[test]
fn atoms_segments_and_numerals_b4() -> TestResult {
    let dti = dti()?;
    check(
        "atomic record",
        indexed("/^ ta vedere nu verdesmeraldo ^/", &dti)?,
        Ok(format!("/^ ta {} nu {} ^/", k("see")?, k("greenemerald")?)),
    )?;
    check(
        "segments",
        indexed("/^ ta vedere nu sensore§ottico ^/", &dti)?,
        Ok(format!(
            "/^ ta {} nu {}§{} ^/",
            k("see")?,
            k("sensor")?,
            k("optical")?
        )),
    )?;
    check(
        "numeral literal",
        indexed("/^ tacha vedere nu sensore a 30§celsius ^/", &dti)?,
        Ok(format!(
            "/^ tacha {} nu {} a 30§{} ^/",
            k("see")?,
            k("sensor")?,
            k("celsius")?
        )),
    )?;
    // Digit 9 lacks [+AGGL_NUM] (B9).
    check(
        "digit without +AGGL_NUM",
        indexed("/^ tacha vedere nu sensore a 19 ^/", &dti)?,
        Err(IndexError::Resolve(ResolveError::MorphViolation {
            form: "19".to_owned(),
            key: key("digit9")?,
            flag: MorphFlag::AgglNum,
        })),
    )
}

#[test]
fn morphology_flags_are_validated_b9() -> TestResult {
    let dti = dti()?;
    // `vedere` lacks +UNION: `sensore§vedere` is not a record, the union is refused.
    check(
        "union without +UNION",
        indexed("/^ ta vedere nu sensore§vedere ^/", &dti)?,
        Err(IndexError::Resolve(ResolveError::MorphViolation {
            form: "sensore§vedere".to_owned(),
            key: key("see")?,
            flag: MorphFlag::Union,
        })),
    )?;
    // A whole record needs no +UNION (`verde` has none).
    check(
        "whole record",
        indexed("/^ ta vedere nu verdesmeraldo ^/", &dti).map(|r| r.is_ok())?,
        true,
    )?;
    // Compound records (`base:` + `param:`, atomic form).
    let compound = morph_dti("", "")?;
    check(
        "agglutination without +AGGL",
        super::resolve_glyph_text("cagnone", &compound),
        Err(ResolveError::MorphViolation {
            form: "cagnone".to_owned(),
            key: key("dog")?,
            flag: MorphFlag::Aggl,
        }),
    )?;
    check(
        "contraction without +CONTR",
        super::resolve_glyph_text("cucciolo", &compound),
        Err(ResolveError::MorphViolation {
            form: "cucciolo".to_owned(),
            key: key("dog")?,
            flag: MorphFlag::Contr,
        }),
    )?;
    let allowed = morph_dti(" morph:+AGGL,+CONTR", " morph:+AGGL,+CONTR")?;
    check(
        "agglutination allowed",
        super::resolve_glyph_text("cagnone", &allowed),
        Ok(super::PathResolution::Record(key("bigdog")?)),
    )?;
    check(
        "contraction allowed",
        super::resolve_glyph_text("cucciolo", &allowed),
        Ok(super::PathResolution::Record(key("puppy")?)),
    )?;
    let half = morph_dti(" morph:+AGGL", "")?;
    check(
        "every component needs the flag",
        super::resolve_glyph_text("cagnone", &half),
        Err(ResolveError::MorphViolation {
            form: "cagnone".to_owned(),
            key: key("big")?,
            flag: MorphFlag::Aggl,
        }),
    )
}

/// `cagnone` = `cagn` + `one` (agglutination), `cucciolo` = dog + small (contraction).
fn morph_dti(dog_morph: &str, other_morph: &str) -> Result<Dti, String> {
    let universal = format!(
        "#awdti 1\n#role universal\n#version 7\n#lang en\n\
rec {dog} dog{dog_morph}\nrec {big} big{other_morph}\nrec {small} small{other_morph}\n\
rec {bigdog} bigdog base:{dog} param:{big}\nrec {puppy} puppy base:{dog} param:{small}\n",
        dog = k("dog")?,
        big = k("big")?,
        small = k("small")?,
        bigdog = k("bigdog")?,
        puppy = k("puppy")?,
    );
    let local = format!(
        "#awdti 1\n#role local\n#universal 7\n#lang it\n\
use {dog} form:cagn\nuse {big} form:one\nuse {small} form:piccolo\n\
use {bigdog} form:cagnone\nuse {puppy} form:cucciolo\n",
        dog = k("dog")?,
        big = k("big")?,
        small = k("small")?,
        bigdog = k("bigdog")?,
        puppy = k("puppy")?,
    );
    Dti::load(&universal, &local).map_err(|d| format!("{d:#?}"))
}

#[test]
fn resolution_errors() -> TestResult {
    let dti = dti()?;
    check(
        "unknown",
        indexed("/^ ta vedere nu blu ^/", &dti)?,
        Err(IndexError::Resolve(ResolveError::UnknownForm {
            form: "blu".to_owned(),
        })),
    )?;
    check(
        "not enabled (B5)",
        indexed("/^ ta vedere nu hidden ^/", &dti)?,
        Err(IndexError::Resolve(ResolveError::NotEnabled {
            form: "hidden".to_owned(),
            key: key("hidden")?,
        })),
    )?;
    // Canonical text admits only primary forms: synonyms and display forms are
    // normalized by the editor, stage 3 does not know them.
    check(
        "synonym",
        indexed("/^ ta vedere nu uscio ^/", &dti)?,
        Err(IndexError::Resolve(ResolveError::UnknownForm {
            form: "uscio".to_owned(),
        })),
    )?;
    check(
        "synonym of a single concept",
        indexed("/^ ta vedere nu smeraldino ^/", &dti)?,
        Err(IndexError::Resolve(ResolveError::UnknownForm {
            form: "smeraldino".to_owned(),
        })),
    )
}

#[test]
fn indexed_form_is_independent_of_the_expanded_form() -> TestResult {
    let dti = dti()?;
    // Multi-valued variable: not expandable in the signal pair, transmissible here.
    check(
        "def blocks kept, names indexed",
        indexed(
            "/^ defu io a sensore y defu io absi verde y ta io nu canis ^/",
            &dti,
        )?,
        Ok(format!(
            "/^ defu {i} a {s} y defu {i} absi {g} y ta {i} nu {d} ^/",
            i = k("i")?,
            s = k("sensor")?,
            g = k("green")?,
            d = k("dog")?
        )),
    )?;
    check(
        "portability blocks kept",
        indexed(
            "/^ ta vedere nu io : anchor porta : ta abbaiare nu canis : end ~ ^/",
            &dti,
        )?,
        Ok(format!(
            "/^ ta {} nu {} : anchor {} : ta {} nu {} : end ~ ^/",
            k("see")?,
            k("i")?,
            k("door")?,
            k("bark")?,
            k("dog")?
        )),
    )?;
    check(
        "anaphora bases",
        indexed(
            "/^ ta vedere nu sensore§ottico : ta abbaiare nu sensore~ ^/",
            &dti,
        )?,
        Ok(format!(
            "/^ ta {see} nu {s}§{o} : ta {b} nu {s}~ ^/",
            see = k("see")?,
            s = k("sensor")?,
            o = k("optical")?,
            b = k("bark")?
        )),
    )?;
    check(
        "inverse anaphora atoms",
        indexed(
            "/^ ta vedere nu sensore : ta abbaiare nu sensore§ottico~ ^/",
            &dti,
        )?,
        Ok(format!(
            "/^ ta {see} nu {s} : ta {b} nu {s}§{o}~ ^/",
            see = k("see")?,
            s = k("sensor")?,
            o = k("optical")?,
            b = k("bark")?
        )),
    )?;
    check(
        "sema by glyph",
        indexed(
            "/^ ta vedere nu io : ta abbaiare nu canis :: tozato vedere~~ ^/",
            &dti,
        )?,
        Ok(format!(
            "/^ ta {see} nu {i} : ta {b} nu {d} :: tozato {see}~~ ^/",
            see = k("see")?,
            i = k("i")?,
            b = k("bark")?,
            d = k("dog")?
        )),
    )?;
    let (session, id) = one("/^ ta vedere nu io : tozato ~~ ^/")?;
    check(
        "lexical wire mode is the written form",
        super::render_wire(&session, id, &dti, super::WireMode::Lexical)
            .map_err(|e| e.to_string())?,
        "/^ ta vedere nu io : tozato ~~ ^/".to_owned(),
    )
}

#[test]
fn bound_symbols_use_dti_or_ephemeral_keys() -> TestResult {
    let dti = dti()?;
    let dev = DtiKey::session_symbol("dev").map_err(|e| e.to_string())?;
    // A def name absent from the DTI: ephemeral key for the name and its references.
    check(
        "ephemeral def name",
        indexed(
            "/^ defu dev a sensore y ta vedere nu dev y defu dev abse ~ ^/",
            &dti,
        )?,
        Ok(format!(
            "/^ defu {dev} a {s} y ta {see} nu {dev} y defu {dev} abse ~ ^/",
            s = k("sensor")?,
            see = k("see")?,
        )),
    )?;
    // A def name that is an enabled primary form keeps its DTI key.
    check(
        "dti def name",
        indexed("/^ defu io a sensore y ta vedere nu io ^/", &dti)?,
        Ok(format!(
            "/^ defu {i} a {s} y ta {see} nu {i} ^/",
            i = k("i")?,
            s = k("sensor")?,
            see = k("see")?,
        )),
    )?;
    // D1: present in the snapshot but not enabled (`hidden`) → ephemeral key.
    let hidden = DtiKey::session_symbol("hidden").map_err(|e| e.to_string())?;
    check(
        "not enabled def name",
        indexed("/^ defu hidden a sensore y ta vedere nu hidden ^/", &dti)?,
        Ok(format!(
            "/^ defu {hidden} a {s} y ta {see} nu {hidden} ^/",
            s = k("sensor")?,
            see = k("see")?,
        )),
    )?;
    // D2: qualified anchor id, atom by atom (`porta` enabled, `uno` ephemeral).
    let uno = DtiKey::session_symbol("uno").map_err(|e| e.to_string())?;
    check(
        "qualified anchor id",
        indexed(
            "/^ ta vedere nu io : anchorrel porta§uno : ta abbaiare nu canis : reloff porta§uno : end ~ ^/",
            &dti,
        )?,
        Ok(format!(
            "/^ ta {see} nu {i} : anchorrel {door}§{uno} : ta {b} nu {d} : reloff {door}§{uno} : end ~ ^/",
            see = k("see")?,
            i = k("i")?,
            door = k("door")?,
            b = k("bark")?,
            d = k("dog")?,
        )),
    )?;
    // A glyph equal to a variable name outside its life is not a reference.
    check(
        "after abse the name is an ordinary glyph",
        indexed(
            "/^ defu dev a sensore y defu dev abse ~ y ta vedere nu dev ^/",
            &dti,
        )?,
        Err(IndexError::Resolve(ResolveError::UnknownForm {
            form: "dev".to_owned(),
        })),
    )
}

#[test]
fn ephemeral_keys_are_deterministic_valid_keys() -> TestResult {
    let first = DtiKey::session_symbol("dev").map_err(|e| e.to_string())?;
    let again = DtiKey::session_symbol("dev").map_err(|e| e.to_string())?;
    let other = DtiKey::session_symbol("dev2").map_err(|e| e.to_string())?;
    check("deterministic", first, again)?;
    check("distinct", first == other, false)?;
    check(
        "valid key",
        DtiKey::parse(first.as_str()).map_err(|e| e.to_string())?,
        first,
    )?;
    check(
        "first character is a letter",
        first
            .as_str()
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase()),
        true,
    )
}

// ═══════════════════════════ Indexed input (C.2, D3) ═══════════════════════════

#[test]
fn validates_indexed_input() -> TestResult {
    let dti = dti()?;
    let text = format!("/^ ta {} nu {} au 30 ^/", k("see")?, k("i")?);
    let (session, id) = one(&text)?;
    let report = validate_indexed_input(&session, id, &dti).map_err(|e| format!("{e:#?}"))?;
    check("entries", report.entries.len(), 2)?;
    check(
        "enabled",
        report.entries.first().map(|e| e.resolution),
        Some(KeyResolution::Enabled(key("see")?)),
    )?;
    // `emerald` is not enabled and has no base: unknown under the degrade policy.
    let text = format!("/^ ta {} nu {} ^/", k("see")?, k("emerald")?);
    let (session, id) = one(&text)?;
    match validate_indexed_input(&session, id, &dti) {
        Err(errors) if matches!(errors.first(), Some(IndexedInputError::UnknownKey { .. })) => {}
        other => return Err(format!("expected UnknownKey, got {other:?}")),
    }
    let (session, id) = one("/^ ta vedere nu io ^/")?;
    match validate_indexed_input(&session, id, &dti) {
        Err(errors) if errors.len() == 2 => Ok(()),
        other => Err(format!("expected 2 NotAKey errors, got {other:?}")),
    }
}

#[test]
fn validates_every_indexed_operand() -> TestResult {
    let dti = dti()?;
    // def name and values, portability id, anaphora bases: all keys are checked.
    let text = format!(
        "/^ defu {i} a {s}§{o} y ta {see} nu {i} : anchor {door} : ta {b} nu {s}~ : end ~ ^/",
        i = k("i")?,
        s = k("sensor")?,
        o = k("optical")?,
        see = k("see")?,
        door = k("door")?,
        b = k("bark")?,
    );
    let (session, id) = one(&text)?;
    let report = validate_indexed_input(&session, id, &dti).map_err(|e| format!("{e:#?}"))?;
    // Bound symbols (def name `i`, its reference, anchor id) are session symbols:
    // accepted as valid keys, not resolved against the static DTI.
    check("entries", report.entries.len(), 5)?;
    // A def name is a session symbol: a key unknown to the DTI is accepted there
    // (and in its references), but not in an ordinary slot.
    let dev = DtiKey::session_symbol("dev").map_err(|e| e.to_string())?;
    let text = format!("/^ defu {dev} a 30 y ta {} nu {dev} ^/", k("see")?);
    let (session, id) = one(&text)?;
    validate_indexed_input(&session, id, &dti).map_err(|e| format!("{e:#?}"))?;
    let text = format!("/^ ta {} nu {dev} ^/", k("see")?);
    let (session, id) = one(&text)?;
    match validate_indexed_input(&session, id, &dti) {
        Err(errors) if matches!(errors.first(), Some(IndexedInputError::UnknownKey { .. })) => {}
        other => return Err(format!("expected UnknownKey, got {other:?}")),
    }
    // A leading digit is always a numeral: `3abc` is an invalid numeral, not a key.
    let text = format!("/^ ta {} nu 3abc ^/", k("see")?);
    let (session, id) = one(&text)?;
    match validate_indexed_input(&session, id, &dti) {
        Err(errors)
            if matches!(
                errors.first(),
                Some(IndexedInputError::InvalidNumeral { .. })
            ) => {}
        other => return Err(format!("expected InvalidNumeral, got {other:?}")),
    }
    // B9 on receipt: `see` lacks +UNION, digit 9 lacks +AGGL_NUM.
    let text = format!(
        "/^ ta {} nu {}§{} a 19 ^/",
        k("see")?,
        k("sensor")?,
        k("see")?
    );
    let (session, id) = one(&text)?;
    match validate_indexed_input(&session, id, &dti) {
        Err(errors) => check(
            "morph violations",
            errors
                .iter()
                .filter_map(|e| match e {
                    IndexedInputError::MorphViolation { flag, .. } => Some(*flag),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![MorphFlag::Union, MorphFlag::AgglNum],
        ),
        Ok(report) => Err(format!("expected morph violations, got {report:?}")),
    }
}

// ═══════════════════════════ Views (C.3) ═══════════════════════════

#[test]
fn views_in_every_mode() -> TestResult {
    let dti = dti()?;
    let (session, id) = one("/^ ta vedere nu verdesmeraldo : ta abbaiare nu ~ ^/")?;
    check(
        "human original",
        render_view(
            &session,
            id,
            Some(&dti),
            ViewDisplayMode::HumanOriginal,
            Layout::Linear,
        )
        .map_err(|e| e.to_string())?,
        "/^ ta vedere nu verde smeraldo : ta abbaiare nu ~ ^/".to_owned(),
    )?;
    check(
        "canonical lexical",
        render_view(
            &session,
            id,
            Some(&dti),
            ViewDisplayMode::CanonicalLexical,
            Layout::Linear,
        )
        .map_err(|e| e.to_string())?,
        "/^ ta vedere nu verdesmeraldo : ta abbaiare nu ~ ^/".to_owned(),
    )?;
    check(
        "indexed",
        render_view(
            &session,
            id,
            Some(&dti),
            ViewDisplayMode::DtiIndexed,
            Layout::Linear,
        )
        .map_err(|e| e.to_string())?,
        format!(
            "/^ ta {} nu {} : ta {} nu ~ ^/",
            k("see")?,
            k("greenemerald")?,
            k("bark")?,
        ),
    )?;
    check(
        "c-style",
        render_view(
            &session,
            id,
            None,
            ViewDisplayMode::CanonicalLexical,
            Layout::CStyle { indent: 4 },
        )
        .map_err(|e| e.to_string())?,
        "/^\n    ta vedere nu verdesmeraldo\n        : ta abbaiare nu ~\n^/".to_owned(),
    )?;
    match render_view(
        &session,
        id,
        None,
        ViewDisplayMode::DtiIndexed,
        Layout::Linear,
    ) {
        Err(super::ViewError::MissingDti) => Ok(()),
        other => Err(format!("expected MissingDti, got {other:?}")),
    }
}
