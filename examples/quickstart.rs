//! Quickstart: ingest periods, inspect the analysis, render the canonical,
//! expanded and indexed forms.
//!
//! Run with `cargo run --example quickstart`.

use apriwave::dti::{Dti, DtiKey};
use apriwave::printer::render_period;
use apriwave::stage3::{render_expanded_period, render_indexed_period};
use apriwave::Session;

/// Builds a DTI key from a short tag (the first character must be a letter).
fn key(tag: &str) -> Result<DtiKey, String> {
    let mut body = [b'0'; 31];
    for (slot, byte) in body.iter_mut().zip(tag.bytes()) {
        *slot = byte;
    }
    DtiKey::from_random_body(body).map_err(|e| e.to_string())
}

fn main() -> Result<(), String> {
    // Stage 1 + stage 2, atomically: on any diagnostic the session is unchanged.
    let mut session = Session::new();
    let ids = session
        .ingest("/^ ta vedere nu io au canis : ta abbaiare nu ~ ^/")
        .map_err(|diagnostics| format!("{diagnostics:?}"))?;
    let id = ids.first().copied().ok_or("no period")?;

    // Canonical written form (exact inverse of the parser).
    let period = session.period(id).ok_or("missing period")?;
    let canonical = render_period(period, session.interner()).map_err(|e| e.to_string())?;
    println!("canonical: {canonical}");

    // Expanded form: anaphora resolved, effective numbers written.
    let expanded = render_expanded_period(&session, id).map_err(|e| e.to_string())?;
    println!("expanded:  {expanded}");

    // Indexed M2M form: every glyph replaced by its DTI key.
    let (see, i, dog, bark) = (key("see")?, key("i")?, key("dog")?, key("bark")?);
    let universal = format!(
        "#awdti 1\n#role universal\n#version 1\n#lang en\n\
         rec {see} see\nrec {i} i\nrec {dog} dog\nrec {bark} bark\n"
    );
    let local = format!(
        "#awdti 1\n#role local\n#universal 1\n#lang it\n\
         use {see} form:vedere\nuse {i} form:io\nuse {dog} form:canis\nuse {bark} form:abbaiare\n"
    );
    let dti = Dti::load(&universal, &local).map_err(|d| format!("{d:?}"))?;
    let indexed = render_indexed_period(&session, id, &dti).map_err(|e| e.to_string())?;
    println!("indexed:   {indexed}");
    Ok(())
}
