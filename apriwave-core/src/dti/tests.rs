//! Phase B tests: keys, `.awdti` parsing, cross validation, lookups and policy.

use super::{
    CensusKind, CensusWarning, Dti, DtiDiagnostic, DtiErrorKind, DtiKey, DtiSource, FormResolution,
    KeyError, KeyResolution, UnknownKeyPolicy,
};

type TestResult = Result<(), String>;

/// Builds a valid key from a short tag, padding the body with zeros.
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

struct Fixture {
    universal: String,
    local: String,
}

/// Small universal snapshot (English) and Italian overlay.
fn fixture() -> Result<Fixture, String> {
    let (green, emerald, green_emerald, door, carry, digit0, digit1, red) = (
        k("green")?,
        k("emerald")?,
        k("greenemerald")?,
        k("door")?,
        k("carry")?,
        k("digit0")?,
        k("digit1")?,
        k("red")?,
    );
    let old = k("oldgreen")?;
    let universal = format!(
        "#awdti 1\n#role universal\n#version 42\n#lang en\n#numeral base:2 digits:{digit0},{digit1}\n\
rec {green} green syn:verdant\n\
rec {emerald} emerald\n\
rec {green_emerald} greenemerald display:\"emerald green\" base:{green} param:{emerald}\n\
rec {door} door\n\
rec {carry} carry\n\
rec {digit0} 0 morph:+AGGL_NUM\n\
rec {digit1} 1 morph:+AGGL_NUM\n\
rec {red} red attr:wavelength§700nm\n\
rec {old} viridian base:{green} superseded_by:{green}\n"
    );
    let local_concept = k("localsensor")?;
    let local = format!(
        "#awdti 1\n#role local\n#universal 42\n#lang it\n#policy degrade\n\
use {green} form:verde\n\
use {green_emerald} form:verdesmeraldo display:\"verde smeraldo\"\n\
use {door} form:porta syn:uscio\n\
use {carry} form:portare syn:uscio\n\
use {digit0}\n\
use {digit1}\n\
new {local_concept} sensoreluce base:{door} display:\"sensore luce\"\n"
    );
    Ok(Fixture { universal, local })
}

fn load(f: &Fixture) -> Result<Dti, String> {
    Dti::load(&f.universal, &f.local).map_err(|d| format!("{d:#?}"))
}

fn expect_error(universal: &str, local: &str, source: DtiSource, kind: DtiErrorKind) -> TestResult {
    match Dti::load(universal, local) {
        Ok(_) => Err(format!("accepted, expected {kind:?}")),
        Err(diagnostics) => {
            if diagnostics
                .iter()
                .any(|d: &DtiDiagnostic| d.source == source && d.kind == kind)
            {
                Ok(())
            } else {
                Err(format!(
                    "expected {kind:?} in {source:?}, got {diagnostics:#?}"
                ))
            }
        }
    }
}

fn check<T: PartialEq + core::fmt::Debug>(what: &str, got: T, want: T) -> TestResult {
    if got == want {
        Ok(())
    } else {
        Err(format!("{what}: got {got:?}, want {want:?}"))
    }
}

// ═══════════════════════════ Keys (B1) ═══════════════════════════

#[test]
fn luhn_mod_36_round_trip_and_error_detection() -> TestResult {
    let valid = key("k9m2x7p4q1w8z3n6v5b0r2t9y4h7j8k")?;
    check("round trip", DtiKey::parse(valid.as_str()), Ok(valid))?;
    check(
        "length",
        DtiKey::parse(valid.as_str()).map(|k| k.as_str().len()),
        Ok(32),
    )?;
    // Every single-character substitution is detected.
    let text = valid.as_str().to_owned();
    for (position, original) in text.char_indices() {
        for replacement in "0123456789abcdefghijklmnopqrstuvwxyz".chars() {
            if replacement == original {
                continue;
            }
            let mut mutated: Vec<char> = text.chars().collect();
            if let Some(slot) = mutated.get_mut(position) {
                *slot = replacement;
            }
            let mutated: String = mutated.into_iter().collect();
            if DtiKey::parse(&mutated).is_ok() {
                return Err(format!("undetected substitution at {position}: {mutated}"));
            }
        }
    }
    check(
        "too short",
        DtiKey::parse("abc"),
        Err(KeyError::Length { found: 3 }),
    )?;
    check(
        "uppercase",
        DtiKey::parse(&text.to_uppercase()).map(|_| ()),
        Err(KeyError::Character {
            ch: text
                .to_uppercase()
                .chars()
                .find(|c| c.is_ascii_uppercase())
                .unwrap_or('A'),
        }),
    )?;
    check(
        "body alphabet",
        DtiKey::from_random_body([b'-'; 31]).map(|_| ()),
        Err(KeyError::Character { ch: '-' }),
    )?;
    // The first character is always a letter: a leading digit is a numeral.
    check(
        "first character of a body",
        DtiKey::from_random_body([b'7'; 31]).map(|_| ()),
        Err(KeyError::FirstCharacter { ch: '7' }),
    )?;
    let mut digit_first = *b"0000000000000000000000000000000";
    if let Some(slot) = digit_first.get_mut(1) {
        *slot = b'a';
    }
    let mut text = String::from_utf8(digit_first.to_vec()).map_err(|e| e.to_string())?;
    text.push('0');
    check(
        "first character of a key",
        DtiKey::parse(&text).map(|_| ()),
        Err(KeyError::FirstCharacter { ch: '0' }),
    )
}

// ═══════════════════════════ Loading and lookups ═══════════════════════════

#[test]
fn fixture_loads_and_resolves_forms() -> TestResult {
    let f = fixture()?;
    let dti = load(&f)?;
    check("version", dti.universal().version(), 42)?;
    check(
        "policy",
        dti.overlay().policy(),
        UnknownKeyPolicy::DegradeToBase,
    )?;
    check(
        "local primary",
        dti.resolve_form("verde"),
        FormResolution::Unique(key("green")?),
    )?;
    check(
        "exchange primary",
        dti.resolve_form("green"),
        FormResolution::Unique(key("green")?),
    )?;
    check(
        "synonym",
        dti.resolve_form("verdant"),
        FormResolution::Unique(key("green")?),
    )?;
    check(
        "composite record",
        dti.resolve_form("verdesmeraldo"),
        FormResolution::Unique(key("greenemerald")?),
    )?;
    check(
        "display with spaces",
        dti.resolve_form("verde smeraldo"),
        FormResolution::Unique(key("greenemerald")?),
    )?;
    // B3: a synonym shared by two concepts is ambiguous; the editor must ask.
    let mut both = vec![key("door")?, key("carry")?];
    both.sort();
    check(
        "homonym synonym",
        dti.resolve_form("uscio"),
        FormResolution::Ambiguous(both),
    )?;
    check("unknown", dti.resolve_form("blu"), FormResolution::Unknown)?;
    check(
        "primary form",
        dti.primary_form(&key("door")?),
        Some("porta"),
    )?;
    check(
        "exchange fallback",
        dti.primary_form(&key("red")?),
        Some("red"),
    )?;
    check(
        "display",
        dti.display_form(&key("greenemerald")?),
        Some("verde smeraldo"),
    )?;
    let numeral = dti.universal().numeral().ok_or("numeral missing")?;
    check("numeral base", numeral.base, 2)?;
    let digit = dti
        .universal()
        .concept(&key("digit0")?)
        .ok_or("digit missing")?;
    check("morph", digit.morph.aggl_num, true)?;
    let red = dti.universal().concept(&key("red")?).ok_or("red missing")?;
    check(
        "attr",
        red.attrs.first().map(|(k, v)| (&**k, &**v)),
        Some(("wavelength", "700nm")),
    )
}

#[test]
fn key_resolution_policies_d3() -> TestResult {
    let f = fixture()?;
    let dti = load(&f)?;
    check(
        "enabled",
        dti.resolve_key(&key("door")?),
        KeyResolution::Enabled(key("door")?),
    )?;
    check(
        "local concept",
        dti.resolve_key(&key("localsensor")?),
        KeyResolution::Enabled(key("localsensor")?),
    )?;
    // `emerald` is not enabled and has no base: unknown.
    check(
        "not enabled",
        dti.resolve_key(&key("emerald")?),
        KeyResolution::Unknown,
    )?;
    // A key absent from both authorities.
    check(
        "unknown key",
        dti.resolve_key(&key("zzz")?),
        KeyResolution::Unknown,
    )?;
    // Degradation: disable `greenemerald`, whose base `green` is enabled.
    let universal = f.universal.as_str();
    let enabled_line = format!(
        "use {} form:verdesmeraldo display:\"verde smeraldo\"\n",
        k("greenemerald")?
    );
    let local = f.local.replace(&enabled_line, "");
    let dti = Dti::load(universal, &local).map_err(|d| format!("{d:#?}"))?;
    check(
        "degraded",
        dti.resolve_key(&key("greenemerald")?),
        KeyResolution::Degraded {
            requested: key("greenemerald")?,
            to: key("green")?,
        },
    )?;
    let reject = local.replace("#policy degrade", "#policy reject");
    let dti = Dti::load(universal, &reject).map_err(|d| format!("{d:#?}"))?;
    check(
        "reject",
        dti.resolve_key(&key("greenemerald")?),
        KeyResolution::Rejected,
    )?;
    let unknown = local.replace("#policy degrade", "#policy unknown");
    let dti = Dti::load(universal, &unknown).map_err(|d| format!("{d:#?}"))?;
    check(
        "mark unknown",
        dti.resolve_key(&key("greenemerald")?),
        KeyResolution::Unknown,
    )
}

// ═══════════════════════════ Validation errors ═══════════════════════════

#[test]
fn header_errors() -> TestResult {
    let f = fixture()?;
    expect_error(
        &f.universal.replace("#version 42\n", ""),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::MissingHeader { name: "version" },
    )?;
    expect_error(
        &f.universal,
        &f.local.replace("#universal 42", "#universal 41"),
        DtiSource::Local,
        DtiErrorKind::VersionMismatch {
            snapshot: 42,
            overlay: 41,
        },
    )?;
    expect_error(
        &f.universal.replace("#role universal", "#role local"),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::WrongRole,
    )?;
    expect_error(
        &f.universal.replace("#awdti 1", "#awdti 2"),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::UnsupportedFormatVersion,
    )?;
    expect_error(
        &format!("{}#lang en\n", f.universal),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::HeaderAfterRecords,
    )?;
    expect_error(
        &f.universal.replace("base:2", "base:3"),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::InvalidNumeral,
    )
}

#[test]
fn record_errors() -> TestResult {
    let f = fixture()?;
    let green = k("green")?;
    expect_error(
        &format!("{}rec {green} other\n", f.universal),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::DuplicateKey(key("green")?),
    )?;
    expect_error(
        &format!("{}rec {} green\n", f.universal, k("green2")?),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::DuplicatePrimary,
    )?;
    // v4.1: uppercase and `_` are valid in forms; an invisible character is not.
    Dti::load(
        &format!(
            "{}rec {} Green\nrec {} a_b\n",
            f.universal,
            k("upper")?,
            k("under")?
        ),
        &f.local,
    )
    .map_err(|d| format!("{d:#?}"))?;
    expect_error(
        &format!("{}rec {} x\u{200F}\n", f.universal, k("bidi")?),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::InvalidForm,
    )?;
    expect_error(
        &format!("{}rec {} x display:\"a  b\"\n", f.universal, k("disp")?),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::InvalidDisplay,
    )?;
    expect_error(
        &format!("{}rec {} x display:\"open\n", f.universal, k("quote")?),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::UnterminatedQuote,
    )?;
    expect_error(
        &format!("{}rec {} x color:red\n", f.universal, k("tag")?),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::UnknownTag,
    )?;
    expect_error(
        &format!("{}rec {} x morph:+FLY\n", f.universal, k("morph")?),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::UnknownMorphFlag,
    )?;
    expect_error(
        &format!(
            "{}rec {} x base:{}\n",
            f.universal,
            k("orphan")?,
            k("missing")?
        ),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::OrphanReference(key("missing")?),
    )?;
    expect_error(
        &format!("{}rec  {green} x\n", f.universal),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::EmptyField,
    )?;
    expect_error(
        &format!("{}use {green}\n", f.universal),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::UnknownRecordKind,
    )
}

#[test]
fn base_cycles_are_rejected() -> TestResult {
    let f = fixture()?;
    let (a, b) = (k("cyclea")?, k("cycleb")?);
    expect_error(
        &format!("{}rec {a} ca base:{b}\nrec {b} cb base:{a}\n", f.universal),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::BaseCycle(key("cyclea")?),
    )?;
    let (la, lb) = (k("localcyclea")?, k("localcycleb")?);
    expect_error(
        &f.universal,
        &format!("{}new {la} la base:{lb}\nnew {lb} lb base:{la}\n", f.local),
        DtiSource::Local,
        DtiErrorKind::BaseCycle(key("localcyclea")?),
    )
}

#[test]
fn forms_admit_any_visible_character_v4_1() -> TestResult {
    let f = fixture()?;
    // v4.1: `§`, `"`, uppercase, punctuation and other scripts are valid forms;
    // a form containing `§` is one glyph (looked up whole).
    let universal = f.universal.replacen("door\n", "Door§front\"x\n", 1);
    let local = f
        .local
        .replacen("form:verdesmeraldo", "form:verde§smeraldo", 1)
        .replacen("form:porta", "form:東京", 1);
    let dti = Dti::load(&universal, &local).map_err(|d| format!("{d:#?}"))?;
    check(
        "local form with §",
        dti.resolve_primary("verde§smeraldo"),
        FormResolution::Unique(key("greenemerald")?),
    )?;
    check(
        "universal form with § and \"",
        dti.resolve_primary("Door§front\"x"),
        FormResolution::Unique(key("door")?),
    )?;
    check(
        "other script",
        dti.resolve_primary("東京"),
        FormResolution::Unique(key("door")?),
    )?;
    // Only invisible characters are refused.
    for bad in ["x\u{202E}y", "x\u{200B}", "\u{FEFF}"] {
        expect_error(
            &f.universal.replacen("door\n", &format!("{bad}\n"), 1),
            &f.local,
            DtiSource::Universal,
            DtiErrorKind::InvalidForm,
        )?;
    }
    Ok(())
}

#[test]
fn labels_coexist_with_glyphs_v4_1() -> TestResult {
    let f = fixture()?;
    let (roma, verde, green, door) = (
        key("labelroma")?,
        key("labelverde")?,
        key("labelgreen")?,
        key("localdoor")?,
    );
    let local = format!(
        "{}label {roma} Roma\nlabel {verde} verde\nlabel {green} green\nnew {door} door\n",
        f.local
    );
    let dti = Dti::load(&f.universal, &local).map_err(|d| format!("{d:#?}"))?;
    check("label Roma", dti.label("Roma"), Some(roma))?;
    check("label count", dti.overlay().labels().len(), 3)?;
    check(
        "label form",
        dti.overlay().labels().form(&verde),
        Some("verde"),
    )?;
    // `verde` is both a local glyph and a local label.
    check("label verde", dti.label("verde"), Some(verde))?;
    check(
        "glyph verde",
        dti.resolve_primary("verde"),
        FormResolution::Unique(key("green")?),
    )?;
    // Census warnings: forms that are already universal glyphs (exchange
    // language), never errors.
    check(
        "census warnings",
        dti.overlay().census_warnings(),
        &[
            CensusWarning {
                line: 15,
                kind: CensusKind::Label,
                form: Box::from("green"),
                universal: key("green")?,
            },
            CensusWarning {
                line: 16,
                kind: CensusKind::Glyph,
                form: Box::from("door"),
                universal: key("door")?,
            },
        ][..],
    )
}

#[test]
fn label_record_rules_v4_1() -> TestResult {
    let f = fixture()?;
    let (a, b) = (k("labela")?, k("labelb")?);
    expect_error(
        &f.universal,
        &format!("{}label {a} roma\nlabel {b} roma\n", f.local),
        DtiSource::Local,
        DtiErrorKind::DuplicateLabel,
    )?;
    expect_error(
        &f.universal,
        &format!("{}label {a} roma\nlabel {a} milano\n", f.local),
        DtiSource::Local,
        DtiErrorKind::DuplicateKey(key("labela")?),
    )?;
    expect_error(
        &f.universal,
        &format!("{}label {} roma\n", f.local, k("door")?),
        DtiSource::Local,
        DtiErrorKind::LocalShadowsUniversal(key("door")?),
    )?;
    expect_error(
        &f.universal,
        &format!("{}label {a} roma display:\"Roma\"\n", f.local),
        DtiSource::Local,
        DtiErrorKind::TagNotAllowed,
    )?;
    expect_error(
        &f.universal,
        &format!("{}label {a}\n", f.local),
        DtiSource::Local,
        DtiErrorKind::MissingField { name: "primary" },
    )?;
    expect_error(
        &format!("{}rec {a} roma\nlabel {b} roma\n", f.universal),
        &f.local,
        DtiSource::Universal,
        DtiErrorKind::UnknownRecordKind,
    )
}

#[test]
fn overlay_rules_b5_b6_d6() -> TestResult {
    let f = fixture()?;
    // B6: `use` on a deprecated key names the successor.
    expect_error(
        &f.universal,
        &format!("{}use {}\n", f.local, k("oldgreen")?),
        DtiSource::Local,
        DtiErrorKind::SupersededKey {
            key: key("oldgreen")?,
            successor: key("green")?,
        },
    )?;
    // `use` of a key absent from the snapshot.
    expect_error(
        &f.universal,
        &format!("{}use {}\n", f.local, k("ghost")?),
        DtiSource::Local,
        DtiErrorKind::OrphanReference(key("ghost")?),
    )?;
    // Anti-spoofing: a local concept cannot reuse a universal key.
    expect_error(
        &f.universal,
        &format!("{}new {} rosso\n", f.local, k("red")?),
        DtiSource::Local,
        DtiErrorKind::LocalShadowsUniversal(key("red")?),
    )?;
    // B3: primary form unique per language.
    expect_error(
        &f.universal,
        &format!("{}new {} porta\n", f.local, k("door2")?),
        DtiSource::Local,
        DtiErrorKind::DuplicatePrimary,
    )?;
    // Tags allowed only where meaningful.
    expect_error(
        &f.universal,
        &format!(
            "{}use {} superseded_by:{}\n",
            f.local,
            k("red")?,
            k("green")?
        ),
        DtiSource::Local,
        DtiErrorKind::TagNotAllowed,
    )?;
    // Display or synonyms on `use` require a local form.
    expect_error(
        &f.universal,
        &format!("{}use {} syn:rosso\n", f.local, k("red")?),
        DtiSource::Local,
        DtiErrorKind::MissingField { name: "form" },
    )?;
    // Universal-only headers are rejected in an overlay.
    expect_error(
        &f.universal,
        &f.local.replace("#lang it", "#lang it\n#version 42"),
        DtiSource::Local,
        DtiErrorKind::WrongRole,
    )
}

#[test]
fn all_diagnostics_are_collected_and_sorted() -> TestResult {
    let f = fixture()?;
    let universal = format!("{}rec {} x\u{202E}\nrec bad x\n", f.universal, k("bad1")?);
    let local = format!("{}use {}\n", f.local, k("ghost")?);
    match Dti::load(&universal, &local) {
        Ok(_) => Err("accepted".to_owned()),
        Err(diagnostics) => {
            check("count", diagnostics.len(), 3)?;
            let order: Vec<DtiSource> = diagnostics.iter().map(|d| d.source).collect();
            check(
                "order",
                order,
                vec![DtiSource::Universal, DtiSource::Universal, DtiSource::Local],
            )
        }
    }
}

#[test]
fn tilde_forms_are_valid_glyphs_and_labels_v4_1() -> TestResult {
    let f = fixture()?;
    // `~` and `~~` may be censused as glyphs (universal or local) and as labels.
    let universal = format!("{}rec {} ~ syn:≈\n", f.universal, k("tilde")?);
    let local = format!(
        "{}use {} form:~\nnew {} ~~\nlabel {} ~\nlabel {} ~~\n",
        f.local,
        k("tilde")?,
        k("localtildes")?,
        k("labeltilde")?,
        k("labeltildes")?
    );
    let dti = Dti::load(&universal, &local).map_err(|d| format!("{d:#?}"))?;
    check(
        "glyph ~",
        dti.resolve_primary("~"),
        FormResolution::Unique(key("tilde")?),
    )?;
    check(
        "glyph ~~",
        dti.resolve_primary("~~"),
        FormResolution::Unique(key("localtildes")?),
    )?;
    check("label ~", dti.label("~"), Some(key("labeltilde")?))?;
    check("label ~~", dti.label("~~"), Some(key("labeltildes")?))?;
    check(
        "synonym of ~",
        dti.resolve_form("≈"),
        FormResolution::Unique(key("tilde")?),
    )
}
