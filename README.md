# ApriWave

**ApriWave (ApricotWave Protocol) v4.0** is a deterministic relational framework for
machine-to-machine semantics. It is implemented in Rust as a **Sans-I/O** engine:
it never touches the network, the clock or the file system on its own. It takes
text in and returns structures, diagnostics and canonical text out.

[![CI](https://github.com/irandamarym/apriwave/actions/workflows/ci.yml/badge.svg)](https://github.com/irandamarym/apriwave/actions/workflows/ci.yml)
![no_std](https://img.shields.io/badge/core-no__std%20%2B%20alloc-blue)
![unsafe](https://img.shields.io/badge/unsafe-forbidden-success)
![license](https://img.shields.io/badge/code-GPL--3.0--or--later-informational)
![docs license](https://img.shields.io/badge/spec%20%26%20docs-CC--BY--SA--4.0-informational)

---

## What it is

ApriWave is **not** a natural language. A period is a sequence of `[Tile] [Glyph]`
pairs:

- **Tiles** form a closed, fixed set of grammatical tools: signals, determiners,
  cases, numbers, connectors and period delimiters.
- **Glyphs** are the content, and belong to a dictionary of concepts, the **DTI**.

```text
/^ ta vedere nu io au canis : ta abbaiare nu ~ ^/
```

The protocol governs **only the grammar of the tiles** and never interprets the
meaning of a glyph. Every valid string has exactly one structure. Loops
(`abdur=<N>`) and set operations (`all`, `some`, `cup`, `cua`, `cuo`, …) are
resolved by the engine into a validated, deterministic **execution plan**: the host
only performs the physical effect.

## Architecture

The repository is a Cargo workspace with two crates:

| Crate | Environment | Contents |
|---|---|---|
| [`apriwave-core`](apriwave-core) | `#![no_std]` + `alloc`, **zero dependencies** | Lexer, parser, stage-2 analysis, canonical printer, stage 3 (indexed M2M form and expanded form), two-level DTI and its `.dti.bin` binary form on buffers, execution plan, policy manager, Legacy Shield. Builds for bare-metal Cortex-M targets (`thumbv6m-none-eabi`, `thumbv7em-none-eabihf`). |
| [`apriwave`](src) | `std` | Re-exports the whole core under the same module paths and adds the file-system cache of `.dti.bin`, the assisted editor, the JSON export, the `apriwave-cli` binary and the experimental Ghost Protocol. |

### The three stages

1. **Stage 1, syntax.** A segmentation-only lexer and a positional parser build an
   AST that mirrors the grammar. Block-local checks run here, and the canonical
   rendering is the exact inverse of the parser.
2. **Stage 2, analysis.** This stage builds the DSP tree (with anchors and forced
   roles) and applies determiner inheritance. It resolves glyph and sema anaphora,
   tracks the lifecycle of `def` variables, builds the coordination tree and applies
   the three close levels (`/^/`, `^/`, `^^/`).
3. **Stage 3, DTI.** Glyphs are resolved into concept keys and the morphology flags
   are validated. This stage produces the **indexed form** (the only M2M exchange
   form), the expanded form and the human views.

## Guarantees

- **Memory safety:** `#![forbid(unsafe_code)]` in every crate.
- **Zero panic:** clippy denies `unwrap`, `expect`, `panic!`, `todo!`,
  `unimplemented!`, `unreachable!`, slice indexing and redundant clones. Every
  failure is a structured `Result`.
- **Atomic ingestion:** `Session::ingest` runs stages 1 and 2 as a transaction. On
  any diagnostic the session, including its interner, is left exactly as it was,
  and all diagnostics are returned sorted by position.
- **Determinism:** the core uses `BTreeMap`/`BTreeSet`, so iteration order is
  deterministic and lookups are O(log n) in the worst case, with no HashDoS
  exposure. The binary DTI form is canonical: one model gives one byte sequence.
- **Bounded resources:** configurable limits apply to DSP depth, loop iterations,
  input size and index spaces. Anaphora, variable and sema chains are resolved
  iteratively. The interner stores each text once, and `Session::compact()`
  releases a session after a hard close (`^^/`) on low-RAM devices.

## Quick start

```rust
use apriwave::dti::{Dti, DtiKey};
use apriwave::printer::render_period;
use apriwave::stage3::{render_expanded_period, render_indexed_period};
use apriwave::Session;

fn key(tag: &str) -> Result<DtiKey, String> {
    let mut body = [b'0'; 31];
    for (slot, byte) in body.iter_mut().zip(tag.bytes()) {
        *slot = byte;
    }
    DtiKey::from_random_body(body).map_err(|e| e.to_string())
}

fn main() -> Result<(), String> {
    // Stages 1 and 2, atomically.
    let mut session = Session::new();
    let ids = session
        .ingest("/^ ta vedere nu io au canis : ta abbaiare nu ~ ^/")
        .map_err(|diagnostics| format!("{diagnostics:?}"))?;
    let id = ids.first().copied().ok_or("no period")?;

    let period = session.period(id).ok_or("missing period")?;
    println!("{}", render_period(period, session.interner()).map_err(|e| e.to_string())?);
    // /^ ta vedere nu io au canis : ta abbaiare nu ~ ^/

    println!("{}", render_expanded_period(&session, id).map_err(|e| e.to_string())?);
    // /^ ta vedere nu io au canis : ta abbaiare nu canis ^/

    // A two-level DTI: universal snapshot + local overlay.
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

    // The indexed M2M form: every glyph replaced by its 32-character DTI key.
    println!("{}", render_indexed_period(&session, id, &dti).map_err(|e| e.to_string())?);
    Ok(())
}
```

The same program is in [`examples/quickstart.rs`](examples/quickstart.rs):

```sh
cargo run --example quickstart
```

### Bare-metal use

Depend on `apriwave-core` only and provide a global allocator (for example
`embedded-alloc`). The core needs no operating system, has no external dependency
and does not use atomic compare-and-swap, so it also runs on Cortex-M0+.

## Build and test

```sh
cargo build --workspace
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check

# Bare-metal check of the core
rustup target add thumbv6m-none-eabi thumbv7em-none-eabihf
cargo check -p apriwave-core --target thumbv6m-none-eabi
cargo check -p apriwave-core --target thumbv7em-none-eabihf
```

Optional Ghost Protocol suites are behind the `ghost-blake3`, `ghost-ascon`,
`ghost-skein` and `ghost-argon2` features of the `apriwave` crate. The default build
has no dependencies.

## Documentation

The normative reference for the grammar, the engine and every module is the
technical manual, written in Italian:
[`docs/Manuale_Tecnico_ApriWave_v4.0.md`](docs/Manuale_Tecnico_ApriWave_v4.0.md).

## License

Copyright (c) 2026 irandamarym

- **Source code:** licensed under the **GNU General Public License v3.0 or later**
  (SPDX: `GPL-3.0-or-later`). See [`LICENSE`](LICENSE).
- **Grammar specification and documentation** ([`docs/`](docs)): licensed under
  **Creative Commons Attribution-ShareAlike 4.0 International** (SPDX:
  `CC-BY-SA-4.0`). See [`docs/LICENSE-CC-BY-SA-4.0.md`](docs/LICENSE-CC-BY-SA-4.0.md).

This program is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See the GNU General Public License for more details.
