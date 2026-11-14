//! DTI Policy Manager (B6, P1–P5): mandatory access control on keys, O(1) lookups.
//!
//! Policies are written on DTI keys (language independent) in a `.awpolicy` file:
//! ```text
//! #awpolicy 1
//! #universal <snapshot version>
//! #default allow|deny
//! signal <KEY> ta,ka,deep                 Signal Mask for the signal glyph <KEY>
//! role <KEY> n:<K>,<K> a:<K> d:… g:… ab:…  Role Matrix (case base → allowed glyph keys)
//! threshold <KEY> cha min:0 max:100        Physical threshold (cha | chu | col)
//! qualifier <KEY> <K>,<K>                  Allowed `§` qualifiers after root <KEY>
//! require <KEY> zato|quo                   Mandatory subordinate (conditional | quo)
//! raw deny | raw allow:<K>,<K>             Enhanced Security (raw labels)
//! ```
//! Every rule refers to the **signal glyph** (the glyph of the signal pair, P1),
//! except `qualifier`, which applies to any glyph path with that root.
//! `#default deny` makes the policy a whitelist: a signal glyph without a
//! `signal` rule is an unauthorized command and case bases missing from its
//! `role` rule are unauthorized roles. With `#default allow`, only the listed
//! rules restrict. Policies run after stage 3 (glyph → key resolution, P3).

use crate::prelude::*;

use alloc::collections::{BTreeMap, BTreeSet};
use core::fmt;

use crate::analyze::{ClauseId, GlobalClause, SlotIndex, SlotValue};
use crate::ast::{
    Case, ClauseRef, Comparative, GlyphSlot, RelationalBlock, RelationalSignal, SecondarySignal,
    TemporalMode,
};
use crate::dti::{Dti, DtiKey};
use crate::session::{PeriodId, Session};
use crate::span::Span;
use crate::stage3::{resolve_path, IndexedAtom, PathResolution};

const FORMAT_VERSION: &str = "1";

// ═══════════════════════════ Rules ═══════════════════════════

/// Whitelist or blacklist stance of the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDefault {
    Allow,
    Deny,
}

/// Mandatory subordinate kinds (P2 `require`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Requirement {
    /// A child clause with a written conditional determiner (`zato`, `zalt`, …).
    Conditional,
    /// A child clause with the secondary signal `quo` (confirmation).
    Quo,
}

/// Enhanced Security rule on raw labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawRule {
    Deny,
    /// Raw labels allowed only in clauses whose signal glyph is listed.
    Allow(BTreeSet<DtiKey>),
}

/// Inclusive numeric range of a threshold rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub min: u64,
    pub max: u64,
}

/// Case bases addressed by the role matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CaseBase {
    N,
    A,
    D,
    G,
    Ab,
}

impl CaseBase {
    const fn of(case: Case) -> Self {
        match case {
            Case::Nominative => Self::N,
            Case::Accusative => Self::A,
            Case::Dative(_) => Self::D,
            Case::Genitive(_) => Self::G,
            Case::Ablative(_) => Self::Ab,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::N => "n",
            Self::A => "a",
            Self::D => "d",
            Self::G => "g",
            Self::Ab => "ab",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "n" => Some(Self::N),
            "a" => Some(Self::A),
            "d" => Some(Self::D),
            "g" => Some(Self::G),
            "ab" => Some(Self::Ab),
            _ => None,
        }
    }
}

/// The 5 mandatory filters (B6).
#[derive(Debug)]
pub struct DtiPolicyManager {
    universal_version: u64,
    default: PolicyDefault,
    signals: BTreeMap<DtiKey, BTreeSet<Box<str>>>,
    roles: BTreeMap<DtiKey, BTreeMap<CaseBase, BTreeSet<DtiKey>>>,
    thresholds: BTreeMap<(DtiKey, Comparative), Range>,
    qualifiers: BTreeMap<DtiKey, BTreeSet<DtiKey>>,
    requires: BTreeMap<DtiKey, Vec<Requirement>>,
    raw: Option<RawRule>,
    enhanced_security: bool,
}

// ═══════════════════════════ Diagnostics ═══════════════════════════

/// Errors in a `.awpolicy` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyErrorKind {
    MissingHeader {
        name: &'static str,
    },
    DuplicateHeader {
        name: &'static str,
    },
    InvalidHeader,
    UnsupportedFormatVersion,
    VersionMismatch {
        snapshot: u64,
        policy: u64,
    },
    HeaderAfterRules,
    UnknownRule,
    MalformedRule,
    InvalidKey,
    /// The key does not exist in the loaded DTI.
    UnknownKey(DtiKey),
    UnknownSignalHead,
    UnknownCaseBase,
    InvalidComparative,
    InvalidRange,
    UnknownRequirement,
    DuplicateRule,
}

impl fmt::Display for PolicyErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeader { name } => write!(f, "missing header #{name}"),
            Self::DuplicateHeader { name } => write!(f, "duplicate header #{name}"),
            Self::InvalidHeader => f.write_str("invalid header directive"),
            Self::UnsupportedFormatVersion => f.write_str("unsupported #awpolicy version"),
            Self::VersionMismatch { snapshot, policy } => write!(
                f,
                "policy written for snapshot version {policy}, loaded snapshot is version {snapshot}"
            ),
            Self::HeaderAfterRules => f.write_str("header directive after the first rule"),
            Self::UnknownRule => f.write_str("unknown rule kind"),
            Self::MalformedRule => f.write_str("malformed rule"),
            Self::InvalidKey => f.write_str("invalid DTI key"),
            Self::UnknownKey(key) => write!(f, "key {key} is not in the DTI"),
            Self::UnknownSignalHead => f.write_str("unknown signal head"),
            Self::UnknownCaseBase => f.write_str("unknown case base (expected n, a, d, g, ab)"),
            Self::InvalidComparative => f.write_str("threshold comparative must be cha, chu or col"),
            Self::InvalidRange => f.write_str("invalid range (expected min:<u64> max:<u64>, min ≤ max)"),
            Self::UnknownRequirement => f.write_str("unknown requirement (expected zato or quo)"),
            Self::DuplicateRule => f.write_str("duplicate rule"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyDiagnostic {
    pub line: u32,
    pub kind: PolicyErrorKind,
}

impl fmt::Display for PolicyDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[policy:{}] {}", self.line, self.kind)
    }
}

impl core::error::Error for PolicyDiagnostic {}

/// Which rule a violation breaks (P4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyRule {
    /// `#default deny` and no `signal` rule for the signal glyph.
    UnauthorizedCommand { glyph: DtiKey },
    /// Signal head not in the Signal Mask.
    SignalMask { glyph: DtiKey, head: String },
    /// Case base not authorized for the signal glyph.
    RoleNotAllowed { glyph: DtiKey, case: &'static str },
    /// Glyph not authorized in that role.
    RoleValue {
        glyph: DtiKey,
        case: &'static str,
        value: DtiKey,
    },
    /// Accusative value outside the physical threshold.
    Threshold {
        glyph: DtiKey,
        comparative: &'static str,
        value: u64,
    },
    /// Accusative value of a threshold rule is not a numeral.
    NotANumeral { glyph: DtiKey },
    /// Qualifier not allowed after the root.
    Qualifier { root: DtiKey, qualifier: DtiKey },
    /// Mandatory subordinate missing.
    MissingSubordinate {
        glyph: DtiKey,
        requirement: Requirement,
    },
    /// Raw label blocked by Enhanced Security.
    RawLabel,
    /// A glyph that cannot be resolved to a DTI key.
    UnresolvedGlyph,
}

/// Positioned policy violation (P4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyViolation {
    pub rule: PolicyRule,
    pub span: Span,
}

impl fmt::Display for PolicyViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[policy] byte {}..{}: {:?}",
            self.span.start, self.span.end, self.rule
        )
    }
}

impl core::error::Error for PolicyViolation {}

// ═══════════════════════════ Loading ═══════════════════════════

const SIGNAL_HEADS: [&str; 11] = [
    "ta", "to", "ka", "ko", "ot", "oh", "qua", "que", "quo", "quu", "deep",
];

struct Loader<'d> {
    dti: &'d Dti,
    diagnostics: Vec<PolicyDiagnostic>,
}

impl Loader<'_> {
    fn report(&mut self, line: u32, kind: PolicyErrorKind) {
        self.diagnostics.push(PolicyDiagnostic { line, kind });
    }

    fn key(&self, text: &str) -> Result<DtiKey, PolicyErrorKind> {
        let key = DtiKey::parse(text).map_err(|_| PolicyErrorKind::InvalidKey)?;
        if self.dti.concept(&key).is_some() {
            Ok(key)
        } else {
            Err(PolicyErrorKind::UnknownKey(key))
        }
    }

    fn keys(&self, list: &str) -> Result<BTreeSet<DtiKey>, PolicyErrorKind> {
        list.split(',').map(|k| self.key(k)).collect()
    }
}

#[derive(Default)]
struct Headers {
    format: Option<()>,
    universal: Option<u64>,
    default: Option<PolicyDefault>,
}

fn set_once<T>(slot: &mut Option<T>, value: T, name: &'static str) -> Result<(), PolicyErrorKind> {
    if slot.is_some() {
        return Err(PolicyErrorKind::DuplicateHeader { name });
    }
    *slot = Some(value);
    Ok(())
}

fn parse_header(headers: &mut Headers, line: &str) -> Result<(), PolicyErrorKind> {
    let directive = line.strip_prefix('#').unwrap_or(line);
    let (name, value) = directive
        .split_once(' ')
        .ok_or(PolicyErrorKind::InvalidHeader)?;
    match name {
        "awpolicy" => {
            if value != FORMAT_VERSION {
                return Err(PolicyErrorKind::UnsupportedFormatVersion);
            }
            set_once(&mut headers.format, (), "awpolicy")
        }
        "universal" => {
            let version = value.parse().map_err(|_| PolicyErrorKind::InvalidHeader)?;
            set_once(&mut headers.universal, version, "universal")
        }
        "default" => {
            let default = match value {
                "allow" => PolicyDefault::Allow,
                "deny" => PolicyDefault::Deny,
                _ => return Err(PolicyErrorKind::InvalidHeader),
            };
            set_once(&mut headers.default, default, "default")
        }
        _ => Err(PolicyErrorKind::InvalidHeader),
    }
}

fn parse_range(min: &str, max: &str) -> Result<Range, PolicyErrorKind> {
    let min: u64 = min
        .strip_prefix("min:")
        .and_then(|v| v.parse().ok())
        .ok_or(PolicyErrorKind::InvalidRange)?;
    let max: u64 = max
        .strip_prefix("max:")
        .and_then(|v| v.parse().ok())
        .ok_or(PolicyErrorKind::InvalidRange)?;
    if min <= max {
        Ok(Range { min, max })
    } else {
        Err(PolicyErrorKind::InvalidRange)
    }
}

impl DtiPolicyManager {
    /// Parses and validates a `.awpolicy` source against the loaded DTI.
    pub fn load(src: &str, dti: &Dti) -> Result<Self, Vec<PolicyDiagnostic>> {
        let mut loader = Loader {
            dti,
            diagnostics: Vec::new(),
        };
        let mut headers = Headers::default();
        let mut policy = Self {
            universal_version: 0,
            default: PolicyDefault::Allow,
            signals: BTreeMap::new(),
            roles: BTreeMap::new(),
            thresholds: BTreeMap::new(),
            qualifiers: BTreeMap::new(),
            requires: BTreeMap::new(),
            raw: None,
            enhanced_security: false,
        };
        let mut rules_started = false;
        for (index, raw_line) in src.split('\n').enumerate() {
            let line_number = u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX);
            let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            if line.is_empty() {
                continue;
            }
            if line.starts_with('#') {
                if rules_started {
                    loader.report(line_number, PolicyErrorKind::HeaderAfterRules);
                } else if let Err(kind) = parse_header(&mut headers, line) {
                    loader.report(line_number, kind);
                }
                continue;
            }
            rules_started = true;
            if let Err(kind) = policy.parse_rule(&loader, line) {
                loader.report(line_number, kind);
            }
        }
        for (present, name) in [
            (headers.format.is_some(), "awpolicy"),
            (headers.universal.is_some(), "universal"),
            (headers.default.is_some(), "default"),
        ] {
            if !present {
                loader.report(0, PolicyErrorKind::MissingHeader { name });
            }
        }
        if let Some(version) = headers.universal {
            policy.universal_version = version;
            if version != dti.universal().version() {
                loader.report(
                    0,
                    PolicyErrorKind::VersionMismatch {
                        snapshot: dti.universal().version(),
                        policy: version,
                    },
                );
            }
        }
        policy.default = headers.default.unwrap_or(PolicyDefault::Deny);
        if loader.diagnostics.is_empty() {
            Ok(policy)
        } else {
            loader.diagnostics.sort_by_key(|d| d.line);
            Err(loader.diagnostics)
        }
    }

    fn parse_rule(&mut self, loader: &Loader<'_>, line: &str) -> Result<(), PolicyErrorKind> {
        let fields: Vec<&str> = line.split(' ').collect();
        match fields.as_slice() {
            ["signal", key, heads] => {
                let key = loader.key(key)?;
                let mut set = BTreeSet::new();
                for head in heads.split(',') {
                    if !SIGNAL_HEADS.contains(&head) {
                        return Err(PolicyErrorKind::UnknownSignalHead);
                    }
                    set.insert(Box::from(head));
                }
                insert_unique(&mut self.signals, key, set)
            }
            ["role", key, cases @ ..] if !cases.is_empty() => {
                let key = loader.key(key)?;
                let mut matrix: BTreeMap<CaseBase, BTreeSet<DtiKey>> = BTreeMap::new();
                for entry in cases {
                    let (base, list) = entry
                        .split_once(':')
                        .ok_or(PolicyErrorKind::MalformedRule)?;
                    let base = CaseBase::parse(base).ok_or(PolicyErrorKind::UnknownCaseBase)?;
                    if matrix.insert(base, loader.keys(list)?).is_some() {
                        return Err(PolicyErrorKind::DuplicateRule);
                    }
                }
                insert_unique(&mut self.roles, key, matrix)
            }
            ["threshold", key, comparative, min, max] => {
                let key = loader.key(key)?;
                let comparative = match *comparative {
                    "cha" => Comparative::Cha,
                    "chu" => Comparative::Chu,
                    "col" => Comparative::Col,
                    _ => return Err(PolicyErrorKind::InvalidComparative),
                };
                insert_unique(
                    &mut self.thresholds,
                    (key, comparative),
                    parse_range(min, max)?,
                )
            }
            ["qualifier", key, list] => {
                let key = loader.key(key)?;
                let set = loader.keys(list)?;
                insert_unique(&mut self.qualifiers, key, set)
            }
            ["require", key, requirement] => {
                let key = loader.key(key)?;
                let requirement = match *requirement {
                    "zato" => Requirement::Conditional,
                    "quo" => Requirement::Quo,
                    _ => return Err(PolicyErrorKind::UnknownRequirement),
                };
                let list = self.requires.entry(key).or_default();
                if list.contains(&requirement) {
                    return Err(PolicyErrorKind::DuplicateRule);
                }
                list.push(requirement);
                Ok(())
            }
            ["raw", rule] => {
                if self.raw.is_some() {
                    return Err(PolicyErrorKind::DuplicateRule);
                }
                self.raw = Some(if *rule == "deny" {
                    RawRule::Deny
                } else {
                    let list = rule
                        .strip_prefix("allow:")
                        .ok_or(PolicyErrorKind::MalformedRule)?;
                    RawRule::Allow(loader.keys(list)?)
                });
                Ok(())
            }
            [kind, ..]
                if matches!(
                    *kind,
                    "signal" | "role" | "threshold" | "qualifier" | "require" | "raw"
                ) =>
            {
                Err(PolicyErrorKind::MalformedRule)
            }
            _ => Err(PolicyErrorKind::UnknownRule),
        }
    }

    /// Enables Enhanced Security: without an explicit `raw` rule, raw labels are denied.
    pub fn with_enhanced_security(mut self, enabled: bool) -> Self {
        self.enhanced_security = enabled;
        self
    }

    pub const fn enhanced_security(&self) -> bool {
        self.enhanced_security
    }

    pub const fn default_stance(&self) -> PolicyDefault {
        self.default
    }

    pub const fn universal_version(&self) -> u64 {
        self.universal_version
    }
}

fn insert_unique<K: Ord, V>(
    map: &mut BTreeMap<K, V>,
    key: K,
    value: V,
) -> Result<(), PolicyErrorKind> {
    if map.contains_key(&key) {
        return Err(PolicyErrorKind::DuplicateRule);
    }
    map.insert(key, value);
    Ok(())
}

// ═══════════════════════════ Checking ═══════════════════════════

/// Resolved content of a glyph position, as seen by the policy.
enum Seen {
    /// Concept key, plus the keys of its qualifiers when the path is not a record.
    Concept {
        key: DtiKey,
        root_qualifiers: Vec<DtiKey>,
    },
    Numeral(u64),
    Raw,
    /// Structural content that carries no key (numeral with qualifiers).
    Structural,
    Unresolved,
}

struct Checker<'a> {
    policy: &'a DtiPolicyManager,
    session: &'a Session,
    dti: &'a Dti,
    violations: Vec<PolicyViolation>,
}

fn numeral_value(digits: &str, base: u8) -> Option<u64> {
    digits.bytes().try_fold(0_u64, |acc, byte| {
        let digit = u64::from(byte.checked_sub(b'0')?);
        if digit >= u64::from(base) {
            return None;
        }
        acc.checked_mul(u64::from(base))?.checked_add(digit)
    })
}

impl Checker<'_> {
    fn violation(&mut self, rule: PolicyRule, span: Span) {
        self.violations.push(PolicyViolation { rule, span });
    }

    /// Every value of a slot (variables contribute all their values).
    fn seen(&self, value: SlotValue) -> Vec<Seen> {
        match value {
            SlotValue::Glyph(slot) => {
                let Some(GlyphSlot::Glyph(path)) = self.session.glyph_slot(slot) else {
                    return vec![Seen::Unresolved];
                };
                match resolve_path(path, self.session.interner(), self.dti) {
                    Ok(PathResolution::Record(key)) => vec![Seen::Concept {
                        key,
                        root_qualifiers: Vec::new(),
                    }],
                    Ok(PathResolution::Segments(atoms)) => {
                        let mut iter = atoms.iter();
                        match iter.next() {
                            Some(IndexedAtom::Key(key)) => vec![Seen::Concept {
                                key: *key,
                                root_qualifiers: iter
                                    .filter_map(|a| match a {
                                        IndexedAtom::Key(k) => Some(*k),
                                        IndexedAtom::Numeral(_) => None,
                                    })
                                    .collect(),
                            }],
                            Some(IndexedAtom::Numeral(digits)) if atoms.len() == 1 => {
                                let base = self.dti.universal().numeral().map_or(10, |n| n.base);
                                numeral_value(digits, base)
                                    .map_or(vec![Seen::Unresolved], |v| vec![Seen::Numeral(v)])
                            }
                            // Numeral root with qualifiers (e.g. `30§celsius`).
                            Some(IndexedAtom::Numeral(_)) => vec![Seen::Structural],
                            None => vec![Seen::Unresolved],
                        }
                    }
                    Err(_) => vec![Seen::Unresolved],
                }
            }
            SlotValue::Raw(_) => vec![Seen::Raw],
            SlotValue::Variable(binding) => self
                .session
                .binding(binding)
                .map(|b| {
                    b.values()
                        .iter()
                        .flat_map(|v| match v {
                            SlotValue::Variable(_) => vec![Seen::Unresolved],
                            other => self.seen(*other),
                        })
                        .collect()
                })
                .unwrap_or_else(|| vec![Seen::Unresolved]),
            // A sema stands for its own signal glyph (`<glyph>~~`): the policy checks
            // the signal glyph of the recalled sema in every slot, so that `deep ~~`
            // cannot bypass the Signal Mask.
            SlotValue::Sema(antecedent) => match self.sema_signal_value(antecedent) {
                Some(value) => self.seen(value),
                None => vec![Seen::Unresolved],
            },
        }
    }

    /// Value of the signal glyph of a recalled sema; when that glyph is itself a
    /// sema anaphora the chain is followed (every antecedent precedes its host, so
    /// the chain strictly moves back in the session and terminates).
    fn sema_signal_value(&self, antecedent: GlobalClause) -> Option<SlotValue> {
        let mut current = antecedent;
        loop {
            let value = self
                .session
                .analysis(current.period)
                .and_then(|period| period.clause(current.clause))
                .and_then(|analysis| analysis.slot(SlotIndex::SIGNAL))
                .map(|slot| slot.value())?;
            match value {
                SlotValue::Sema(next)
                    if (next.period.index(), next.clause.index())
                        < (current.period.index(), current.clause.index()) =>
                {
                    current = next;
                }
                SlotValue::Sema(_) => return None,
                other => return Some(other),
            }
        }
    }

    fn signal_heads(signal: &RelationalSignal) -> Vec<&'static str> {
        match signal {
            RelationalSignal::Primary { signal, .. } => vec![signal.surface()],
            RelationalSignal::Secondary {
                secondary, primary, ..
            } => vec![secondary.surface(), primary.surface()],
            RelationalSignal::Deep { .. } => vec!["deep"],
        }
    }

    fn check_clause(&mut self, id: PeriodId, clause: ClauseId, block: &RelationalBlock) {
        let Some(analysis) = self
            .session
            .analysis(id)
            .and_then(|period| period.clause(clause))
        else {
            return;
        };
        let signal_span = block.signal.tile.span;
        // Signal glyph (P1): the resolved glyph of the signal pair.
        let signal_seen = analysis
            .slot(SlotIndex::SIGNAL)
            .map(|slot| self.seen(slot.value()))
            .unwrap_or_default();
        let signal_key = signal_seen.iter().find_map(|s| match s {
            Seen::Concept { key, .. } => Some(*key),
            _ => None,
        });

        // Filter 5 (Enhanced Security) and generic checks on every pair.
        let raw_allowed = match &self.policy.raw {
            Some(RawRule::Deny) => false,
            Some(RawRule::Allow(keys)) => signal_key.is_some_and(|k| keys.contains(&k)),
            None => !self.policy.enhanced_security && self.policy.default == PolicyDefault::Allow,
        };
        let pair_count = u32::try_from(block.pair_count()).unwrap_or(0);
        for position in 0..pair_count {
            let span = block.glyph(position).map_or(signal_span, |g| g.span);
            let Some(slot) = analysis.slot(SlotIndex::new(position)) else {
                continue;
            };
            for seen in self.seen(slot.value()) {
                match seen {
                    Seen::Raw if !raw_allowed => self.violation(PolicyRule::RawLabel, span),
                    Seen::Unresolved => self.violation(PolicyRule::UnresolvedGlyph, span),
                    // Filter 3a: qualifier limits.
                    Seen::Concept {
                        key,
                        root_qualifiers,
                    } => {
                        if let Some(allowed) = self.policy.qualifiers.get(&key) {
                            for qualifier in root_qualifiers {
                                if !allowed.contains(&qualifier) {
                                    self.violation(
                                        PolicyRule::Qualifier {
                                            root: key,
                                            qualifier,
                                        },
                                        span,
                                    );
                                }
                            }
                        }
                    }
                    Seen::Raw | Seen::Numeral(_) | Seen::Structural => {}
                }
            }
        }

        let Some(glyph) = signal_key else {
            return;
        };
        // Filter 1: Signal Mask.
        match self.policy.signals.get(&glyph) {
            Some(mask) => {
                for head in Self::signal_heads(&block.signal.tile.node) {
                    if !mask.contains(head) {
                        self.violation(
                            PolicyRule::SignalMask {
                                glyph,
                                head: head.to_owned(),
                            },
                            signal_span,
                        );
                    }
                }
            }
            None if self.policy.default == PolicyDefault::Deny => {
                self.violation(PolicyRule::UnauthorizedCommand { glyph }, signal_span);
            }
            None => {}
        }
        // Filter 2: Role Matrix.
        let matrix = self.policy.roles.get(&glyph);
        for (offset, pair) in block.cases.iter().enumerate() {
            let base = CaseBase::of(pair.tile.node.case);
            let position = u32::try_from(offset.saturating_add(1)).unwrap_or(u32::MAX);
            let allowed = match matrix {
                Some(matrix) => matrix.get(&base),
                None if self.policy.default == PolicyDefault::Deny => None,
                None => continue,
            };
            let Some(allowed) = allowed else {
                self.violation(
                    PolicyRule::RoleNotAllowed {
                        glyph,
                        case: base.name(),
                    },
                    pair.tile.span,
                );
                continue;
            };
            let Some(slot) = analysis.slot(SlotIndex::new(position)) else {
                continue;
            };
            for seen in self.seen(slot.value()) {
                if let Seen::Concept { key, .. } = seen {
                    if !allowed.contains(&key) {
                        self.violation(
                            PolicyRule::RoleValue {
                                glyph,
                                case: base.name(),
                                value: key,
                            },
                            pair.glyph.span,
                        );
                    }
                }
            }
        }
        // Filter 3b: physical thresholds (P5).
        if let Some(comparative) = analysis.signal().and_then(|s| s.comparative) {
            if let Some(range) = self.policy.thresholds.get(&(glyph, comparative)) {
                for (offset, pair) in block.cases.iter().enumerate() {
                    if pair.tile.node.case != Case::Accusative {
                        continue;
                    }
                    let position = u32::try_from(offset.saturating_add(1)).unwrap_or(u32::MAX);
                    let Some(slot) = analysis.slot(SlotIndex::new(position)) else {
                        continue;
                    };
                    for seen in self.seen(slot.value()) {
                        match seen {
                            Seen::Numeral(value) if value < range.min || value > range.max => self
                                .violation(
                                    PolicyRule::Threshold {
                                        glyph,
                                        comparative: comparative.surface(),
                                        value,
                                    },
                                    pair.glyph.span,
                                ),
                            Seen::Numeral(_) => {}
                            _ => self.violation(PolicyRule::NotANumeral { glyph }, pair.glyph.span),
                        }
                    }
                }
            }
        }
        // Filter 4: mandatory subordinate.
        if let Some(requirements) = self.policy.requires.get(&glyph) {
            let period = self.session.period(id);
            for requirement in requirements {
                let satisfied = analysis.children().iter().any(|child| {
                    let block = period.and_then(|p| p.clause(child.index()));
                    let Some(ClauseRef::Relational(block)) = block else {
                        return false;
                    };
                    match requirement {
                        Requirement::Conditional => matches!(
                            block.signal.tile.node.determiners().tense,
                            Some(TemporalMode::Conditional(_))
                        ),
                        Requirement::Quo => matches!(
                            block.signal.tile.node,
                            RelationalSignal::Secondary {
                                secondary: SecondarySignal::Quo,
                                ..
                            }
                        ),
                    }
                });
                if !satisfied {
                    self.violation(
                        PolicyRule::MissingSubordinate {
                            glyph,
                            requirement: *requirement,
                        },
                        signal_span,
                    );
                }
            }
        }
    }
}

impl DtiPolicyManager {
    /// Checks every relational clause of an analyzed period (all violations are
    /// collected). `def` and portability blocks are not commands: their effects
    /// are checked through the resolved values of the relational clauses.
    pub fn check_period(
        &self,
        session: &Session,
        id: PeriodId,
        dti: &Dti,
    ) -> Result<(), Vec<PolicyViolation>> {
        let Some(period) = session.period(id) else {
            return Err(vec![PolicyViolation {
                rule: PolicyRule::UnresolvedGlyph,
                span: Span::new(0, 0),
            }]);
        };
        let mut checker = Checker {
            policy: self,
            session,
            dti,
            violations: Vec::new(),
        };
        let count = u32::try_from(period.clause_count()).unwrap_or(u32::MAX);
        for index in 0..count {
            if let Some(ClauseRef::Relational(block)) = period.clause(index) {
                checker.check_clause(id, ClauseId::new(index), block);
            }
        }
        if checker.violations.is_empty() {
            Ok(())
        } else {
            Err(checker.violations)
        }
    }
}

#[cfg(test)]
mod tests;
