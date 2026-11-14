//! B6/B5 tests: `.awpolicy` loading, the 5 filters, Legacy Shield.

use crate::dti::{Dti, DtiKey};
use crate::session::{PeriodId, Session};
use crate::shield::{LegacyShield, ShieldViolation};

use super::{DtiPolicyManager, PolicyErrorKind, PolicyRule, Requirement};

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

const TAGS: [(&str, &str, &str); 9] = [
    ("open", "open", "aprire"),
    ("read", "read", "leggere"),
    ("operator", "operator", "operatore"),
    ("valve", "valve", "valvola"),
    ("door", "door", "porta"),
    ("temperature", "temperature", "temperatura"),
    ("priority", "priority", "priorita"),
    ("high", "high", "alta"),
    ("low", "low", "bassa"),
];

fn dti() -> Result<Dti, String> {
    let digits: Vec<String> = (0..10)
        .map(|d| k(&format!("digit{d}")))
        .collect::<Result<_, _>>()?;
    let mut universal = format!(
        "#awdti 1\n#role universal\n#version 5\n#lang en\n#numeral base:10 digits:{}\n",
        digits.join(",")
    );
    let mut local = String::from("#awdti 1\n#role local\n#universal 5\n#lang it\n");
    for (d, key) in digits.iter().enumerate() {
        universal.push_str(&format!("rec {key} {d} morph:+AGGL_NUM\n"));
        local.push_str(&format!("use {key}\n"));
    }
    for (tag, en, it) in TAGS {
        // B9: every concept may be joined by `§` (qualifiers `valvola§priorita`).
        universal.push_str(&format!("rec {} {en} morph:+UNION\n", k(tag)?));
        local.push_str(&format!("use {} form:{it}\n", k(tag)?));
    }
    Dti::load(&universal, &local).map_err(|d| format!("{d:#?}"))
}

fn policy(dti: &Dti, default: &str, rules: &str) -> Result<DtiPolicyManager, String> {
    let src = format!("#awpolicy 1\n#universal 5\n#default {default}\n{rules}");
    DtiPolicyManager::load(&src, dti).map_err(|d| format!("{d:#?}"))
}

fn rules() -> Result<String, String> {
    Ok(format!(
        "signal {open} ta,ka\nrole {open} n:{operator} a:{valve},{door}\n\
signal {read} ta,quo\nrole {read} n:{operator} a:{temperature}\n\
threshold {read} cha min:0 max:100\nqualifier {valve} {priority},{high}\nrequire {open} zato\n\
signal {door} ta\nrole {door} n:{operator}\n",
        open = k("open")?,
        read = k("read")?,
        operator = k("operator")?,
        valve = k("valve")?,
        door = k("door")?,
        temperature = k("temperature")?,
        priority = k("priority")?,
        high = k("high")?,
    ))
}

fn ingest(session: &mut Session, src: &str) -> Result<PeriodId, String> {
    let ids = session
        .ingest(src)
        .map_err(|d| format!("rejected {src:?}: {d:#?}"))?;
    ids.last().copied().ok_or_else(|| "no period".to_owned())
}

fn violations(manager: &DtiPolicyManager, dti: &Dti, src: &str) -> Result<Vec<PolicyRule>, String> {
    let mut session = Session::new();
    let id = ingest(&mut session, src)?;
    Ok(match manager.check_period(&session, id, dti) {
        Ok(()) => Vec::new(),
        Err(found) => found.into_iter().map(|v| v.rule).collect(),
    })
}

fn check<T: PartialEq + core::fmt::Debug>(what: &str, got: T, want: T) -> TestResult {
    if got == want {
        Ok(())
    } else {
        Err(format!("{what}: got {got:?}, want {want:?}"))
    }
}

const OK_OPEN: &str = "/^ ta aprire nu operatore a valvola§priorita : tazato porta nu operatore ^/";

#[test]
fn accepted_command_passes_all_filters() -> TestResult {
    let dti = dti()?;
    let manager = policy(&dti, "deny", &rules()?)?;
    check(
        "no violations",
        violations(&manager, &dti, OK_OPEN)?,
        Vec::new(),
    )
}

#[test]
fn signal_mask_and_unauthorized_command() -> TestResult {
    let dti = dti()?;
    let manager = policy(&dti, "deny", &rules()?)?;
    check(
        "deep not in mask",
        violations(
            &manager,
            &dti,
            "/^ deep aprire nu operatore a valvola : tazato porta nu operatore ^/",
        )?
        .into_iter()
        .next(),
        Some(PolicyRule::SignalMask {
            glyph: key("open")?,
            head: "deep".to_owned(),
        }),
    )?;
    check(
        "unknown command under deny",
        violations(&manager, &dti, "/^ ta valvola ^/")?,
        vec![PolicyRule::UnauthorizedCommand {
            glyph: key("valve")?,
        }],
    )?;
    // A recalled sema stands for its signal glyph: `deep ~~` cannot bypass the mask.
    check(
        "deep ~~ is checked on the recalled signal glyph",
        violations(
            &manager,
            &dti,
            "/^ ta aprire nu operatore a valvola : tazato porta nu operatore y deep ~~ ^/",
        )?
        .contains(&PolicyRule::SignalMask {
            glyph: key("open")?,
            head: "deep".to_owned(),
        }),
        true,
    )?;
    let permissive = policy(&dti, "allow", "")?;
    check(
        "allow default",
        violations(&permissive, &dti, "/^ ta valvola ^/")?,
        Vec::new(),
    )
}

#[test]
fn role_matrix_thresholds_qualifiers_requirements() -> TestResult {
    let dti = dti()?;
    let manager = policy(&dti, "deny", &rules()?)?;
    check(
        "value not allowed in role",
        violations(
            &manager,
            &dti,
            "/^ ta aprire nu operatore a temperatura : tazato porta nu operatore ^/",
        )?,
        vec![PolicyRule::RoleValue {
            glyph: key("open")?,
            case: "a",
            value: key("temperature")?,
        }],
    )?;
    check(
        "case base not allowed",
        violations(
            &manager,
            &dti,
            "/^ ta aprire nu operatore d valvola : tazato porta nu operatore ^/",
        )?,
        vec![PolicyRule::RoleNotAllowed {
            glyph: key("open")?,
            case: "d",
        }],
    )?;
    check(
        "threshold exceeded",
        violations(&manager, &dti, "/^ tacha leggere nu operatore a 150 ^/")?,
        vec![PolicyRule::Threshold {
            glyph: key("read")?,
            comparative: "cha",
            value: 150,
        }],
    )?;
    check(
        "threshold respected",
        violations(&manager, &dti, "/^ tacha leggere nu operatore a 30 ^/")?,
        Vec::new(),
    )?;
    check(
        "qualifier not allowed",
        violations(
            &manager,
            &dti,
            "/^ ta aprire nu operatore a valvola§bassa : tazato porta nu operatore ^/",
        )?,
        vec![PolicyRule::Qualifier {
            root: key("valve")?,
            qualifier: key("low")?,
        }],
    )?;
    check(
        "missing conditional subordinate",
        violations(&manager, &dti, "/^ ta aprire nu operatore a valvola ^/")?,
        vec![PolicyRule::MissingSubordinate {
            glyph: key("open")?,
            requirement: Requirement::Conditional,
        }],
    )
}

#[test]
fn enhanced_security_and_variables() -> TestResult {
    let dti = dti()?;
    let permissive = policy(&dti, "allow", "")?;
    let label = "/^ ta porta nu \"Xcvb124\" ^/";
    check(
        "raw allowed",
        violations(&permissive, &dti, label)?,
        Vec::new(),
    )?;
    let enhanced = policy(&dti, "allow", "")?.with_enhanced_security(true);
    check(
        "raw blocked",
        violations(&enhanced, &dti, label)?,
        vec![PolicyRule::RawLabel],
    )?;
    let allow_door =
        policy(&dti, "allow", &format!("raw allow:{}\n", k("door")?))?.with_enhanced_security(true);
    check(
        "raw allowed for door",
        violations(&allow_door, &dti, label)?,
        Vec::new(),
    )?;
    // Values reached through `def` variables are checked as well.
    let manager = policy(&dti, "deny", &rules()?)?;
    check(
        "variable value",
        violations(
            &manager,
            &dti,
            "/^ def obj a temperatura y ta aprire nu operatore a obj : tazato porta nu operatore ^/",
        )?,
        vec![PolicyRule::RoleValue {
            glyph: key("open")?,
            case: "a",
            value: key("temperature")?,
        }],
    )?;
    check(
        "unresolved glyph",
        violations(&permissive, &dti, "/^ ta porta nu sconosciuto ^/")?,
        vec![PolicyRule::UnresolvedGlyph],
    )
}

#[test]
fn policy_file_errors() -> TestResult {
    let dti = dti()?;
    let expect = |src: String, kind: PolicyErrorKind| -> TestResult {
        match DtiPolicyManager::load(&src, &dti) {
            Ok(_) => Err(format!("accepted, expected {kind:?}")),
            Err(d) if d.iter().any(|x| x.kind == kind) => Ok(()),
            Err(d) => Err(format!("expected {kind:?}, got {d:#?}")),
        }
    };
    expect(
        "#awpolicy 1\n#universal 5\n".to_owned(),
        PolicyErrorKind::MissingHeader { name: "default" },
    )?;
    expect(
        "#awpolicy 1\n#universal 4\n#default deny\n".to_owned(),
        PolicyErrorKind::VersionMismatch {
            snapshot: 5,
            policy: 4,
        },
    )?;
    expect(
        format!(
            "#awpolicy 1\n#universal 5\n#default deny\nsignal {} ta,xx\n",
            k("open")?
        ),
        PolicyErrorKind::UnknownSignalHead,
    )?;
    expect(
        format!(
            "#awpolicy 1\n#universal 5\n#default deny\nsignal {} ta\n",
            k("ghost")?
        ),
        PolicyErrorKind::UnknownKey(key("ghost")?),
    )?;
    expect(
        format!(
            "#awpolicy 1\n#universal 5\n#default deny\nthreshold {} cup min:1 max:2\n",
            k("read")?
        ),
        PolicyErrorKind::InvalidComparative,
    )?;
    expect(
        format!(
            "#awpolicy 1\n#universal 5\n#default deny\nthreshold {} cha min:5 max:2\n",
            k("read")?
        ),
        PolicyErrorKind::InvalidRange,
    )?;
    expect(
        format!(
            "#awpolicy 1\n#universal 5\n#default deny\nsignal {o} ta\nsignal {o} ka\n",
            o = k("open")?
        ),
        PolicyErrorKind::DuplicateRule,
    )?;
    expect(
        "#awpolicy 1\n#universal 5\n#default deny\nexecute all\n".to_owned(),
        PolicyErrorKind::UnknownRule,
    )
}

// ═══════════════════════════ Legacy Shield (B5) ═══════════════════════════

#[test]
fn shield_forwards_canonical_form_and_blocks_violations() -> TestResult {
    let dti = dti()?;
    let manager = policy(&dti, "deny", &rules()?)?;
    let mut shield = LegacyShield::new(dti, manager);
    check(
        "canonical reconstruction",
        shield.filter(OK_OPEN),
        Ok(OK_OPEN.to_owned()),
    )?;
    let periods = shield.session().periods().len();
    match shield.filter("/^ ta aprire nu operatore a temperatura : tazato porta nu operatore ^/") {
        Err(found) if matches!(found.first(), Some(ShieldViolation::Policy(_))) => {}
        other => return Err(format!("expected a policy block, got {other:?}")),
    }
    check(
        "session untouched after a block",
        shield.session().periods().len(),
        periods,
    )?;
    match shield.filter("/^ Ta aprire ^/") {
        Err(found) if matches!(found.first(), Some(ShieldViolation::Grammar(_))) => {}
        other => return Err(format!("expected a grammar block, got {other:?}")),
    }
    // Enhanced Security is forced on: raw labels are blocked.
    match shield.filter("/^ ta aprire nu \"rm\" a valvola : tazato porta nu operatore ^/") {
        Err(found)
            if found.iter().any(
                |v| matches!(v, ShieldViolation::Policy(p) if p.rule == PolicyRule::RawLabel),
            ) => {}
        other => return Err(format!("expected a raw-label block, got {other:?}")),
    }
    // A blocked `def` does not survive in the shield session.
    let _ = shield.filter("/^ defu x a valvola y ta valvola ^/");
    check(
        "no variable leaked",
        shield.session().live_variable("x").is_none(),
        true,
    )
}
