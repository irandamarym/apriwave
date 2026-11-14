//! Canonical rendering (Pure Code View) of a period: exact inverse of the parser.
//! Used by the `parse → render == input` round-trip tests.

use crate::prelude::*;

use core::fmt;

use crate::ast::{
    AblativeDeterminer, Case, CaseTile, ClauseBlock, Conditional, DefOperation, DefSignal,
    DefinitionBlock, GlyphAnaphora, GlyphPath, GlyphSlot, Link, LinkedClause, Period,
    PortabilityBlock, Reflexive, RelationalBlock, RelationalSignal, SemaAnaphora,
    SignalDeterminers, TemporalMode, ANAPHORA_MARK, DEEP_SURFACE, QUALIFIER_SEPARATOR,
    RAW_STRING_QUOTE, SEMA_ANAPHORA_MARK,
};
use crate::symbol::{Interner, Symbol};

/// The symbol does not belong to the given interner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownSymbol(pub Symbol);

impl fmt::Display for UnknownSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "symbol {:?} not found in the interner", self.0)
    }
}

impl core::error::Error for UnknownSymbol {}

/// Maps the glyph content of even slots (and the other glyph operands) to text.
///
/// The structure of the period (tiles, connectors, anaphora marks, quotes) is
/// always written by the printer; only glyphs go through the map. The lexical map
/// writes the interned text; the indexed wire form (stage 3) writes DTI keys.
pub(crate) trait GlyphMap {
    type Error;
    /// A whole glyph path (`glyph§attr`): slot glyphs, `glyph~~` bases, portability ids.
    fn path(&mut self, path: &GlyphPath, site: GlyphSite) -> Result<String, Self::Error>;
    /// A single atom: `def` variable names and the atoms of `base~` / `base§mod~`.
    fn atom(&mut self, atom: Symbol, site: GlyphSite) -> Result<String, Self::Error>;
    /// Content of a raw string (written between quotes by the printer).
    fn raw(&mut self, content: Symbol) -> Result<String, Self::Error>;
}

/// Where a glyph operand appears (lets a map treat bound symbols specially).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlyphSite {
    /// Even slot of pair `pair` of clause `clause` (`0` = head), relational or `def` value.
    Slot { clause: u32, pair: u32 },
    /// Name of a `def` variable.
    DefName,
    /// Id of a portability block.
    PortabilityId,
    /// Atoms of `base~` / `base§mod~` and the path of `glyph~~`.
    AnaphoraBase,
}

/// Lexical map: the interned text of every glyph.
pub(crate) struct LexicalMap<'i>(pub &'i Interner);

impl LexicalMap<'_> {
    fn text(&self, symbol: Symbol) -> Result<String, UnknownSymbol> {
        self.0
            .resolve(symbol)
            .map(str::to_owned)
            .ok_or(UnknownSymbol(symbol))
    }
}

impl GlyphMap for LexicalMap<'_> {
    type Error = UnknownSymbol;

    fn path(&mut self, path: &GlyphPath, _site: GlyphSite) -> Result<String, UnknownSymbol> {
        let mut out = self.text(path.root)?;
        for qualifier in path.qualifiers.iter() {
            out.push(QUALIFIER_SEPARATOR);
            out.push_str(&self.text(*qualifier)?);
        }
        Ok(out)
    }

    fn atom(&mut self, atom: Symbol, _site: GlyphSite) -> Result<String, UnknownSymbol> {
        self.text(atom)
    }

    fn raw(&mut self, content: Symbol) -> Result<String, UnknownSymbol> {
        self.text(content)
    }
}

/// Text of an even slot through `map`.
pub(crate) fn map_slot<M: GlyphMap + ?Sized>(
    map: &mut M,
    slot: &GlyphSlot,
    site: GlyphSite,
) -> Result<String, M::Error> {
    let base = GlyphSite::AnaphoraBase;
    Ok(match slot {
        GlyphSlot::Glyph(path) => map.path(path, site)?,
        GlyphSlot::RawString(content) => {
            let mut out = String::new();
            out.push(RAW_STRING_QUOTE);
            out.push_str(&map.raw(*content)?);
            out.push(RAW_STRING_QUOTE);
            out
        }
        GlyphSlot::GlyphAnaphora(GlyphAnaphora::Exact) => ANAPHORA_MARK.to_string(),
        GlyphSlot::GlyphAnaphora(GlyphAnaphora::Prefix { base }) => {
            let mut out = map.atom(*base, GlyphSite::AnaphoraBase)?;
            out.push(ANAPHORA_MARK);
            out
        }
        GlyphSlot::GlyphAnaphora(GlyphAnaphora::Inverse { base, qualifiers }) => {
            let mut out = map.atom(*base, GlyphSite::AnaphoraBase)?;
            for qualifier in qualifiers.iter() {
                out.push(QUALIFIER_SEPARATOR);
                out.push_str(&map.atom(*qualifier, GlyphSite::AnaphoraBase)?);
            }
            out.push(ANAPHORA_MARK);
            out
        }
        GlyphSlot::SemaAnaphora(SemaAnaphora::Latest) => SEMA_ANAPHORA_MARK.to_owned(),
        GlyphSlot::SemaAnaphora(SemaAnaphora::ByGlyph { glyph }) => {
            let mut out = map.path(glyph, base)?;
            out.push_str(SEMA_ANAPHORA_MARK);
            out
        }
    })
}

fn determiners(out: &mut String, d: &SignalDeterminers) {
    if let Some(q) = d.quantifier {
        out.push_str(q.surface());
    }
    if d.reflexive.is_some() {
        out.push_str(Reflexive::SURFACE);
    }
    if let Some(c) = d.comparative {
        out.push_str(c.surface());
    }
    match d.tense {
        Some(TemporalMode::Indicative(t)) => out.push_str(t.surface()),
        Some(TemporalMode::Conditional(Conditional(t))) => {
            out.push_str(Conditional::PREFIX);
            out.push_str(t.surface());
        }
        None => {}
    }
    if let Some(n) = d.number {
        out.push_str(n.surface());
    }
}

pub fn signal(sig: &RelationalSignal) -> String {
    let mut out = String::new();
    match sig {
        RelationalSignal::Primary {
            signal,
            determiners: d,
        } => {
            out.push_str(signal.surface());
            determiners(&mut out, d);
        }
        RelationalSignal::Secondary {
            secondary,
            primary,
            determiners: d,
        } => {
            out.push_str(secondary.surface());
            out.push_str(primary.surface());
            determiners(&mut out, d);
        }
        RelationalSignal::Deep { determiners: d } => {
            out.push_str(DEEP_SURFACE);
            determiners(&mut out, d);
        }
    }
    out
}

pub fn case(tile: CaseTile) -> String {
    let mut out = String::from(tile.case.stem());
    if let Some(n) = tile.number {
        out.push_str(n.surface());
    }
    if let Some(count) = tile.case.loop_count() {
        out.push(crate::ast::LoopCount::SEPARATOR);
        out.push_str(&count.get().to_string());
    }
    out
}

pub fn def_signal(sig: DefSignal) -> String {
    let mut out = String::from(DefSignal::SURFACE);
    if let Some(n) = sig.number {
        out.push_str(n.surface());
    }
    out
}

fn relational<M: GlyphMap + ?Sized>(
    map: &mut M,
    tokens: &mut Vec<String>,
    block: &RelationalBlock,
    clause: u32,
) -> Result<(), M::Error> {
    tokens.push(signal(&block.signal.tile.node));
    tokens.push(map_slot(
        map,
        &block.signal.glyph.node,
        GlyphSite::Slot { clause, pair: 0 },
    )?);
    for (pair, case_pair) in (1_u32..).zip(&block.cases) {
        tokens.push(case(case_pair.tile.node));
        tokens.push(map_slot(
            map,
            &case_pair.glyph.node,
            GlyphSite::Slot { clause, pair },
        )?);
    }
    Ok(())
}

/// Surface of the tile of a `def` operation.
pub(crate) fn def_operation_tile(op: &DefOperation) -> String {
    match op {
        DefOperation::Assign { tile, .. } => case(CaseTile {
            case: Case::Accusative,
            number: tile.node,
        }),
        DefOperation::Modify { .. } => Case::Ablative(Some(AblativeDeterminer::Fi))
            .stem()
            .to_owned(),
        DefOperation::Union { .. } => Case::Ablative(Some(AblativeDeterminer::Si))
            .stem()
            .to_owned(),
        DefOperation::Unbind { .. } => Case::Ablative(Some(AblativeDeterminer::Se))
            .stem()
            .to_owned(),
    }
}

fn definition<M: GlyphMap + ?Sized>(
    map: &mut M,
    tokens: &mut Vec<String>,
    block: &DefinitionBlock,
    clause: u32,
) -> Result<(), M::Error> {
    tokens.push(def_signal(block.signal.node));
    tokens.push(map.atom(block.name.node, GlyphSite::DefName)?);
    for (pair, op) in (1_u32..).zip(block.operations.iter()) {
        tokens.push(def_operation_tile(op));
        tokens.push(match op {
            DefOperation::Assign { value, .. }
            | DefOperation::Modify { value, .. }
            | DefOperation::Union { value, .. } => {
                map_slot(map, &value.node, GlyphSite::Slot { clause, pair })?
            }
            DefOperation::Unbind { .. } => ANAPHORA_MARK.to_string(),
        });
    }
    Ok(())
}

fn clause<M: GlyphMap + ?Sized>(
    map: &mut M,
    tokens: &mut Vec<String>,
    block: &ClauseBlock,
    index: u32,
) -> Result<(), M::Error> {
    match block {
        ClauseBlock::Relational(b) => relational(map, tokens, b, index),
        ClauseBlock::Definition(b) => definition(map, tokens, b, index),
    }
}

fn colons(count: u8) -> String {
    ":".repeat(usize::from(count))
}

/// Tokens of every written clause (connector included), in written order.
pub(crate) fn period_clause_tokens<M: GlyphMap + ?Sized>(
    period: &Period,
    map: &mut M,
) -> Result<Vec<Vec<String>>, M::Error> {
    let mut lines = Vec::with_capacity(period.clause_count());
    let mut head = Vec::new();
    clause(map, &mut head, &period.head, 0)?;
    lines.push(head);
    for (index, linked) in (1_u32..).zip(&period.tail) {
        let mut tokens = Vec::new();
        match linked {
            LinkedClause::Clause { link, block } => {
                match link.node {
                    Link::Subordination(run) => tokens.push(colons(run.count())),
                    Link::Coordination { conj, colons: run } => {
                        tokens.push(conj.node.surface().to_owned());
                        if let Some(run) = run {
                            tokens.push(colons(run.node.count()));
                        }
                    }
                }
                clause(map, &mut tokens, block, index)?;
            }
            LinkedClause::Portability { colons: run, block } => {
                tokens.push(colons(run.node.count()));
                tokens.push(block.keyword().surface().to_owned());
                tokens.push(match block {
                    PortabilityBlock::Anchor { id, .. }
                    | PortabilityBlock::AnchorRel { id, .. }
                    | PortabilityBlock::Rel { id, .. }
                    | PortabilityBlock::RelOff { id, .. } => {
                        map.path(&id.node, GlyphSite::PortabilityId)?
                    }
                    PortabilityBlock::End { .. } => ANAPHORA_MARK.to_string(),
                });
            }
        }
        lines.push(tokens);
    }
    Ok(lines)
}

/// Single-line rendering of the period through `map`.
pub(crate) fn render_period_with<M: GlyphMap + ?Sized>(
    period: &Period,
    map: &mut M,
) -> Result<String, M::Error> {
    let mut tokens = vec![period.open.node.surface().to_owned()];
    tokens.extend(period_clause_tokens(period, map)?.into_iter().flatten());
    tokens.push(period.close.node.surface().to_owned());
    Ok(tokens.join(" "))
}

/// Surface form of a glyph path (`glyph§attr1§attr2`).
pub fn render_glyph_path(path: &GlyphPath, interner: &Interner) -> Result<String, UnknownSymbol> {
    LexicalMap(interner).path(path, GlyphSite::AnaphoraBase)
}

/// Surface form of the content of an even slot.
pub fn render_glyph_slot(slot: &GlyphSlot, interner: &Interner) -> Result<String, UnknownSymbol> {
    map_slot(&mut LexicalMap(interner), slot, GlyphSite::AnaphoraBase)
}

/// Single-line canonical form of the period.
pub fn render_period(period: &Period, interner: &Interner) -> Result<String, UnknownSymbol> {
    render_period_with(period, &mut LexicalMap(interner))
}
