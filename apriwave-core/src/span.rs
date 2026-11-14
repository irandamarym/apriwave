//! Positions in the source.
//!
//! Offsets are in **bytes** (UTF-8): `/^≈` and `/^≃` take 5 bytes each.
//! The lexer rejects inputs longer than `u32::MAX` bytes, so every offset is
//! representable without truncation.

/// Half-open byte range `[start, end)` in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}

/// Node paired with its own position in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub const fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}
