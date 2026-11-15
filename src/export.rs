//! Deterministic structured export of an analyzed period (Worksite 1, Phase A).
//!
//! Dependency-free JSON serializer: field order is fixed and escaping
//! covers `"`, `\`, all control characters and U+2028/U+2029, so for the same
//! session the output is byte-for-byte identical. v4.1: coordination is no
//! longer a structure (coordinated clauses are siblings in written order, each
//! with its own conjunction), every slot reports the Glyph / Label choice of
//! the user and the period lists its warnings.

use crate::analyze::{
    ClauseAnalysis, ClauseId, DspRole, GlobalClause, Resolution, SlotAnalysis, SlotIndex, SlotRef,
    SlotValue,
};
use crate::ast::{
    Case, ClauseRef, Conditional, Conjunction, DefOperation, ExplicitNumber, GlyphSlot,
    PortabilityBlock, QuantumNumber, TemporalMode, ANAPHORA_MARK,
};
use crate::printer::{self, render_glyph_path, render_glyph_slot};
use crate::session::{PeriodId, Session, SlotKind};
use crate::symbol::Symbol;

use std::fmt;

/// Export errors (inconsistencies between the session and the requested identifiers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportError {
    PeriodNotFound(PeriodId),
    ClauseNotFound(u32),
    UnknownSymbol(Symbol),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeriodNotFound(id) => write!(f, "period {} not found", id.index()),
            Self::ClauseNotFound(index) => write!(f, "clause {index} not found"),
            Self::UnknownSymbol(symbol) => write!(f, "symbol {symbol:?} not found in the interner"),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<printer::UnknownSymbol> for ExportError {
    fn from(err: printer::UnknownSymbol) -> Self {
        Self::UnknownSymbol(err.0)
    }
}

// ═══════════════════════════ Minimal JSON tree ═══════════════════════════

enum Json {
    Null,
    Bool(bool),
    Num(u64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(&'static str, Json)>),
}

impl Json {
    fn str(text: &str) -> Self {
        Self::Str(text.to_owned())
    }

    fn opt_str(text: Option<&str>) -> Self {
        text.map_or(Self::Null, Self::str)
    }

    fn num(value: u32) -> Self {
        Self::Num(u64::from(value))
    }

    /// Depth bounded by the schema (≤ 8 levels): recursion is safe.
    fn write(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(true) => out.push_str("true"),
            Self::Bool(false) => out.push_str("false"),
            Self::Num(n) => out.push_str(&n.to_string()),
            Self::Str(s) => escape(out, s),
            Self::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Self::Obj(fields) => {
                out.push('{');
                for (i, (key, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    escape(out, key);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn escape(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ═══════════════════════════ Stable names ═══════════════════════════

const fn role_name(role: DspRole) -> &'static str {
    match role {
        DspRole::Normal => "normal",
        DspRole::Transparent => "transparent",
        DspRole::ForcedSibling => "forced_sibling",
        DspRole::ForcedRoot => "forced_root",
        DspRole::Portability => "portability",
    }
}

const fn number_name(number: QuantumNumber) -> &'static str {
    match number {
        QuantumNumber::Dual => "dual",
        QuantumNumber::Singular => "singular",
        QuantumNumber::Plural => "plural",
        QuantumNumber::NonDual => "non_dual",
    }
}

fn tense_surface(tense: TemporalMode) -> String {
    match tense {
        TemporalMode::Indicative(t) => t.surface().to_owned(),
        TemporalMode::Conditional(Conditional(t)) => {
            format!("{}{}", Conditional::PREFIX, t.surface())
        }
    }
}

fn conj_json(conj: Option<Conjunction>) -> Json {
    Json::opt_str(conj.map(Conjunction::surface))
}

fn clause_ref_json(clause: GlobalClause) -> Json {
    Json::Obj(vec![
        ("period", Json::num(clause.period.index())),
        ("clause", Json::num(clause.clause.index())),
    ])
}

fn slot_ref_json(slot: SlotRef) -> Json {
    Json::Obj(vec![
        ("period", Json::num(slot.clause.period.index())),
        ("clause", Json::num(slot.clause.clause.index())),
        ("position", Json::num(slot.slot.position())),
    ])
}

// ═══════════════════════════ Construction ═══════════════════════════

struct Exporter<'s> {
    session: &'s Session,
}

impl Exporter<'_> {
    fn symbol(&self, symbol: Symbol) -> Result<String, ExportError> {
        self.session
            .interner()
            .resolve(symbol)
            .map(str::to_owned)
            .ok_or(ExportError::UnknownSymbol(symbol))
    }

    /// `expand`: top-level variables show their own values; nested ones
    /// show only the binding (no recursion over binding chains).
    fn value(&self, value: SlotValue, expand: bool) -> Result<Json, ExportError> {
        Ok(match value {
            SlotValue::Glyph(slot) => {
                let text = match self.session.glyph_slot(slot) {
                    Some(content) => {
                        Json::Str(render_glyph_slot(content, self.session.interner())?)
                    }
                    None => Json::Null,
                };
                Json::Obj(vec![
                    ("kind", Json::str("glyph")),
                    ("text", text),
                    ("ref", slot_ref_json(slot)),
                ])
            }
            SlotValue::Variable(id) => {
                let mut fields = vec![
                    ("kind", Json::str("variable")),
                    ("binding", Json::num(id.index())),
                ];
                if expand {
                    if let Some(binding) = self.session.binding(id) {
                        fields.push(("name", Json::Str(self.symbol(binding.name())?)));
                        fields.push(("number", Json::str(number_name(binding.quantum_number()))));
                        let values = binding
                            .values()
                            .iter()
                            .map(|v| self.value(*v, false))
                            .collect::<Result<Vec<Json>, ExportError>>()?;
                        fields.push(("values", Json::Arr(values)));
                    }
                }
                Json::Obj(fields)
            }
            SlotValue::Sema(clause) => Json::Obj(vec![
                ("kind", Json::str("sema")),
                ("clause", clause_ref_json(clause)),
            ]),
        })
    }

    fn slot(
        &self,
        at: SlotRef,
        analysis: &SlotAnalysis,
        written: &GlyphSlot,
    ) -> Result<Json, ExportError> {
        let resolution = match analysis.resolution() {
            Resolution::Literal => Json::Obj(vec![("kind", Json::str("literal"))]),
            Resolution::Variable { binding } => Json::Obj(vec![
                ("kind", Json::str("variable")),
                ("binding", Json::num(binding.index())),
            ]),
            Resolution::GlyphAnaphora { antecedent, .. } => Json::Obj(vec![
                ("kind", Json::str("glyph_anaphora")),
                ("antecedent", slot_ref_json(antecedent)),
            ]),
            Resolution::SemaAnaphora { clause } => Json::Obj(vec![
                ("kind", Json::str("sema_anaphora")),
                ("clause", clause_ref_json(clause)),
            ]),
        };
        Ok(Json::Obj(vec![
            (
                "written",
                Json::Str(render_glyph_slot(written, self.session.interner())?),
            ),
            ("resolution", resolution),
            ("value", self.value(analysis.value(), true)?),
            ("number", Json::str(number_name(analysis.number()))),
            (
                "slot_kind",
                match self.session.slot_kind(at) {
                    Some(SlotKind::Glyph) => Json::str("glyph"),
                    Some(SlotKind::Label) => Json::str("label"),
                    None => Json::Null,
                },
            ),
        ]))
    }

    fn header(index: u32, kind: &str, analysis: &ClauseAnalysis) -> Vec<(&'static str, Json)> {
        vec![
            ("index", Json::num(index)),
            ("kind", Json::str(kind)),
            ("role", Json::str(role_name(analysis.role()))),
            ("depth", Json::num(u32::from(analysis.depth()))),
            ("conjunction", conj_json(analysis.conjunction())),
        ]
    }

    fn clause(
        &self,
        at: GlobalClause,
        block: ClauseRef<'_>,
        analysis: &ClauseAnalysis,
    ) -> Result<Json, ExportError> {
        let index = at.clause.index();
        let slot_ref = |position: u32| SlotRef {
            clause: at,
            slot: SlotIndex::new(position),
        };
        let mut fields = Self::header(index, clause_kind(block), analysis);
        match block {
            ClauseRef::Relational(block) => {
                fields.push((
                    "parent",
                    analysis
                        .parent()
                        .map_or(Json::Null, |p| Json::num(p.index())),
                ));
                fields.push((
                    "children",
                    Json::Arr(
                        analysis
                            .children()
                            .iter()
                            .map(|c| Json::num(c.index()))
                            .collect(),
                    ),
                ));
                fields.push((
                    "signal",
                    Json::Str(printer::signal(&block.signal.tile.node)),
                ));
                if let Some(signal) = analysis.signal() {
                    fields.push((
                        "effective",
                        Json::Obj(vec![
                            (
                                "quantifier",
                                Json::opt_str(signal.quantifier.map(|q| q.surface())),
                            ),
                            (
                                "quantifier_inherited",
                                Json::Bool(signal.quantifier_inherited),
                            ),
                            ("reflexive", Json::Bool(signal.reflexive)),
                            (
                                "reflexive_inherited",
                                Json::Bool(signal.reflexive_inherited),
                            ),
                            (
                                "comparative",
                                Json::opt_str(signal.comparative.map(|c| c.surface())),
                            ),
                            (
                                "comparative_inherited",
                                Json::Bool(signal.comparative_inherited),
                            ),
                            (
                                "tense",
                                signal
                                    .tense
                                    .map_or(Json::Null, |t| Json::Str(tense_surface(t))),
                            ),
                            ("tense_inherited", Json::Bool(signal.tense_inherited)),
                            (
                                "number",
                                Json::opt_str(signal.number.map(ExplicitNumber::surface)),
                            ),
                            ("number_inherited", Json::Bool(signal.number_inherited)),
                        ]),
                    ));
                }
                fields.push((
                    "canonical_order",
                    Json::Arr(
                        analysis
                            .canonical_order()
                            .iter()
                            .map(|i| Json::num(*i))
                            .collect(),
                    ),
                ));
                // All pairs share the same structure: the tile declares their type.
                let tiles = std::iter::once(("signal", printer::signal(&block.signal.tile.node)))
                    .chain(
                        block
                            .cases
                            .iter()
                            .map(|pair| ("case", printer::case(pair.tile.node))),
                    );
                let glyphs = std::iter::once(&block.signal.glyph)
                    .chain(block.cases.iter().map(|pair| &pair.glyph));
                let mut pairs = Vec::with_capacity(block.pair_count());
                for (position, ((tile_kind, tile), glyph)) in tiles.zip(glyphs).enumerate() {
                    let position =
                        u32::try_from(position).map_err(|_| ExportError::ClauseNotFound(index))?;
                    let mut entry = vec![
                        ("position", Json::num(position)),
                        ("tile_kind", Json::str(tile_kind)),
                        ("tile", Json::Str(tile)),
                    ];
                    match analysis.slot(SlotIndex::new(position)) {
                        Some(slot) => {
                            if let Json::Obj(rest) =
                                self.slot(slot_ref(position), slot, &glyph.node)?
                            {
                                entry.extend(rest);
                            }
                        }
                        // `~` marker after `abdur` (B2): structural, never resolved.
                        None => {
                            entry.push((
                                "written",
                                Json::Str(render_glyph_slot(&glyph.node, self.session.interner())?),
                            ));
                            entry.push((
                                "resolution",
                                Json::Obj(vec![("kind", Json::str("marker"))]),
                            ));
                        }
                    }
                    pairs.push(Json::Obj(entry));
                }
                fields.push(("pairs", Json::Arr(pairs)));
            }
            ClauseRef::Definition(block) => {
                fields.push(("signal", Json::Str(printer::def_signal(block.signal.node))));
                fields.push(("name", Json::Str(self.symbol(block.name.node)?)));
                let mut operations = Vec::with_capacity(block.operations.len());
                // Pair 0 = name; the operations occupy positions 1…n.
                let operation_slots = analysis.slots().iter().skip(1);
                for (offset, (op, slot)) in block.operations.iter().zip(operation_slots).enumerate()
                {
                    let position = offset
                        .checked_add(1)
                        .and_then(|p| u32::try_from(p).ok())
                        .ok_or(ExportError::ClauseNotFound(index))?;
                    let (name, tile, written) = match op {
                        DefOperation::Assign { tile, value } => {
                            let mut surface = String::from(Case::Accusative.stem());
                            if let Some(n) = tile.node {
                                surface.push_str(ExplicitNumber::surface(n));
                            }
                            ("assign", surface, Some(&value.node))
                        }
                        DefOperation::Modify { value, .. } => {
                            ("modify", "abfi".to_owned(), Some(&value.node))
                        }
                        DefOperation::Union { value, .. } => {
                            ("union", "absi".to_owned(), Some(&value.node))
                        }
                        DefOperation::Unbind { .. } => ("unbind", "abse".to_owned(), None),
                    };
                    let mut entry = vec![
                        ("position", Json::num(position)),
                        ("op", Json::str(name)),
                        ("tile", Json::Str(tile)),
                    ];
                    match (slot, written) {
                        (Some(slot), Some(written)) => {
                            entry.push(("value", self.slot(slot_ref(position), slot, written)?));
                        }
                        _ => entry.push(("value", Json::Null)),
                    }
                    operations.push(Json::Obj(entry));
                }
                fields.push(("operations", Json::Arr(operations)));
            }
            ClauseRef::Portability(block) => {
                fields.push(("keyword", Json::str(block.keyword().surface())));
                let operand = match block {
                    PortabilityBlock::Anchor { id, .. }
                    | PortabilityBlock::AnchorRel { id, .. }
                    | PortabilityBlock::Rel { id, .. }
                    | PortabilityBlock::RelOff { id, .. } => {
                        render_glyph_path(&id.node, self.session.interner())?
                    }
                    PortabilityBlock::End { .. } => ANAPHORA_MARK.to_string(),
                };
                fields.push(("operand", Json::Str(operand)));
            }
        }
        Ok(Json::Obj(fields))
    }
}

const fn clause_kind(block: ClauseRef<'_>) -> &'static str {
    match block {
        ClauseRef::Relational(_) => "relational",
        ClauseRef::Definition(_) => "definition",
        ClauseRef::Portability(_) => "portability",
    }
}

/// Deterministic JSON export of the analyzed period `id`.
pub fn export_analyzed_json(session: &Session, id: PeriodId) -> Result<String, ExportError> {
    let period = session.period(id).ok_or(ExportError::PeriodNotFound(id))?;
    let analysis = session
        .analysis(id)
        .ok_or(ExportError::PeriodNotFound(id))?;
    let exporter = Exporter { session };

    let mut clauses = Vec::with_capacity(period.clause_count());
    for (offset, clause_analysis) in analysis.clauses().iter().enumerate() {
        let index = u32::try_from(offset).map_err(|_| ExportError::ClauseNotFound(u32::MAX))?;
        let block = period
            .clause(index)
            .ok_or(ExportError::ClauseNotFound(index))?;
        let at = GlobalClause {
            period: id,
            clause: ClauseId::new(index),
        };
        clauses.push(exporter.clause(at, block, clause_analysis)?);
    }

    let document = Json::Obj(vec![
        ("period", Json::num(id.index())),
        ("open", Json::str(period.open.node.surface())),
        ("close", Json::str(period.close.node.surface())),
        (
            "roots",
            Json::Arr(
                analysis
                    .roots()
                    .iter()
                    .map(|r| Json::num(r.index()))
                    .collect(),
            ),
        ),
        ("clauses", Json::Arr(clauses)),
        (
            "warnings",
            Json::Arr(
                analysis
                    .warnings()
                    .iter()
                    .map(|warning| {
                        Json::Obj(vec![
                            ("code", Json::str(warning.kind.code().as_str())),
                            ("message", Json::Str(warning.kind.to_string())),
                            ("start", Json::num(warning.span.start)),
                            ("end", Json::num(warning.span.end)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]);
    let mut out = String::new();
    document.write(&mut out);
    Ok(out)
}
