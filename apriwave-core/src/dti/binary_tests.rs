//! `.dti.bin` tests: round trips, determinism, FNV-1a and buffer cache (the
//! file-system API is tested in the `apriwave` crate).

use alloc::collections::{BTreeMap, BTreeSet};

use super::super::{
    fnv1a_64, load_or_compile_local, load_or_compile_universal, ConceptRecord, Dti, DtiDiagnostic,
    DtiErrorKind, DtiKey, DtiLoadError, DtiSource, LexicalEntry, Lexicon, LocalOverlay, MorphFlags,
    UniversalSnapshot, UnknownKeyPolicy,
};

type TestResult = Result<(), String>;

/// Builds a valid key from a short tag (first character must be a letter),
/// padding the body with zeros.
fn k(tag: &str) -> Result<String, String> {
    let mut body = [b'0'; 31];
    for (slot, byte) in body.iter_mut().zip(tag.bytes()) {
        *slot = byte;
    }
    DtiKey::from_random_body(body)
        .map(|key| key.as_str().to_owned())
        .map_err(|e| e.to_string())
}

fn key(tag: &str) -> Result<DtiKey, String> {
    DtiKey::parse(&k(tag)?).map_err(|e| e.to_string())
}

fn check<T: PartialEq + core::fmt::Debug>(what: &str, got: T, want: T) -> TestResult {
    if got == want {
        Ok(())
    } else {
        Err(format!("{what}: got {got:?}, want {want:?}"))
    }
}

fn universal_source(version: u64) -> Result<String, String> {
    let (zebra, green, emerald, green_emerald, door, carry, digit0, digit1, red, old) = (
        k("zebra")?,
        k("green")?,
        k("emerald")?,
        k("greenemerald")?,
        k("door")?,
        k("carry")?,
        k("digit0")?,
        k("digit1")?,
        k("red")?,
        k("oldgreen")?,
    );
    // `zebra` comes first although its key sorts last: the shared synonym
    // `verdant` must keep its insertion order through the binary.
    Ok(format!(
        "#awdti 1\n#role universal\n#version {version}\n#lang en\n#numeral base:2 digits:{digit0},{digit1}\n\
rec {zebra} zebra syn:verdant,striped\n\
rec {green} green syn:verdant\n\
rec {emerald} emerald display:\"bright stone\"\n\
rec {green_emerald} greenemerald display:\"bright stone\" base:{green} param:{emerald}\n\
rec {door} door\n\
rec {carry} carry morph:+AGGL,+UNION\n\
rec {digit0} 0 morph:+AGGL_NUM\n\
rec {digit1} 1 morph:+AGGL_NUM\n\
rec {red} red attr:wavelength§700nm attr:\"unit§nano meter\" attr:note§say\"hi\"\n\
rec {old} viridian base:{green} superseded_by:{green}\n"
    ))
}

fn local_source(universal_version: u64) -> Result<String, String> {
    let (green, green_emerald, door, carry, digit0, digit1, sensor, lamp) = (
        k("green")?,
        k("greenemerald")?,
        k("door")?,
        k("carry")?,
        k("digit0")?,
        k("digit1")?,
        k("localsensor")?,
        k("alamp")?,
    );
    Ok(format!(
        "#awdti 1\n#role local\n#universal {universal_version}\n#lang it\n#policy reject\n\
use {green} form:verde\n\
use {green_emerald} form:verdesmeraldo display:\"verde smeraldo\"\n\
use {door} form:porta syn:uscio\n\
new {sensor} sensoreluce base:{door} display:\"sensore luce\" syn:uscio morph:+CONTR\n\
use {carry} form:portare syn:uscio\n\
use {digit0}\n\
use {digit1}\n\
new {lamp} lampada base:{sensor}\n"
    ))
}

fn universal() -> Result<(String, UniversalSnapshot), String> {
    let source = universal_source(42)?;
    let snapshot = UniversalSnapshot::parse(&source).map_err(|d| format!("{d:#?}"))?;
    Ok((source, snapshot))
}

fn local(universal: &UniversalSnapshot) -> Result<(String, LocalOverlay), String> {
    let source = local_source(42)?;
    let overlay = LocalOverlay::parse(&source, universal).map_err(|d| format!("{d:#?}"))?;
    Ok((source, overlay))
}

fn expect_error<T>(what: &str, result: Result<T, DtiLoadError>, want: &DtiLoadError) -> TestResult {
    match result {
        Ok(_) => Err(format!("{what}: accepted, expected {want:?}")),
        Err(error) => check(what, &error, want),
    }
}

// ═══════════════════════════ FNV-1a ═══════════════════════════

#[test]
fn fnv1a_64_known_vectors() -> TestResult {
    check("empty", fnv1a_64(b""), 0xcbf2_9ce4_8422_2325)?;
    check("a", fnv1a_64(b"a"), 0xaf63_dc4c_8601_ec8c)?;
    check("foobar", fnv1a_64(b"foobar"), 0x8594_4171_f739_67e8)
}

// ═══════════════════════════ Entry points ═══════════════════════════

#[test]
fn parse_entry_points_match_load() -> TestResult {
    let (universal_src, snapshot) = universal()?;
    let (local_src, overlay) = local(&snapshot)?;
    let joined = Dti::from_parts(snapshot, overlay).map_err(|d| format!("{d:#?}"))?;
    let loaded = Dti::load(&universal_src, &local_src).map_err(|d| format!("{d:#?}"))?;
    check("joined view", &joined, &loaded)
}

#[test]
fn from_parts_cross_checks_the_snapshot_version() -> TestResult {
    let (_, snapshot) = universal()?;
    let (_, overlay) = local(&snapshot)?;
    let other = UniversalSnapshot::parse(&universal_source(43)?).map_err(|d| format!("{d:#?}"))?;
    match Dti::from_parts(other, overlay) {
        Ok(_) => Err("mismatched parts accepted".to_owned()),
        Err(diagnostics) => check(
            "diagnostic",
            diagnostics.first().map(|d| d.kind),
            Some(DtiErrorKind::VersionMismatch {
                snapshot: 43,
                overlay: 42,
            }),
        ),
    }
}

// ═══════════════════════════ Round trips ═══════════════════════════

#[test]
fn universal_round_trip_preserves_the_model() -> TestResult {
    let (source, snapshot) = universal()?;
    let hash = fnv1a_64(source.as_bytes());
    let bytes = snapshot.to_binary(hash);
    let (decoded, stored) = UniversalSnapshot::from_binary(&bytes).map_err(|e| e.to_string())?;
    check("hash", stored, hash)?;
    check("model", &decoded, &snapshot)?;
    // Insertion order of shared forms survives although it is not key order.
    check(
        "verdant order",
        decoded.lexicon().synonyms("verdant"),
        [key("zebra")?, key("green")?].as_slice(),
    )?;
    check(
        "display order",
        decoded.lexicon().displays("bright stone"),
        [key("emerald")?, key("greenemerald")?].as_slice(),
    )?;
    let red = decoded.concept(&key("red")?).ok_or("red missing")?;
    check("attribute count", red.attrs.len(), 3)
}

#[test]
fn local_round_trip_preserves_the_model() -> TestResult {
    let (_, snapshot) = universal()?;
    let (source, overlay) = local(&snapshot)?;
    let hash = fnv1a_64(source.as_bytes());
    let bytes = overlay.to_binary(hash);
    let (decoded, stored) =
        LocalOverlay::from_binary(&bytes, &snapshot).map_err(|e| e.to_string())?;
    check("hash", stored, hash)?;
    check("model", &decoded, &overlay)?;
    check(
        "uscio order",
        decoded.lexicon().synonyms("uscio"),
        [key("door")?, key("localsensor")?, key("carry")?].as_slice(),
    )
}

#[test]
fn serialization_is_deterministic() -> TestResult {
    let (source, snapshot) = universal()?;
    let hash = fnv1a_64(source.as_bytes());
    let first = snapshot.to_binary(hash);
    check("same model", &snapshot.to_binary(hash), &first)?;
    // Independently parsed models (different hash map states) encode identically.
    let (_, again) = universal()?;
    check("reparsed model", &again.to_binary(hash), &first)?;
    let (decoded, _) = UniversalSnapshot::from_binary(&first).map_err(|e| e.to_string())?;
    check("after reload", &decoded.to_binary(hash), &first)?;

    let (local_src, overlay) = local(&snapshot)?;
    let local_hash = fnv1a_64(local_src.as_bytes());
    let local_bytes = overlay.to_binary(local_hash);
    let (_, local_again) = local(&snapshot)?;
    check(
        "reparsed overlay",
        &local_again.to_binary(local_hash),
        &local_bytes,
    )?;
    let (decoded, _) =
        LocalOverlay::from_binary(&local_bytes, &snapshot).map_err(|e| e.to_string())?;
    check(
        "overlay after reload",
        &decoded.to_binary(local_hash),
        &local_bytes,
    )
}

// ═══════════════════════════ Cache helpers ═══════════════════════════

#[test]
fn cache_hit_returns_none() -> TestResult {
    let (source, snapshot) = universal()?;
    let bytes = snapshot.to_binary(fnv1a_64(source.as_bytes()));
    let (loaded, fresh) =
        load_or_compile_universal(&source, Some(&bytes)).map_err(|e| e.to_string())?;
    check("universal fresh", fresh, None)?;
    check("universal model", &loaded, &snapshot)?;

    let (local_src, overlay) = local(&snapshot)?;
    let local_bytes = overlay.to_binary(fnv1a_64(local_src.as_bytes()));
    let (loaded, fresh) = load_or_compile_local(&local_src, Some(&local_bytes), &snapshot)
        .map_err(|e| e.to_string())?;
    check("local fresh", fresh, None)?;
    check("local model", &loaded, &overlay)
}

#[test]
fn missing_or_stale_binary_is_recompiled() -> TestResult {
    let (source, snapshot) = universal()?;
    let expected = snapshot.to_binary(fnv1a_64(source.as_bytes()));
    let (_, fresh) = load_or_compile_universal(&source, None).map_err(|e| e.to_string())?;
    check("absent", fresh.as_ref(), Some(&expected))?;

    // A binary compiled from another source (different hash) is stale.
    let stale = snapshot.to_binary(fnv1a_64(b"older source"));
    let (loaded, fresh) =
        load_or_compile_universal(&source, Some(&stale)).map_err(|e| e.to_string())?;
    check("stale", fresh.as_ref(), Some(&expected))?;
    check("stale model", &loaded, &snapshot)?;

    let (local_src, overlay) = local(&snapshot)?;
    let local_expected = overlay.to_binary(fnv1a_64(local_src.as_bytes()));
    let local_stale = overlay.to_binary(0);
    let (_, fresh) = load_or_compile_local(&local_src, Some(&local_stale), &snapshot)
        .map_err(|e| e.to_string())?;
    check("local stale", fresh.as_ref(), Some(&local_expected))
}

#[test]
fn source_errors_are_reported() -> TestResult {
    match load_or_compile_universal("#awdti 1\n#role local\n", None) {
        Err(DtiLoadError::Source(diagnostics)) if !diagnostics.is_empty() => Ok(()),
        other => Err(format!("expected Source error, got {other:?}")),
    }
}

/// Minimal universal snapshot without numeral: the record count sits at a
/// fixed offset (21 header + 8 version + 4 + 2 `en` + 1 numeral flag = 36).
fn minimal_universal() -> Result<(String, Vec<u8>), String> {
    let source = format!(
        "#awdti 1\n#role universal\n#version 1\n#lang en\nrec {} door\n",
        k("door")?
    );
    let snapshot = UniversalSnapshot::parse(&source).map_err(|d| format!("{d:#?}"))?;
    let bytes = snapshot.to_binary(fnv1a_64(source.as_bytes()));
    Ok((source, bytes))
}

fn with_bytes(bytes: &[u8], at: usize, replacement: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    for (slot, byte) in out.iter_mut().skip(at).zip(replacement) {
        *slot = *byte;
    }
    out
}

/// Replaces the first occurrence of `from` with `to`.
fn splice(bytes: &[u8], from: &[u8], to: &[u8]) -> Result<Vec<u8>, String> {
    let at = bytes
        .windows(from.len())
        .position(|window| window == from)
        .ok_or("pattern not found")?;
    let (head, tail) = bytes.split_at_checked(at).ok_or("bad split")?;
    let tail = tail.get(from.len()..).ok_or("bad tail")?;
    Ok([head, to, tail].concat())
}

fn length_prefixed(text: &str) -> Result<Vec<u8>, String> {
    let len = u32::try_from(text.len()).map_err(|e| e.to_string())?;
    Ok([len.to_le_bytes().as_slice(), text.as_bytes()].concat())
}

#[test]
fn corrupt_binaries_report_specific_errors_and_are_recompiled() -> TestResult {
    let (source, good) = minimal_universal()?;
    let len = good.len();
    let cases: Vec<(&str, Vec<u8>, DtiLoadError)> = vec![
        ("empty", Vec::new(), DtiLoadError::Truncated { offset: 0 }),
        (
            "bad magic",
            with_bytes(&good, 0, b"XWDTIBIN"),
            DtiLoadError::BadMagic,
        ),
        (
            "unsupported version",
            with_bytes(&good, 8, &7_u32.to_le_bytes()),
            DtiLoadError::UnsupportedVersion { found: 7 },
        ),
        (
            "wrong kind",
            with_bytes(&good, 12, &[1]),
            DtiLoadError::WrongKind { found: 1 },
        ),
        (
            "truncated",
            good.get(..len.saturating_sub(3)).ok_or("short")?.to_vec(),
            DtiLoadError::Truncated {
                offset: len.saturating_sub(4),
            },
        ),
        (
            "trailing bytes",
            [good.as_slice(), &[0]].concat(),
            DtiLoadError::TrailingBytes { offset: len },
        ),
        (
            "invalid utf-8",
            with_bytes(&good, 33, &[0xFF]),
            DtiLoadError::InvalidUtf8 { offset: 33 },
        ),
        (
            "huge record count",
            with_bytes(&good, 36, &u32::MAX.to_le_bytes()),
            DtiLoadError::Truncated { offset: 36 },
        ),
        (
            "bad presence tag",
            with_bytes(&good, 35, &[9]),
            DtiLoadError::Malformed {
                offset: 35,
                what: "invalid presence tag",
            },
        ),
    ];
    for (what, bytes, want) in cases {
        expect_error(what, UniversalSnapshot::from_binary(&bytes), &want)?;
        let (_, fresh) =
            load_or_compile_universal(&source, Some(&bytes)).map_err(|e| e.to_string())?;
        check(what, fresh.as_ref(), Some(&good))?;
    }
    Ok(())
}

#[test]
fn local_binary_is_rejected_as_universal_and_vice_versa() -> TestResult {
    let (source, snapshot) = universal()?;
    let (local_src, overlay) = local(&snapshot)?;
    let local_bytes = overlay.to_binary(fnv1a_64(local_src.as_bytes()));
    let universal_bytes = snapshot.to_binary(fnv1a_64(source.as_bytes()));
    expect_error(
        "local as universal",
        UniversalSnapshot::from_binary(&local_bytes),
        &DtiLoadError::WrongKind { found: 1 },
    )?;
    expect_error(
        "universal as local",
        LocalOverlay::from_binary(&universal_bytes, &snapshot),
        &DtiLoadError::WrongKind { found: 0 },
    )
}

#[test]
fn injected_fields_are_rejected() -> TestResult {
    let (source, good) = minimal_universal()?;
    // No text is rebuilt: a primary form with a space is checked as a glyph
    // atom by the shared field validation and refused.
    let injected = splice(
        &good,
        &length_prefixed("door")?,
        &length_prefixed("door syn:hinge")?,
    )?;
    expect_error(
        "space injection",
        UniversalSnapshot::from_binary(&injected),
        &DtiLoadError::Invalid(vec![DtiDiagnostic {
            source: DtiSource::Universal,
            line: 1,
            kind: DtiErrorKind::InvalidForm,
        }]),
    )?;
    let (_, fresh) =
        load_or_compile_universal(&source, Some(&injected)).map_err(|e| e.to_string())?;
    check("recompiled", fresh.as_ref(), Some(&good))?;

    let broken = splice(
        &good,
        &length_prefixed("door")?,
        &length_prefixed("do\nor")?,
    )?;
    match UniversalSnapshot::from_binary(&broken) {
        Err(DtiLoadError::Malformed { what, .. }) => check("what", what, "line break in string"),
        other => Err(format!("line break accepted: {other:?}")),
    }
}

#[test]
fn local_binary_is_revalidated_against_the_snapshot() -> TestResult {
    let (_, snapshot) = universal()?;
    let (local_src, overlay) = local(&snapshot)?;
    let bytes = overlay.to_binary(fnv1a_64(local_src.as_bytes()));
    let other = UniversalSnapshot::parse(&universal_source(43)?).map_err(|d| format!("{d:#?}"))?;
    match LocalOverlay::from_binary(&bytes, &other) {
        Err(DtiLoadError::Invalid(diagnostics)) => check(
            "version mismatch",
            diagnostics
                .iter()
                .any(|d| matches!(d.kind, DtiErrorKind::VersionMismatch { .. })),
            true,
        ),
        other => Err(format!("expected Invalid, got {other:?}")),
    }
}

// ═══════════════════════ Shared model invariants ═══════════════════════

fn concept(key: DtiKey, base: Option<DtiKey>, superseded_by: Option<DtiKey>) -> ConceptRecord {
    ConceptRecord {
        key,
        base,
        param: None,
        morph: MorphFlags::default(),
        superseded_by,
        attrs: Box::new([]),
    }
}

fn entry(key: DtiKey, primary: &str) -> LexicalEntry {
    LexicalEntry {
        key,
        primary: Box::from(primary),
        display: None,
        synonyms: Box::new([]),
    }
}

/// Encodes a hand-built (possibly invalid) snapshot: the decoder must refuse it
/// with the same diagnostics as the text parser.
fn raw_universal(records: Vec<(ConceptRecord, LexicalEntry)>) -> Vec<u8> {
    let mut concepts = BTreeMap::new();
    let mut lexicon = Lexicon::default();
    for (record, entry) in records {
        concepts.insert(record.key, record);
        lexicon.insert(entry);
    }
    UniversalSnapshot {
        version: 1,
        language: Box::from("en"),
        numeral: None,
        concepts,
        lexicon,
    }
    .to_binary(0)
}

fn invalid(kind: DtiErrorKind, source: DtiSource, line: u32) -> DtiLoadError {
    DtiLoadError::Invalid(vec![DtiDiagnostic { source, line, kind }])
}

#[test]
fn decoder_applies_the_shared_model_invariants() -> TestResult {
    let (a, b, c) = (key("aa")?, key("bb")?, key("cc")?);
    // Atomic forms: `§` is refused in a primary form (QualifiedForm).
    expect_error(
        "qualified primary",
        UniversalSnapshot::from_binary(&raw_universal(vec![(
            concept(a, None, None),
            entry(a, "verde§smeraldo"),
        )])),
        &invalid(DtiErrorKind::QualifiedForm, DtiSource::Universal, 1),
    )?;
    // References must exist.
    expect_error(
        "orphan base",
        UniversalSnapshot::from_binary(&raw_universal(vec![(
            concept(a, Some(c), None),
            entry(a, "uno"),
        )])),
        &invalid(DtiErrorKind::OrphanReference(c), DtiSource::Universal, 1),
    )?;
    // No cycle in `base` chains.
    expect_error(
        "base cycle",
        UniversalSnapshot::from_binary(&raw_universal(vec![
            (concept(a, Some(b), None), entry(a, "uno")),
            (concept(b, Some(a), None), entry(b, "due")),
        ])),
        &DtiLoadError::Invalid(vec![
            DtiDiagnostic {
                source: DtiSource::Universal,
                line: 1,
                kind: DtiErrorKind::BaseCycle(a),
            },
            DtiDiagnostic {
                source: DtiSource::Universal,
                line: 2,
                kind: DtiErrorKind::BaseCycle(b),
            },
        ]),
    )?;
    // Local overlay: `use` of a deprecated key, `new` reusing a universal key.
    let universal_bytes = raw_universal(vec![
        (concept(a, None, None), entry(a, "uno")),
        (concept(b, None, Some(a)), entry(b, "due")),
    ]);
    let (universal, _) =
        UniversalSnapshot::from_binary(&universal_bytes).map_err(|e| e.to_string())?;
    let mut enabled = BTreeSet::new();
    enabled.insert(b);
    let deprecated = LocalOverlay {
        universal_version: 1,
        language: Box::from("it"),
        policy: UnknownKeyPolicy::DegradeToBase,
        enabled,
        local: BTreeMap::new(),
        lexicon: Lexicon::default(),
    }
    .to_binary(0);
    expect_error(
        "use of a deprecated key",
        LocalOverlay::from_binary(&deprecated, &universal),
        &invalid(
            DtiErrorKind::SupersededKey {
                key: b,
                successor: a,
            },
            DtiSource::Local,
            1,
        ),
    )?;
    let mut local = BTreeMap::new();
    local.insert(a, concept(a, None, None));
    let mut lexicon = Lexicon::default();
    lexicon.insert(entry(a, "spoof"));
    let spoof = LocalOverlay {
        universal_version: 1,
        language: Box::from("it"),
        policy: UnknownKeyPolicy::DegradeToBase,
        enabled: BTreeSet::new(),
        local,
        lexicon,
    }
    .to_binary(0);
    expect_error(
        "anti-spoofing",
        LocalOverlay::from_binary(&spoof, &universal),
        &invalid(DtiErrorKind::LocalShadowsUniversal(a), DtiSource::Local, 1),
    )
}
