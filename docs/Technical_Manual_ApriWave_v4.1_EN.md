# Technical Manual — ApriWave · GHLIP Protocol v4.1

> English translation of [`Manuale_Tecnico_ApriWave_v4.1.md`](Manuale_Tecnico_ApriWave_v4.1.md). The Italian edition remains the normative text: in case of discrepancy, the Italian edition prevails. Codes, tiles and examples are identical in both editions.

**Consolidated edition of 15/11/2026 — version 4.1.** Single normative reference for the grammar, the engine and the modules of the ApriWave workspace (crates `apriwave-core` and `apriwave`, version `4.1.0`). It merges Manual v4.0, the Framework of Evolutions and decisions Q1–Q28, D1–D14, R1–R14, H1–H8, B1–B12, L1–L5, S1–S3, G1–G7, P1–P5 and V1–V10.

Version 4.1 removes the arbitrary restrictions introduced by the previous iterations: glyphs with any visible character and pure Slot Locking; `§` with DTI priority; labels registered in the local DTI instead of quotes; `ref` compatible with the Accusative and with the comparatives; repeated cases with a stable canonical order; anaphora reduced to `~` and `~~`; coordination in written order; full inheritance in subordinate clauses; nested `abdur` loops and inherited conditional; downgrade of single error codes to warnings.

It contains no removed element, such as `§ack§req`/`§ack§opt` (B1), `sys§`, quoted labels, `base~`, `base§mod~`, `glyph~~` or the coordination tree.

---

## 1. Principles

1. **Neutral relational frame.** GHLIP is not a natural language. *Tiles* are a closed, fixed set of grammatical tools; *glyphs* are the content and belong to the DTI.
2. **Golden rule.** The protocol governs **exclusively the grammar of the tiles** and never interprets the meaning of DTI glyphs. The loop count is read only from the `abdur=<N>` tile; sets are computed only on the structural `base:` relations of the keys.
3. **Absolute determinism.** Every valid string corresponds to exactly one structure.
4. **Self-contained engine (update of M§1.1).** The engine does not delegate the resolution of `abdur=<N>` loops or of set operations (`all`, `some`, `cup`, `absi`, `cua`, `abpa`, `cuo`, `abse`) to external modules. It resolves them internally and produces a deterministic, validated **execution plan**. Only the physical effect is left to the host device.
5. **Slot symmetry.** All even slots have the same status: no glyph is "special" because of its position. The only different constraints come from the **tile** preceding the glyph: `def` (Q11), the portability tiles (Q6), `abse ~` (D2/R14), `abdur ~` (B2).
6. **Terms.** In the Italian original *glifo* and *sema* are invariable in the plural. This edition uses *glyph* / *glyphs*, and *sema* as an invariable term.
7. **Neutrality (v4.1).** The grammar is a mechanical, deterministic and neutral engine: it neither knows nor judges the content of glyphs, and guides the user without imposing arbitrary choices or bans. Only the constraints needed by the structure (Slot Locking, DSP, tiles) and by security (invisible characters, D9) remain.

### 1.1 Architectural nomenclature note

The name of the **projects and containers** (repository and Rust crates) is distinct from the name of the **protocol** and of the **program**.

**Level 0 — structural and relational (this repository)**

- **Project / container name: ApriWave** (*Apricot Wave*). It identifies the ecosystem, the repository and the Rust crates (`apriwave-core` and `apriwave`). It joins the historical tribute to *Apricot Computers* (ACT) with the concept of a carrier wave and of the quantum wave function.
- **Protocol name: GHLIP** (**Ghlip**), technical acronym of **G**rammatical **H**ierarchical **L**inear **I**nter-species **P**rotocol. It formally identifies the deterministic Level-0 grammatical protocol:
  - **G — Grammatical:** closed, immutable frame of the *Universal Grammatical Tiles* (*Tile Matrix*).
  - **H — Hierarchical:** tree hierarchy without closing tags, based on *Dynamic Stack Pruning* (`:`, `::`, `:::`).
  - **L — Linear:** deterministic linear scan based on *Slot Locking* (`1 Tile -> 1 Glyph`).
  - **I — Inter-species:** neutral framework free of cognitive or anthropocentric bias, born for inter-species contact logs and for deterministic M2M communication through the DTI dictionary.
  - **P — Protocol:** pure Level-0 protocol, separate from the computational layer.
- **Terminological disambiguation:**
  - `GHLIP` (`Ghlip`) is the formal acronym naming the whole Level-0 protocol.
  - The term `glyph` (Italian `glifo`, invariable in the plural in Italian: *i glifo*) denotes exclusively the single atomic element registered in the DTI dictionary, within the founding triad *Tiles* + *Glyph* = *Sema*.

**Level 1 — computational and inferential (second project of the ecosystem)**

- **Second project name: Apricot Logic.** It is the twin Level-1 project, separate from Level 0.
- **Program / engine name: LEMMA.** It is the Level-1 logical and homoiconic program and engine. It operates on the `/^ ... ^/` periods already validated by the `GHLIP` protocol, and interfaces with `apriwave-core` through the transactional hook `FunctionEvaluator` (`Session::evaluate_and_ingest`, §22).

---

## 2. Writing

| Rule | Detail |
|---|---|
| Separator | Exactly **one ASCII space** between tokens. Double, leading or trailing spaces, tabs and line breaks give ERR_SYNTAX (Q12). Free-form writing is allowed only in the editor (§21). |
| Tiles | Lowercase; `.` `\|` `(` `)` forbidden (M§1.4). |
| Glyphs (v4.1) | **Any visible character**: every script, uppercase, digits, punctuation, symbols (`-`, `_`, `"`, `§`, `~`, `:`, `/`, `^`…). Only invisible characters are forbidden (D9): control (`Cc`) and format/bidi U+061C, U+200B–200F, U+202A–202E, U+2060–2064, U+2066–2069, U+FEFF (`MalformedGlyph { InvisibleCharacter }`). |
| Alternation | Every block is a sequence of **[Tile] [Glyph]** pairs: tiles occupy the odd slots, glyphs the even ones. |
| Slot Locking (Q8, V2) | **Single even-slot rule.** Any sequence of visible characters (`canis`, `Roma`, the delimiters `/^`, `/^≈`, `^/`, `/^/`, `^^/`, runs of `:`, the tile names `a`, `ta`, `y`…, `"`, `§`, `~`, `~~`) can be registered in the DTI as a glyph and in the local DTI as a label. The role of the token in the even slot (**Glyph**, **Label** or **Anaphora**) is fixed uniquely by the **annotation of the slot** (`SlotKind`), chosen by the user in the editor menu among the roles the DTI allows and given at ingestion (`Session::ingest_with_choices(src, &[(Span, SlotKind)])`): slot annotated Glyph → glyph, slot annotated Label → label. An unannotated slot has the positional role of stage 1: Anaphora for the exact tokens `~` and `~~` (or the marker of `abse ~` and `end ~`), Glyph for every other token. `Session::ingest` means no annotation. The same mechanism tells a glyph from a homograph label and an anaphora from a glyph or a label written `~` / `~~`: there is no special case. Stages 1 and 2 read only the position and the annotation, so they stay independent of the DTI. Display and M2M transmission have no conflicts: a Glyph has the glyph colour and travels as its `DtiKey`, a Label has the label colour and travels as `LVAAAA<form>` (e.g. `LVAAAA~`, `LVAAAA~~`), an Anaphora travels as `~` / `~~`. |
| Missing glyph | Since an even slot accepts any token, a glyph is missing only at the end of the input (`/^ ta vedere nu` → ERR_CASE_AMBIGUITY). In `/^ ta vedere nu ^/` the token `^/` is the glyph of `nu` and the period stays open (ERR_SYNTAX `MissingPeriodClose`). **Double check:** besides the missing close, the token placed in the even slot is **always checked against the DTI** like any glyph: if `^/` is not registered, the editor highlights it as not registered (`UnknownGlyph { structural: true }`) and asks the user to register it in the local DTI as a **Glyph** or as a **Label** (§21.1); in a valid period (`/^ ta vedere nu ^/ ^/`) stage 3 resolves it in the DTI like any glyph (`UnknownForm` if not registered, §17.1). |

---

## 3. Period

```ebnf
session = period , { SP , period } ;
period  = open , SP , block , { SP , link , SP , block } , SP , close ;
```

| Open | Meaning | Constraint |
|---|---|---|
| `/^` | Start of period; resets the DSP context | — |
| `/^≈` | Strict simultaneity with the previous period | Requires an antecedent (Q23): a previous period, possibly in the same input, **not** closed by `^^/` (D5, R12) |
| `/^≃` | Logical proximity | As above |

| Close | Level-0 anaphora | `def` variables | Synchronization |
|---|---|---|---|
| `/^/` Soft | **kept**: they stay visible in the following periods and accumulate | kept | kept |
| `^/` Standard | cleared | kept | kept |
| `^^/` Hard | cleared | **cleared** | **cleared** |

---

## 4. Blocks

| Block | Form |
|---|---|
| Relational | `[signal+determiners] [glyph] { [case] [glyph] }` |
| Definition | `[def…] [name] [op] [value] { [op] [value] }` (§12) |
| Portability | `anchor|anchorrel|rel|reloff <glyph>` or `end ~`, only after `:` (§9.5) |

The first block of the period has no connector and cannot be a portability block.

---

## 5. Signal tiles

### 5.1 Heads
| Code | Meaning |
|---|---|
| `ta` | Affirmation (default) |
| `to` | Negation |
| `ka` | Question |
| `ko` | Negative question |
| `ot` | Possibility |
| `oh` | Vocative (forces a sibling in the DSP, §9.3) |
| `qua` / `que` / `quo` / `quu` | Secondary signals, only in subordinate clauses (Q22): basic indirect, hypothetical, imperative, indeterminate. They are **mandatorily** followed by a primary (`quata`, `quotazato`). |
| `deep` | M2M channel (forces level 0 in the DSP, §9.3). Accepts determiners (`deepall`), never a primary (Q4). |
| `def`, `defu`, `defm`, `defnum` | Definition: agglutinates **only** the number (Q4) |

### 5.2 Determiners (agglutinated, fixed order)
`[signal][quantifier][ref][comparative][temporal | conditional][number]`

| Class | Codes |
|---|---|
| Quantifier | `all` (∀), `some` (∃) |
| Reflexive | `ref`. It admits the Accusative (v4.1): the direct object is not mandatory (direct reflexive) but fully allowed (indirect or transitive reflexive: "the node updates its firmware", `taref aggiornare nu nodo a firmware`). |
| Comparative | `cha` greater, `chu` smaller, `cla` different, `col` equal, `cal` similar, `cua` included, `cuo` excluded, `cup` united |
| Temporal | `ato` present (default), `alt` remote past, `alv` remote future, `att` recent past, `atv` near future, `asu` indeterminate time |
| Conditional | `z` + temporal: `zato`, `zalt`, `zalv`, `zatt`, `zatv`, `zasu` |
| Number (rule 1B) | `u`, `m`, `num`, always last: `tau`, `tam`, `tanum`, `deepallu`, `tachazatou`. When omitted in a subordinate clause it is inherited from the parent signal (§6.1, §10). |

- No determiner starts with `u`, `m` or `n`, so reading the trailing number stays unambiguous.
- A determiner out of sequence gives **ERR_ORDER**, also after the number (`tauzato`).
- A slot occupied twice gives ERR_ORDER. This covers duplicates (Q24), temporal and conditional together such as `taaltzato` (Q5), and a repeated number (`tauu`, `deepmu`).
- `ref` combines with any comparative (`tarefcha`, `taallrefchaato`): the D6 incompatibility is repealed (v4.1).

---

## 6. Case tiles

`[case][determiner][number]` and, only for `abdur`, `=<N>`.

| Code | Case | Use |
|---|---|---|
| `n` | Nominative | Subject |
| `a` | Accusative | Object, object of comparison |
| `g` / `gpa` | Genitive / genitive partitive | |
| `d` / `dad` | Dative / allative (**only** `dad`, Q2) | |
| `ab` | Generic ablative | |
| `abef` | Elative | |
| `aben` | Inessive | |
| `abel` | Illative | |
| `abse` | Abessive ("without") | |
| `abfi` | Translative | |
| `abcu` | Instructive | |
| `absi` | Comitative | |
| `abdu` | Continuous temporal ("while / during") — **never a loop** | |
| `abpa` | Partitive | |
| **`abdur=<N>`** | **Loop operator** (B2) | `N = [1-9][0-9]*` |

- **Number**: omitted (Dual), `u` Singular, `m` Plural, `num` Non-Dual. Examples: `nu`, `am`, `abenu`, `dadnum`, `abduru=5`.
- **Repeated cases (v4.1).** The same case may appear several times in the same block: several `nu`, several `au`, several coordinated or listed `du`, several `abdur=<N>` (§13). The Q21 ban and the exception reserved to `absi` are repealed. The canonical order is stable and respects the written order for equal cases (§11.2).

### 6.1 Quantum number (H5)
| State | Marker | Meaning |
|---|---|---|
| Dual | omitted | Superposition / indeterminacy (wave-particle duality) before the collapse |
| Singular | `u` | Collapse onto unity |
| Plural | `m` | Collapse onto multiplicity |
| Non-Dual | `num` | Quantity detected but not yet classified |

Every pair has an effective number (**rule 1B**):

| Pair | Number written in the tile | Number omitted |
|---|---|---|
| Signal pair (slots 1–2) | Prevails (`tau ~`, `tam gruppo`) | In a subordinate clause with a primary or secondary signal it **inherits the number of the parent signal** (v4.1, §10); otherwise Dual. A `~` or a `def` variable in slot 2 does not impose its own number. |
| Case pair (slots 3–4 and following) | Prevails | Dual. Before `~` it inherits the number of the antecedent pair; before a `def` variable it takes the variable's number (Q18, R9.4) |

The rule is symmetric to `def`/`defu`/`defm`/`defnum` and to the case tiles: an explicit number written by the user always prevails.

---

## 7. Glyph slots

| Form | Example | Notes |
|---|---|---|
| Glyph | `canis`, `Roma`, `x-y`, `東京`, `30`, `^/`, `"` | Any token of visible characters in an even slot whose role is Glyph (§2): a slot annotated Glyph, or an unannotated slot with a token other than `~` / `~~`. The editor shows it in the glyph colour. |
| Qualified glyph | `video§rosso`, `verde§smeraldo§142` | `§` separates distinct DTI glyphs. If, however, the whole token (containing or equal to `§`) is registered in the DTI as a single glyph, **the DTI has priority** and the token is one glyph (§17.1). Stage 1 also accepts empty segments (`§`, `x§`, `§x`); at stage 3 an empty segment is an unknown form. |
| Label | `Roma` | Slot that the user marked as a label with the mandatory selection (§16.6); the form is registered in the local DTI (`label`). It is written without quotes or prefixes and the editor shows it in the label colour; in the indexed transmission it travels as `LVAAAA<form>`. Quotes `"…"` no longer have any grammatical value: `"` is a character like any other. |
| Glyph anaphora | `~` (unannotated slot) | Last canonical glyph of the antecedent block (§11) |
| Sema anaphora | `~~` (unannotated slot) | Whole antecedent sema, in any even slot (§11.5) |
| Slot role (single rule) | `canis`, `^/`, `~`, `~~` | For every token, whatever its characters: slot annotated Glyph → literal glyph (`Resolution::Literal`, or a live `def` variable); slot annotated Label → inert label; unannotated slot → stage-1 role (Anaphora for `~` / `~~`, Glyph for every other token). An annotated token searches no antecedent and gives no ERR_REFERENCE, also in the first block of the period |

The forms `base~`, `base§mod~` and `glyph~~` are **repealed** (v4.1): a token such as `video~` or `vedere~~` is an ordinary glyph.

---

## 8. Connectors

```ebnf
link = colon_run | conjunction , [ SP , colon_run ] ;
```

| Connector | Depth |
|---|---|
| `:`×N | N (with an active relative anchor: K + (N−1), §9.5) |
| `y`, `vel`, `aut` without `:` | Current depth of the last active block |
| `sed` without `:` | Level 0 (closes the open branches) |
| Conjunction + `:`×N | N |

Going from depth D to a depth > D+1 gives **ERR_SCOPE** (Q14). Configurable limit `max_dsp_depth`, default 64 (Q13).

---

## 9. DSP tree

### 9.1 Placement
The DSP stack holds the active block at every depth. Placing a block at depth T truncates the stack to T; the parent is `stack[T−1]`.

### 9.2 Coordination (Q15, R4, v4.1)
- Siblings under the same parent keep the user's **exact written order**. There is no coordination tree and no presumed boolean grouping (`CoordNode` is removed): the user decides the order, not the grammar.
- Every block carries its own conjunction: `y`, `vel`, `aut` bind the group of siblings, `sed` binds the last element. The logical reading of the conjunctions belongs to Apricot Logic.
- Siblings written without a conjunction are distinct branches.
- `sed` without `:` returns to level 0 (§8).
- A conjunction without a left sibling (for example after a transparent `def`) stays on its own clause.

### 9.3 Forced roles (H4, Q16 — "Explicit colons win")
| Signal | Without `:` | With explicit `:` |
|---|---|---|
| `oh` (also in the secondary) | Sibling of the last active block | Follows the `:` |
| `deep` | Level 0 | Follows the `:` |
| `def` | Transparent: does not enter the stack | The `:` are only checked (level skip) |

### 9.4 Example
`/^ ta vedere nu io au canis : ta abbaiare nu ~ :: tazato fame nu ~ : ta osservare nu gatto ^/` produces: vedere(0) → abbaiare(1) → fame(2); osservare(1) is a child of vedere.

### 9.5 Anchors (Q6, R1–R3, H3)
Portability tiles appear only after `:`, never at the head nor after a conjunction (D1). They accept no case pairs.

| Tile | Effect |
|---|---|
| `: anchor <id>` | Checks the `:`, truncates the stack, registers the anchor with its insertion depth, **absolute** mode |
| `: anchorrel <id>` | Like `anchor`, but activates the **relative** mode |
| `: rel <id>` | Reactivates the relative mode on `<id>` (ERR_REFERENCE if absent) |
| `: reloff <id>` | Deactivates the relative mode; the anchor stays open |
| `: end ~` | Closes the last anchor, brings the stack back to its depth (ERR_SCOPE if no anchor is open) |

- In relative mode with an anchor of depth K, a clause with C `:` goes to depth **K + (C−1)**.
- An anchor written in relative mode saves the effective depth as its own K (R1).
- Anchors are local to the period: an unclosed anchor gives ERR_SCOPE (R3).
- Portability blocks are not nodes of the tree (R2).
- In the editor `Ctrl+C` and `Alt+C` copy the selected subordinate clauses as an `anchor` or `anchorrel` block (§21.4).

---

## 10. Inheritance and checks

| Rule | Content |
|---|---|
| Q17 / R5 (v4.1) | A subordinate clause with a primary or secondary signal inherits from its parent (effective values, hence transitively) **tense/conditional, the number of the signal and every omitted determiner**: quantifier (`all`/`some`), reflexive `ref` and comparative. A determiner written on the subordinate always prevails (override). `deep` does not inherit (except the determiners written on `deep`). |
| Q19 | A **written** comparative requires at least one Accusative (`a`, `au`, `am`, `anum`), otherwise ERR_SEMANTIC. An inherited comparative imposes nothing. |
| Q22 | Secondary signal at level 0: ERR_SCOPE. |

Repealed in v4.1: D6 (`ref` + comparative), D8 (`ref` + Accusative), Q20 (`QuantifierMisuse`: quantifier without case pairs), Q21 (duplicate cases).

---

## 11. Anaphora

### 11.1 Antecedents (R6)
The search order is:
1. the parent;
2. the ancestors up to the root;
3. the preceding level-0 blocks, most recent first;
4. the level-0 blocks kept by `/^/`.

Pruned branches are invisible. `~` does not see its own block.

### 11.2 Canonical order
First the signal pair (position 0), then the case pairs in the order `n → a → d → g → ab`. The sort is **stable** and respects the written order for equal cases: in `au 2 au 1` the pair `au 2` precedes `au 1`.

### 11.3 Candidates (same rule in every pair)
The candidates are all the pairs of the antecedent block, in reverse canonical order. For a given block:

| Anaphora | Rule |
|---|---|
| `~` | The first pair that does not hold a sema anaphora, i.e. the last canonical glyph of the antecedent block |

- Chains of `~` are followed down to the final value (R9.1).
- Every glyph, labels included, is a valid antecedent.
- The number of a candidate is the effective number of its pair (§6.1). For a `def` variable in a case pair it is the variable's number (R9.4).

### 11.4 Number
The number written in the tile prevails. When it is omitted, a case pair inherits the antecedent's number (Q18); the signal pair follows rule 1B (§6.1).

### 11.5 Sema anaphora (R8)
- `~~` refers to the first block of the search order.
- Without an antecedent: ERR_REFERENCE.
- `~~` behaves in the same way in **every** even slot, slot 2 included. It resolves to the antecedent sema (`SemaAnaphora { clause }`, value `Sema(clause)`) and never copies or merges the case pairs of the recalled sema into the current block. Every block is validated only on its own written pairs.

---

## 12. `def` variables (H1, Q11, R11, R13, R14, D2)

| Operation | Tile | Effect |
|---|---|---|
| Assignment | `a`, `au`, `am`, `anum` | Creates the variable. On an already live variable it gives ERR_SEMANTIC; a number conflicting with `def[n]` gives ERR_SEMANTIC. |
| Modification | `abfi` (no number, D2) | New value |
| Union | `absi` (no number) | Adds a value |
| Deletion | `abse ~` | Removes the variable; `~` is a marker, not an anaphora (R14) |

- The name is the **whole token** of the slot whose role is Glyph (§2), whatever its characters (`§`, `~`, `~~` included). A slot with the Anaphora role is not a name (`DefNameNotAGlyph`, ERR_SYNTAX). In `abse ~` and `end ~` the operand must have the marker role (unannotated `~`); an operand with the Glyph role gives `UnbindRequiresTilde` / `EndRequiresTilde`.
- At least one operation is required, otherwise ERR_SEMANTIC.
- A `def` must always be followed by a connector (`y` or `:`).
- Using `abfi`, `absi` or `abse` on a non-existent variable gives ERR_REFERENCE. Using them with a `def[n]` whose number differs from the variable's gives ERR_SEMANTIC.
- **R13**: while the variable is live, every glyph equal to its name (whole token), in any pair of a relational block, is a reference to the variable.

Canonical example: `/^ defu dev a sensore§ottico y defu dev abfi sensore§termico y ta leggere nu dev y defu dev abse ~ ^/`

---

## 13. `abdur=<N>` loops (B2, L1–L5)

- `abdu` is **only** the temporal complement.
- `abdur=<N>` is the base form; for morphological symmetry `abduru=<N>`, `abdurm=<N>`, `abdurnum=<N>` are allowed. It repeats **the clause and its whole subtree** N times in the plan.
- **Several loops in one block (v4.1):** several `abdur=<N>` tiles in the same clause form loops **nested in written order** (N₁ × N₂ × …): the first written is the outermost.
- The glyph following the tile is `~` (structural marker) or any glyph. Its value is **never** read for the count.
- The form `abdur` without `=<N>` is a **loop control marker**. It is allowed in a subordinate clause (depth ≥ 1) whose signal carries a conditional that is **written or inherited** from the parent (v4.1, §10): `tazato x : ta c abdur ~` is valid. An indicative tense written on the subordinate cancels the inherited conditional (override). Outside this context it gives ERR_SCOPE. The protocol validates its structure; evaluating the condition belongs to Apricot Logic (L4).
- Exceeding `max_loop_iterations` (default 1000) with N, or with the steps generated by nested loops, gives `LimitExceeded{LoopIterations}`, i.e. ERR_SCOPE.

**Execution plan** (`plan::build_execution_plan`):
- `ExecutionPlan { steps: Vec<Step { clause, iteration, targets }> }`. `iteration` holds one index per loop enclosing the step, outermost first; the loops of one clause follow the written order.
- Steps follow the tree order: each block first, then its children, roots in order.
- Control subordinate clauses are emitted only once, before the repeated body.
- `def` and portability blocks generate no step.

Examples:
- `/^ ta x abdur=2 ~ : ta y abdur=2 ~ ^/` produces x[0], y[0,0], y[0,1], x[1], y[1,0], y[1,1];
- `/^ ta x abdur=2 ~ abdur=3 ~ ^/` produces x[0,0], x[0,1], x[0,2], x[1,0], x[1,1], x[1,2].

---

## 14. Structural sets (B3, S1–S3)

The reference set is the `base:` taxonomy of the enabled keys plus the values of the `def` variables. A clause has targets only if it has a quantifier or a `cup`, `cua` or `cuo` comparative.

| Element | Operation |
|---|---|
| Accusative pairs (`a`, `au`, `am`, `anum`) | Base set |
| Nominative pairs (`n`, `nu`, `nm`, `nnum`), **only if the block has no Accusative** | Base set (intransitive or reflexive quantified clauses) |
| `all` / `some` | Expand base and unions with the enabled descendants |
| `cup` / `absi` | Union |
| `cua` / `abpa` (with `cua`, the `absi` operands) | Intersection with the operand and its descendants |
| `cuo` / `abse` (with `cuo`, the `absi` operands) | Subtraction of the operand and its descendants |

- **Segmented operand** `<K1>§<K2>`: identifies the **enabled** DTI record with `base:K1` and `param:K2`. Longer chains `<K1>§<K2>§…§<Kn>` are reduced from the left (`(K1§K2)§K3`…). If the compound record does not exist among the enabled entries, or is not unique, the plan returns `PlanError::Composite`. Example: `sensore§ottico` → the record `base:sensore param:ottico`.
- The order of application is: union → intersection → subtraction. The result is sorted by key, without duplicates.
- Mode: `all` or no quantifier → `All`; `some` → `AtLeastOne` (the list stays deterministic).

Canonical examples:
- `/^≈ deepall attivare nu sistema am sensore abse sensore§ottico ^/`
- `/^≈ deepall avvio nm sensore abse sensore§ottico ^/` (base on the Nominative)
- `/^≈ deepall attivare nu sistema am sensore abpa sensore§ottico ^/`
- `/^≈ deepallcuo attivare nu sistema a sensore absi sensore§ottico ^/`
- `/^≈ deepallcup attivare nu sistema a sensore§ottico absi sensore§termico ^/`

---

## 15. Diagnostics

| Code | Examples |
|---|---|
| ERR_SYNTAX | Invalid token or structure; non-canonical spaces; signal tile without glyph (at the end of the input); unclosed period; constraints of `def` and of the portability tiles; glyph with invisible characters (D9) |
| ERR_ORDER | Determiners out of sequence or duplicated (also temporal + conditional) |
| ERR_REFERENCE | Anaphora without antecedent; unknown anchor; undefined variable |
| ERR_SEMANTIC | Written comparative without Accusative; `def` without operations; reassignment; number conflict |
| ERR_SCOPE | Level skip; synchronization without antecedent; misplaced portability; unclosed anchors; limits (depth, loops, sizes); Q22; `abdur` without a written or inherited conditional |
| ERR_CASE_AMBIGUITY | **Only** a case tile without the following glyph (at the end of the input, §2) |

- Ingestion is **atomic**: with any diagnostic the session stays unchanged, and the errors of both stages are returned sorted by position.
- DTI, policy, Ghost, plan and editor have error types **separate** from the 6 grammatical codes (Q26).

### 15.1 Downgrade to warning (v4.1, V9)
- **By default every code is a blocking error** (`Severity::Error`).
- The user may downgrade, **at their own risk**, every single code to a warning (`Severity::Warning`): `ParserConfig::warnings` (`WarningCodes`, with `with(code)` / `without(code)`) and the `[warnings]` section of `settings.toml` (§21.2), where every code is set individually. A code that is not listed stays an error.
- Structural errors, which prevent building a complete and coherent tree, stay **always blocking** (`DiagnosticKind::is_structural`): every ERR_SYNTAX and ERR_ORDER case, `def` without operations, portability outside subordination, exceeded limits, DSP level skip, `end` without anchor, unclosed anchor, case tile without glyph.
- A downgraded code does not reject the period in `Session::ingest` (the signature does not change): the diagnostic is read from `AnalyzedPeriod::warnings()` (`Diagnostic { severity, .. }`, `is_warning()`), assigned to the period whose text contains it, and appears in the JSON export and in the CLI (`--warn=<CODE>`).

---

## 16. Two-level DTI (Phase B)

### 16.1 Architecture (D1)
| Level | Content |
|---|---|
| **Universal snapshot** | Read-only, replaceable as a whole, one exchange language |
| **Local overlay** | Enabled entries (`use`), local forms, local concepts (`new`), labels (`label`, v4.1), key policy |

The external universal registry holds every language. The local snapshot is updated with the exchange language only, because keys are immutable.

### 16.2 Key (D2, B1, B2, B4)
- 32 characters `[a-z0-9]`: 31 random characters plus **one Luhn mod 36 check character**, which detects any single-character substitution.
- **The first character is always a letter `[a-z]`** (then 30 characters `[a-z0-9]` and the check character). In the indexed form a token starting with a letter is a key and a token starting with a digit is a literal numeral. The distinction is O(1) and does not collide even with 32-digit numbers. `DtiKey::parse` and `DtiKey::from_random_body` reject a leading digit with `KeyError::FirstCharacter`.
- The key is **per concept**, not per glyph.
- A key is never modified, reused or deleted; deprecation is expressed with `superseded_by:`.
- Random generation happens **outside the crate**. `DtiKey::from_random_body` computes the check character.

### 16.3 `.awdti` format
```text
#awdti 1
#role universal
#version 42
#lang en
#numeral base:10 digits:<K0>,…,<K9>
rec <KEY> <primary> [display:"…"] [syn:a,b] [base:<KEY>] [param:<KEY>] [morph:+AGGL,+CONTR,+UNION,+AGGL_NUM] [superseded_by:<KEY>] [attr:k§v]
```
```text
#awdti 1
#role local
#universal 42
#lang it
#policy degrade|unknown|reject
use <KEY> [form:<primary>] [display:"…"] [syn:…]
new <KEY> <primary> [base:<KEY>] [display:"…"] [syn:…] [morph:…]
label <KEY> <form>
```

Fields are separated by a single space. Only the values of `display:` and `attr:` may contain spaces inside `"…"`; every other field (kind, key, forms, `form:`, `syn:`) is read literally, so a form may contain any visible character, `"` included.

**Load-time checks:**
- mandatory headers and format version;
- valid forms (v4.1): the primary form, the local `form:`, the `syn:` synonyms and the labels are tokens of **any visible character**, without exceptions (`§`, `~`, `~~`, delimiters and `:` included; a form with `§` is one glyph, §17.1); the role with which the form appears in a slot is fixed by the annotation of the slot (§2); only the empty string, the space and invisible characters are rejected (`DtiErrorKind::InvalidForm`). The same holds for the `.dti.bin` binary encoding. `QualifiedForm` is removed. Human multiword forms use `display:"verde smeraldo"`; conceptual composition uses `base:` and `param:`;
- **unique primary form per language** (B3); synonyms and `display` may apply to several concepts;
- duplicate keys and orphan references;
- cycles in `base` chains;
- **anti-spoofing**: a `new` or a `label` cannot reuse a universal key;
- `label`: only in the local file (in the universal one it gives `UnknownRecordKind`), no tag allowed, key unique in the overlay, form unique among labels (`DuplicateLabel`); the same form may also exist as a glyph (`use`/`new`);
- **census warning** (never an error): if the form of a local glyph (`use … form:`, `new`) or of a label is already the primary form of another universal glyph in the exchange language, `LocalOverlay::census_warnings()` returns a `CensusWarning { line, kind, form, universal }`, so that the user does not unknowingly send a label without knowing that the glyph exists;
- `use` on a deprecated key: error naming the successor (B6);
- `#universal` different from `#version`: error (B11);
- `#numeral` with base 2–10, as many digits as the base, existing keys.

The `attr` tag is opaque (B8). The `morph` flags are **actively validated** in stage 3 and in the editor (§17.1, B9). One language per overlay (B7).

### 16.4 `.dti.bin` binary format (dual architecture C1)
Next to the `.awdti` text parser, the `dti` module compiles and loads a deterministic binary format (pure Rust, no dependency, `#![forbid(unsafe_code)]`). All values are little-endian.

| Part | Content |
|---|---|
| Header | magic `AWDTIBIN` (8 bytes), format version `u32` (= **2** in v4.1), kind `u8` (0 universal, 1 local), **FNV-1a 64 hash** of the `.awdti` source bytes |
| Universal metadata | `#version` `u64`, `#lang`, `#numeral` (flag, base, digit keys), record count `u32` |
| Local metadata | `#universal` `u64`, `#lang`, `#policy` `u8`, record count `u32` |
| Universal sections | One column per field, in record order: keys, primary forms, `display`, synonyms, `base`, `param`, `morph`, `superseded_by`, `attr` |
| Local sections | Kind (`0` `use`, `1` `new`, `2` `label`), keys, forms, `display`, synonyms, `base`, `morph`. The `label` records follow the concept records, in key order. |

- **Encodings:** key = 32 ASCII bytes; string = `u32` length + UTF-8; optional = `u8` tag + value; list = `u32` count + items; `morph` = bits (1 AGGL, 2 CONTR, 4 UNION, 8 AGGL_NUM).
- **Indices:** they are not stored and are rebuilt at load time.
- **Same validation as the text, without text:** loading builds the model directly from the decoded columns, without ever producing, formatting or re-parsing `.awdti` text. It uses the **same functions** as the text parser:
  - per-record checks: tags allowed by the record kind, valid forms (`InvalidForm`), unique synonyms, well-formed `display` and `attr`;
  - header values: `#lang` and `#numeral` (base 2–10, one digit per base);
  - cross checks: existing references (`base`, `param`, `superseded_by`, `use`), no `use` on deprecated keys, no `base` cycles, anti-spoofing (`new`/`label` on a universal key), unique primary form, unique labels. Census warnings are recomputed (with the record ordinal as line) and take no part in model equality.
- The diagnostics of a binary (`Invalid`) report the record ordinal (1, 2, …) as line, and 0 for the metadata.
- The model is then re-encoded and must match the input byte for byte, otherwise `NonCanonical`: only the canonical encoding is accepted.
- **Determinism:** the same model always produces the same bytes, and `from_binary(to_binary(m)) == m`.
- **Robustness:** every read is bounds-checked; counts are capped by the remaining bytes before any allocation; trailing bytes are rejected; maximum size `MAX_BINARY_BYTES`.

**API** (`apriwave::dti`):
- `UniversalSnapshot::to_binary(&self, source_hash) -> Vec<u8>` and `UniversalSnapshot::from_binary(bytes) -> Result<(Self, u64), DtiLoadError>`.
- `LocalOverlay::to_binary(&self, source_hash)` and `LocalOverlay::from_binary(bytes, &universal)`. The overlay is always re-validated against the given snapshot.
- `UniversalSnapshot::parse`, `LocalOverlay::parse`, `Dti::from_parts` (which re-checks the snapshot/overlay pairing) and `fnv1a_64`.
- `load_or_compile_universal(source, Option<binary>)` and `load_or_compile_local(source, Option<binary>, &universal)`. If the binary is valid and has the same FNV-1a hash as the source, it is loaded from the binary (`None`). Otherwise (absent, corrupt or stale binary) the text is compiled and the new bytes are returned (`Some`).
- File variants `load_or_compile_universal_path` and `load_or_compile_local_path`, **only in the `apriwave` host crate** (`apriwave::dti`, error `DtiFileError { Io, Load(DtiLoadError) }`):
  - the cache sits next to the source (`foo.awdti` → `foo.dti.bin`, `binary_cache_path`);
  - a cache read error counts as a missing cache;
  - the cache is rewritten through a temporary file and a rename.
- `DtiLoadError` errors: `BadMagic`, `UnsupportedVersion`, `WrongKind`, `Truncated`, `TrailingBytes`, `InvalidUtf8`, `LengthOverflow`, `TooLarge`, `InvalidKey`, `Malformed`, `NonCanonical`, `Invalid`, `Source`. The buffer functions live in `apriwave-core` and do not use the file system.

### 16.5 Lookup
| Function | Behaviour |
|---|---|
| `resolve_primary` | Primary forms only (local and exchange language, no priority between languages). It is the only lookup used by stage 3: canonical text admits only primary forms. |
| `resolve_form` | Primary forms first, then synonyms and `display`. Used only by the editor to normalize (§21.1). Returns a unique, ambiguous (the editor asks) or unknown result. |
| `compounds(base, param)` | Enabled entries with that `base:`/`param:` pair (segmented set operands, §14) |
| `label(form)` | Key of the local label with that form (v4.1); `overlay().labels()` lists the labels |
| `resolve_key` (D3) | Returns `Enabled` for enabled or local keys. For the others it follows the policy: `degrade` walks up the `base:` chain to the first enabled entry, `unknown` reports them as unknown, `reject` rejects them. |

### 16.6 Labels (v4.1, V5)
- A **label** is an element stored in the **local DTI** with the *Label* type (`label <KEY> <form>`), absent from the universal DTI. It replaces the old quoted labels, removed from the grammar.
- **Coexistence:** the same value (e.g. `roma`) may exist at the same time in the local DTI as a glyph (`use`/`new`) and as a label.
- **Check against the universal DTI:** when a glyph or a label is registered, if the term is already a universal glyph in the exchange language a census warning is produced (§16.3).
- **Mandatory selection (V5, amendment of 15/11/2026):** when a form exists in the local DTI as a label (alone, or together with a glyph), the user **must select from the menu** whether it is a **Glyph** or a **Label** in order to write it (§21.3). The choice is recorded per slot in the session at ingestion (`Session::ingest_with_choices`, or `Session::set_slot_kind(slot, SlotKind::{Glyph, Label})` afterwards) and decides colour and transmission. Every slot therefore always has a deterministic classification (`Session::effective_slot_kind`): in programmatic or CLI flows without an interactive selection, an unmarked slot has the stage-1 role (§2) and, when that role is Glyph, counts as `SlotKind::Glyph` (default). There is no unresolved "glyph or label" case: `IndexError::GlyphOrLabel` is removed.
- **Editor:** a label is written and shown without quotes or prefix, in the label colour. A slot marked as a label is an **inert string**, never touched by the pre-processor (no normalization, no multiword phrase, no glyph check); a slot marked as a glyph follows the DTI rules.
- **Homographs and synonyms (a non-issue):** the protocol ignores semantic content. Even if two identical labels (for example the same acronym) denoted two different real-world objects, the context belongs to sender and receiver: the protocol transmits the same string `LVAAAA<form>` anyway. Likewise, a form that is also a synonym of a glyph creates no conflict: the selection of the slot decides.
- **Transmission:** in the indexed form a glyph travels as its `DtiKey` (32 lowercase characters `[a-z][a-z0-9]{31}`), a label as **`LVA<SUBTYPE><form>`** (e.g. `LVAAAAroma`): `LVA` (3 characters) identifies the label, the subtype is 3 uppercase letters `[A-Z]{3}` (`AAA` = base label, the only one generated by this implementation). The prefix is generated by the sending machine, never typed by the user. Keys are lowercase and numerals start with a digit, so the uppercase prefix gives an O(1) distinction with no collision.
- **Receiver:** a token starting with `LVA` is **always and only a label**, even when absent from its own DTI. If the form is also a glyph of the receiver's DTI, the label stays a label but is **highlighted** to report the homonymy (`IndexedLabel { slot, form, subtype, homonym }`, §17.3).
- **Unknown subtypes and self-update:** a subtype other than `AAA` means that it was provided for by the grammar of the sending machine. The receiver **does not reject the message**: it treats the token as a valid label (`LabelSubtype::Known(code)` if `AAA` or already registered, otherwise `LabelSubtype::Dynamic(code)`), registers the new code in the subtype table of the session (`Session::register_label_subtype`, `label_subtypes`; the table survives `compact`) with a single informative note (`NewLabelSubtype { slot, code }`), and keeps the 3 characters if it re-renders the message in indexed form. Only a truncated or corrupted token (shorter than 7 characters, or with a subtype that is not `[A-Z]{3}`) is an error (`MalformedLabel`).

> [!IMPORTANT]
> **Security note of the grammar — safe-environment execution for unregistered labels.**
> - Labels are opaque strings that travel with the prefix `LVA<SUBTYPE><form>` (default `LVAAAA<form>`).
> - If a label is not correctly registered in the local DTI of the receiving node (diagnostic note `UncensusedLabel`, or an external label received under `label allow`), or carries a dynamic subtype not yet classified (`LabelSubtype::Dynamic`), the grammar prescribes that the runtime or host system handle and execute it **exclusively inside an isolated safe environment** (sandbox / purely passive data context), **with no privilege of direct execution** on system commands or physical actuators.

---

## 17. Stage 3 (Phase C)

### 17.1 Glyph → key resolution (B4, B5, B9)
1. Canonical text (stages 1, 2 and 3) admits **only primary forms**, unique 1:1 per language (B3). Synonyms and `display` forms are normalized exclusively by the editor (§21.1); stage 3 searches only the primary forms.
2. **DTI priority (v4.1, V3):** the **whole token** is looked up first among the primary forms, even when it contains `§`: a registered `a§b` is one glyph with one key. Only when the whole token is not a form is a path `s1§s2§…§sn` resolved **segment by segment** into `<K1>§<K2>§…§<Kn>` (with `+UNION`); an empty segment is an unknown form (`UnknownForm`).
3. **Glyph or label (deterministic):** a token already in transmission label form (`LVA<XYZ><form>`, machine generated) stays unchanged; otherwise the classification of the slot in the session decides: marked `Label` → label, otherwise (marked `Glyph` or unmarked) → glyph resolved in the DTI. The DTI check is therefore performed on every slot with the Glyph role, whatever the characters of the token (delimiters placed in an even slot, `~` / `~~` annotated Glyph: e.g. a registered `~` → its `DtiKey`).
4. If the snapshot declares `#numeral`, an all-digit atom is a **literal numeral**: every digit must be smaller than the base (`InvalidNumeral`).
5. An entry that is not enabled is an error suggesting `use <KEY>`.

**Active validation of the `morph` flags** (`ResolveError::MorphViolation { form, key, flag }`, with `MorphFlag::{Union, Aggl, Contr, AgglNum}`):

| Flag | When it is checked | Who must carry it |
|---|---|---|
| `+UNION` | A qualified glyph `a§b` resolved by segments (n ≥ 2) | Every concept joined by `§` (numerals are not concepts and are exempt: in `30§celsius` only `celsius` needs it) |
| `+AGGL` | Record with `base:` and `param:` whose form is an atom equal to the concatenation of a primary form of `base` and one of `param` (`cagn` + `one` → `cagnone`) | Both components (`base` and `param`) |
| `+CONTR` | Record with `base:` and `param:` whose form is an atom different from the concatenation (`cucciolo` = dog + small) | Both components |
| `+AGGL_NUM` | Every digit of a literal numeral | The concept of the digit (`#numeral … digits:`) |

- Agglutinated or contracted forms must be registered as records. Stage 3 never segments an unknown atom automatically: in the indexed form a segmented agglutination would be indistinguishable from a `§` union.
- The same validation applies on input (`validate_indexed_input`: `+UNION` on the keys joined by `§`, `+AGGL_NUM` on the digits) and in the editor (§21.1).

### 17.2 Expanded form (D8)
It is a **resolution view** for people, not the exchange form. The expanded form:
- contains neither `def` blocks nor portability blocks;
- uses explicit `:` for every depth ≥ 1;
- substitutes variables and `~` with their value. In **case pairs** it rewrites the tile with the effective number (`n ~` → `nu humanum`; `a gruppo` with `defm gruppo` → `am …`). The signal tile stays as written: with the explicit number if there was one, otherwise with the number inherited from the parent or Dual, which a new reading rebuilds identically (rule 1B). Substituting `~` or a variable in slot 2 never fails;
- **multi-valued variable in a case pair**: the original tile (with the effective number) for the first value, then one `absi <value_i>` pair for every further value. With `dev = [s1, s2]`: `au dev` → `au s1 absi s2` (V6);
- **`~~` stays `~~`** in any even slot: removing `def` and portability blocks never changes the latest sema, because those blocks are not relational clauses.

When something cannot be expressed, the result is an **export error**, never an invalid output:

| Error | Cause |
|---|---|
| `MultiValuedVariable` | Multi-valued variable in the signal pair (slot 2) |
| `InexpressibleDepth` | Depth that requires anchors |
| `InexpressibleValue` | Value that cannot occupy a glyph position |
| `EmptyExpansion` | The period contains only `def` blocks |

`InexpressibleNumber` and `InexpressibleSema` **no longer exist**.

### 17.3 Indexed form and channel (D4)
- **The M2M indexed form does not depend on the expanded form.** It is the **written** canonical form of the validated period, with every DTI glyph replaced by its `DtiKey`. The `def` blocks, the portability tiles (`anchor`, `anchorrel`, `rel`, `reloff`, `end ~`), the anaphora (`~`, `~~`), the tiles and the numerals stay intact; labels become `LVAAAA<form>`.

| Written element | Indexed form |
|---|---|
| Glyph in slots and in `def` values | The key of the whole token if it is a primary form (also with `§`); otherwise one key per segment: `<K1>§<K2>…` |
| Label (v4.1) | Slot marked `Label` → `LVAAAA<form>`; token already `LVA<XYZ><form>` → unchanged (subtype kept) |
| Already indexed glyph (retransmission) | A glyph whose segments are already enabled keys (or numerals) stays unchanged; a session symbol that is already a key stays that key: a period received in indexed form is re-rendered identically |
| `def` variable name and its references (R13: glyph resolved by stage 2 as a live variable) | **Session symbol**: the `DtiKey` if the name is the primary form of an enabled entry, otherwise an **ephemeral key** (also when the entry exists but is not enabled) |
| Anchor id (`anchor`, `anchorrel`, `rel`, `reloff`) | Session symbol of the **whole token** (`porta§uno` → `<K_eph(porta§uno)>`; enabled `porta` → `<K_porta>`) |
| `~`, `~~`, numerals | Unchanged |

**Ephemeral session key** (`DtiKey::session_symbol(name)`): computed with the internal SHAKE256 over `len(domain) u64 LE ‖ domain ‖ name`, with domain `apriwave-session-symbol-v1`.
- The bytes are read with unbiased rejection sampling: the first character from `[a-z]` (byte < 234, mod 26), the next 30 from `[a-z0-9]` (byte < 252, mod 36), then the Luhn mod 36 check character.
- The key is deterministic: the same name always gives the same key, so the name and its references coincide.
- A primary form shared by two enabled entries in the two languages remains ambiguous (B3).

- **100% of the periods valid at stage 2** are transmissible, including periods made only of `def` blocks or with relative anchors. The only condition is that every ordinary glyph is a primary form of the DTI or a slot marked as a label. Every slot travels according to its role, for any token: Glyph → `DtiKey`, Label → `LVAAAA<form>` (e.g. `LVAAAA~`, `LVAAAA~~`), Anaphora → `~` / `~~`, resolved identically on both sides.
- `render_wire(WireMode::Lexical)` returns the written canonical form with lexical glyphs; `WireMode::Indexed` returns the same structure with keys. The channel mode is agreed **out of band**.
- M2M exchange happens only in the indexed form.
- `validate_indexed_input` checks a received period:
  - a token starting with `LVA` is a label: any `[A-Z]{3}` subtype goes into `IndexedReport::labels` (with `subtype`, and with `homonym` when the form is also a glyph of the receiver's DTI); a subtype not known yet produces the note `IndexedReport::new_subtypes`, never an error; a truncated or corrupted token gives `MalformedLabel`;
  - every other ordinary glyph (slots, `def` values) must be a valid key, resolved with the policy, or, if it starts with a digit, a valid numeral (`InvalidNumeral` otherwise);
  - it also checks `+UNION` and `+AGGL_NUM` (`MorphViolation`);
  - **local symbols** are accepted as valid keys without requiring them to be in the snapshot: the keys declared in a `def` block and used in its operations, the R13 references to the variable (recognized by the receiver's stage 2), and the anchor ids (`anchor`/`anchorrel` and the following `rel`/`reloff`);
  - an ephemeral key in an ordinary position is an unknown key.
- On receipt the sender's glyph/label choices are already encoded in the text: no menu is needed.
- `receive_indexed_input(&mut Session, id, dti)` runs `validate_indexed_input`, registers the new subtypes in the session and marks the slots of the received labels as `Label`; on error the session does not change.

### 17.4 Views
| Mode | Content |
|---|---|
| `HumanOriginal` | Written structure with the `display` forms ("verde smeraldo") |
| `CanonicalLexical` | Written structure with the primary forms (`verdesmeraldo`) |
| `DtiIndexed` | Indexed form (written structure with keys) |

Layout: `Linear` on one line, or `CStyle { indent }` with one clause per line indented by depth.

---

## 18. DTI Policy Manager (B6, P1–P5)

### 18.1 `.awpolicy` format (written on keys, hence language-independent)
```text
#awpolicy 1
#universal <snapshot version>
#default allow|deny
signal <KEY> ta,ka,deep
role <KEY> n:<K>,<K> a:<K> d:… g:… ab:…
threshold <KEY> cha|chu|col min:<u64> max:<u64>
qualifier <KEY> <K>,<K>
require <KEY> zato|quo
label deny | label allow:<K>,<K>
```

### 18.2 The 5 filters (deterministic O(log n) worst-case lookup)
The rules are indexed in `BTreeMap`/`BTreeSet`: every lookup costs **O(log n) in the guaranteed worst case**, with a deterministic iteration order and no HashDoS exposure. A hash table offered O(1) only amortized (worst case O(n)) and a non-deterministic iteration: in hard real-time systems the worst case is what counts. Comparing interned atoms (`Symbol`) stays O(1).

All rules refer to the **glyph of the signal pair** (P1), except `qualifier`, which applies to every path with that root.

| # | Filter | Rule |
|---|---|---|
| 1 | Signal Mask | Allowed signal heads. For a secondary signal both the secondary and the primary must be allowed. |
| 2 | Role matrix | Allowed keys for every base case (`n`, `a`, `d`, `g`, `ab`) |
| 3 | Metadata limits and thresholds | Allowed `§` qualifiers; numeral of the Accusative within [min, max] with `cha`, `chu` or `col` (P5) |
| 4 | Mandatory subordinate | A child with a written conditional (`zato`) or with the `quo` secondary |
| 5 | Enhanced Security | Labels (v4.1) are forbidden, or allowed only in the clauses whose signal glyph is listed |

- With `#default deny` the policy is an allow-list: a glyph without a `signal` rule is an unauthorized command, and a base case missing from its `role` rule is an unauthorized role. With `allow`, only the written rules restrict.
- Without a `label` rule, labels are forbidden if Enhanced Security is active or if the default is `deny` (violation `PolicyRule::Label`).
- A slot marked `Label` in the session, or an `LVA<XYZ><form>` token (for example traffic filtered by the Legacy Shield), is a label; every other slot is a glyph, even when the form is registered as a label.
- Values reached through variables and anaphora are checked on the **resolved values**; a glyph that cannot be resolved in the DTI is a violation.
- A recalled sema (`~~`) counts, in any slot, as the **signal glyph** of the antecedent sema (following any sema chain). For example `: deep ~~` is checked on the key of the recalled signal and cannot bypass the Signal Mask.
- Violations have the form `PolicyViolation { rule, span }` (P4).

> [!IMPORTANT]
> **Security note of the grammar — safe-environment execution for unregistered labels.**
> - Labels are opaque strings that travel with the prefix `LVA<SUBTYPE><form>` (default `LVAAAA<form>`).
> - If a label is not correctly registered in the local DTI of the receiving node (diagnostic note `UncensusedLabel`, or an external label received under `label allow`), or carries a dynamic subtype not yet classified (`LabelSubtype::Dynamic`), the grammar prescribes that the runtime or host system handle and execute it **exclusively inside an isolated safe environment** (sandbox / purely passive data context), **with no privilege of direct execution** on system commands or physical actuators.

---

## 19. Apricot Legacy Shield (B5)

`LegacyShield::filter(&mut self, input) -> Result<String, Vec<ShieldViolation>>`:
- it is a filter **without network I/O**;
- it applies stage 1, stage 2, stage 3 and the policy, with **Enhanced Security always active**;
- when it blocks an input it undoes every change to its own session, because ingestion is transactional (`Session::ingest_checked`);
- when it accepts an input it re-emits the **reconstructed canonical form**, never the original text.

*Virtual patching* consists in updating only `.awdti` and `.awpolicy`.

---

## 20. Ghost Protocol v2 (B4, G1–G7)

A steganographic stream cipher based on semantic mapping: an indexed period becomes another GHLIP period, **valid at stage 1 and at stage 2**, which stage 2 resolves identically (same structure, same antecedents, same variables). Decryption is exact.

| Element | Specification |
|---|---|
| Suite | `CryptoSuite { Ascon, Skein512, Blake3, Shake256 }`. **SHAKE256** (FIPS 202) is internal, dependency-free, verified against `hashlib`. Ascon, Skein-512 and BLAKE3 are optional features (`ghost-ascon`, `ghost-skein`, `ghost-blake3`); a suite that is not compiled in gives `SuiteUnavailable`. |
| KDF | **Argon2id** (feature `ghost-argon2`). Default: RFC 9106 *second recommendation* (t=3, p=4, m=64 MiB, 16 B salt, 32 B tag); the first one is also available (t=1, p=4, m=2 GiB). `Shake256KeyDerivation` must be used only with high-entropy secrets; it is not a password hash. |
| Keystream | XOF(label, master key, nonce, snapshot version, class, position P), with length-prefixed fields. Every class has its own position counter. |
| Domain (G4) | Non-deprecated keys of the universal snapshot S, sorted by bytes, permuted with **Fisher-Yates** (uniform draws by rejection). |
| Numerals | Every digit is encrypted with a Fisher-Yates permutation over `0..B-1`, where B is the base of `#numeral base:B`, class `numeral-digit`. The ciphertext is a valid numeral of the same base and length. Without `#numeral` → `NoNumeralSystem`; digit ≥ B → `DigitOutOfBase`. |
| Ephemeral keys (F) | Keys outside the snapshot (K \ S: `def` names, anchor-id atoms and R13 references without an enabled DTI entry) are encrypted with a **10-round Feistel network** on the 31-character body, in mixed radix: half L ∈ Z₂₆·₃₆¹⁵, half R ∈ Z₃₆¹⁵, each in a `u128`. The round function is the suite keystream with class `ephemeral-key`. The Luhn character is recomputed; **cycle-walking** keeps every image outside S. On decryption, an O(1) membership test on S selects the inverse Fisher-Yates or the inverse Feistel. An ephemeral key is allowed only in a bound position (`def` name, anchor-id atom, R13 reference); elsewhere → `UnboundEphemeralKey`. |
| Symbol table (rule 5, G1) | **Injective** plaintext ↔ ciphertext mapping, reference-counted. It holds the atoms of: the block being written; the blocks visible as antecedents (active DSP stack, level-0 blocks of the period, blocks kept by `/^/`), with the value paths of their pairs; the live `def` variables (name and values, until `abse ~` or `^^/`); the open anchors (id atoms, until `end ~` or the end of the period). A pruned branch or a close with `^/`/`^^/` releases the entries. Two atoms with different plaintext never receive the same cipher key while they are visible. |
| Per-position bijection | σ_P is a *chain walk* over the base permutation τ_P (Fisher-Yates, digits or Feistel), consistent with the table: σ(p_j) = c_j for the live entries; otherwise y = τ(x) and, while y equals a bound cipher c_j, it continues with y = τ(p_j). It is bijective and exactly invertible in at most \|B\|+1 steps; with an empty table σ = τ. As a result, `def` names, R13 references, `abfi`/`absi`/`abse` and `anchor`/`rel`/`reloff` ids automatically take the cipher atoms of what they refer to, and no other atom can capture the reference. |
| What is encrypted (G5) | Every key atom and every numeral of the indexed form. `~`, `~~`, `abse ~`, `end ~` and the `def` tiles (never permuted) stay unchanged. Labels (`LVA…`, v4.1) remain **rejected** (`GhostError::Label`), so that no plaintext is exposed. |
| `permute_grammar_tiles` (G2) | Also permutes: the primary signal only within the class {`ta`, `to`, `ka`, `ko`, `ot`} (`oh` stays fixed because it governs the DSP, §9.3); the tense within the two separate classes {omitted, `ato`, `alt`, `alv`, `att`, `atv`, `asu`} and {`zato`, `zalt`, `zalv`, `zatt`, `zatv`, `zasu`}; the number of the case tiles and the number of the signal tile (number class: omitted, `u`, `m`, `num`). |
| `GhostSession` (C) | An ordered stream between two endpoints. Each endpoint keeps the sequence counter, a plaintext and a ciphertext `Session` (stage 2 in the context of the previous periods) and the symbol table. API: `GhostSession::new(params)`, `encrypt(plain, dti, key, nonce) -> GhostEnvelope`, `decrypt(&envelope, dti, key) -> String`. Every message is atomic: on error the state stays unchanged. The sender checks that the ciphertext is valid at stage 2 with identical resolutions. The plaintext must be valid at stage 2 in its own context, otherwise `Stage2 { diagnostics }`. `encrypt_period`/`decrypt_period` remain as wrappers over a fresh session, valid only for self-contained periods. |
| Integrity (G3) | `GhostEnvelope { suite, snapshot_version, sequence, nonce, ciphertext, mac_tag }`. The MAC covers sequence, nonce and ciphertext (plus suite, version and flags) and travels out of band. Constant-time comparison. |
| Anti-replay and ordering (G6) | The receiver accepts only the next sequence number: lower → `Replay`, higher → `OutOfOrder { expected, found }`. The 128-bit nonce is still provided by the caller. Decryption proceeds as: suite → version → MAC → sequence → decryption → registration. |

**Declared limit:** while they are within the visibility scope, equal plaintext atoms have the same cipher atom. This is the price of validity at stage 2 (decision G1). Once released they become independent per position again.

---

## 21. Editor (Phase D, H7)

### 21.1 Pre-processor (`preprocess_assisted_input`)
- **Comments** `//`: they start only at the beginning of a token or right after a complete period delimiter (`^/// note`). Inside any other token `//` belongs to the token (v4.1: a glyph such as `http://x` keeps its `//`).
- **Slot Locking and slot role (single rule):** as in the parser, the role of every token in an even slot is fixed by the annotation (`SlotChoice`): Glyph or Label when annotated, otherwise the stage-1 role (Anaphora for `~` / `~~`, Glyph for every other token: delimiters, `:`, braces and `"` included). For an unannotated token with more than one possible role, the single note `SlotRoleMenu { token, span, anaphora, glyph, label }` opens the menu with the allowed roles (`anaphora`: unannotated role of the token; `glyph` / `label`: registered in the DTI as a glyph / label). The menu opens when the token is registered as a label, or when its unannotated role is Anaphora and the DTI registers it. With a Glyph `SlotChoice` the token is normalized (if it is a synonym) and checked against the DTI like any glyph.
- **Free spacing**: tabs and line breaks are reduced to a single space.
- **Braces** `{ }`:
  - they open the children of the clause just written;
  - every inner clause receives the `:` of its own depth; after a conjunction the conjunction is emitted first, then the `:`;
  - errors: hand-written `:` inside braces, portability tiles inside braces, unbalanced braces, missing connector at level 0.
- **Signal train with `_`**, only in the signal tile: `ta_zato_cha` becomes `tachazato`, with automatic reordering; duplicates stay visible to the parser.
- **Aliases** `>` `<` `=` `!=` → `cha` `chu` `col` `cla`. The ambiguous aliases `~=`, `+` and `-` are **removed** and produce a diagnostic.
- **Multiword phrases** ("verde smeraldo"), with the DTI: the longest match is searched, provided that no word of the phrase is a tile. Ambiguous result → error; entry not enabled → error with the suggestion `use <KEY>`.
- **Normalization into the primary form.** This is the only place where `syn:` and `display` are resolved, before the text enters stage 1/2. It applies uniformly to every glyph position: relational slots, `def` names and values, portability ids. It works first on the whole token (DTI priority, also with `§`), then on every `§` segment, so a `def` name and its references stay identical (R13). Slots marked as labels are never normalized; unmarked slots are glyphs and are normalized:
  - primary form (also of an entry that is not enabled) → unchanged;
  - synonym or `display` of a single enabled entry → primary form, with the `NormalizedForm` note;
  - synonym or `display` of an entry that is not enabled → `DisabledEntry` with the suggestion `use <KEY>`;
  - synonym shared by several entries → `AmbiguousForm`;
  - free identifier, completely absent from the DTI → unchanged.
- **Glyph/Label selections (v4.1):** `preprocess_with_choices(input, settings, dti, &[SlotChoice { span, kind }])` receives the user's selections (byte range of the token in the input). A token marked `Label` stays an inert string. Every annotated token (Glyph or Label) appears with its range in the produced text and its `SlotKind` in `AssistOutput::slot_choices`, to be passed to `Session::ingest_with_choices(&out.text, &out.slot_choices)`, which records the choices atomically. `preprocess_assisted_input` is equivalent to no selection (every slot is a glyph).
- **DTI check of every glyph:** every token of an ordinary even slot is checked against the DTI, also when Slot Locking turned a period delimiter or a run of `:` into a glyph. A token that is not registered produces the note `UnknownGlyph { token, span, structural }` (`structural` for delimiters and `:`, together with the missing close reported by the session): the editor highlights it as not registered and asks to register it in the local DTI as a glyph (`new`) or as a label (`label`). A token marked `Label` but not registered as a label produces `UncensusedLabel`. `reclassify_as_label` is removed.
- **Mandatory selection:** for an unmarked token that the local DTI holds as a label, the note `SlotRoleMenu` (with `label: true`, and `glyph: true` if it is also a glyph) asks the user for the menu selection (§21.3).
- **`morph` flags** (with the DTI): a glyph violating `+UNION`, `+AGGL`, `+CONTR` or `+AGGL_NUM` produces the diagnostic `MorphViolation { form, key, flag }`, with the same rules as stage 3 (§17.1).
- The signal train also recognizes the number (`ta_u_zato` → `tazatou`), which always goes last.

### 21.2 `settings.toml`
```toml
[editor]
braces = true
comments = true
signal_underscore = true
symbol_aliases = true
multiword = true
autocomplete = "auto"          # or "manual" (v4.1)

[shortcuts.comparative]
prefix = "Alt+C"
">" = "cha"
"<" = "chu"
"=" = "col"
"!" = "cla"
"~" = "cal"
"[" = "cua"
"-" = "cuo"
"+" = "cup"

[shortcuts.actions]             # v4.1
suggest = "Alt+Down"
copy_absolute = "Ctrl+C"
copy_relative = "Alt+C"

[custom_shortcuts]
"Alt+V" = "@live_variables"

[warnings]                      # v4.1, at your own risk
ERR_SYNTAX = "error"
ERR_ORDER = "error"
ERR_REFERENCE = "error"
ERR_SEMANTIC = "error"
ERR_SCOPE = "error"
ERR_CASE_AMBIGUITY = "error"
```
- `ShortcutResolver` resolves `Alt+C` followed by the symbol into the canonical syllable. `@live_variables` returns the sorted list of the live `def` variables of the session; any other string is a literal insertion.
- `autocomplete`: `"auto"` opens the suggestion menu under the text while typing; `"manual"` opens it only on request, with the `suggest` action (`Alt+Down`).
- `[shortcuts.actions]`: keys of the editor actions; chords also accept `Down`. `copy_relative` may equal the comparative prefix, because the two contexts are disjoint (§21.4). A custom shortcut equal to the prefix or to an action gives `ChordConflict`.
- `[warnings]`: every code is set individually to `"warning"` or `"error"` (§15.1); a code that is not listed stays an error. The value (`EditorSettings::warnings`) is passed to `ParserConfig::warnings`.

### 21.3 Suggestion menu (v4.1)
- `suggestions(prefix, &Session, Option<&Dti>) -> Vec<Suggestion { kind, text }>`, with `SuggestionKind::{Glyph, Label, Sema}` and at most `MAX_SUGGESTIONS` entries, without duplicates. Order: the glyphs of the session, most recent first (`def` names included), the labels of the local DTI, then the semas of the session, most recent first, whose canonical text or signal glyph starts with the prefix.
- Selecting an entry inserts `text`: the glyph or the label as is, a sema as the **canonical text of the whole clause** (`ta vedere nu io`).
- **Glyph/Label disambiguation (mandatory):** for a value that the local DTI holds as a label the menu shows the allowed entries (both if it is also a glyph) and the user must choose one to write it; if the session already has a choice for a slot with that value, the chosen entry comes first. The choice goes to the pre-processor as a `SlotChoice` and to the session with `Session::ingest_with_choices` (or `Session::set_slot_kind` on a slot already ingested). The rule applies to every form, whatever its characters (`~` and `~~` included).
- Activation: automatic (`autocomplete = "auto"`) or with `Alt+Down` (`ShortcutOutcome::Suggest`).

### 21.4 Copy with anchors (v4.1)
- `copy_subordinates(text, selection, CopyMode, Option<id>) -> CopiedBlock { text, id }` copies the selected subordinate clauses (byte range of whole clauses of canonical text; the first introduced by `:`, the others also by a conjunction). Slot Locking is respected: a `:` in an even slot is a glyph.
- `CopyMode::Absolute` (`Ctrl+C`): `<:…> anchor <id> <clauses> <:…> end ~`, with the `:` of the clauses as written.
- `CopyMode::Relative` (`Alt+C` with a selection): `<:…> anchorrel <id> <clauses> : end ~`, with the `:` rewritten relative to the shallowest clause, which gets a single `:`.
- The anchor gets the `:` of the shallowest clause, so the block pasted in place describes the same tree. When the id is omitted, the first `copy<N>` absent from the text is generated.
- `CopyError` errors: `EmptySelection`, `InvalidSelection`, `NotSubordinate`, `IncompleteClause`, `InvalidId`.
- **Disjoint contexts:** `ShortcutResolver::resolve_in(session, ShortcutContext { selection }, sequence)`. With an active selection `Alt+C` → `CopyRelative` and `Ctrl+C` → `CopyAbsolute`; without a selection, while typing, `Alt+C` opens the comparative insertion (`> < = ! ~ + -`) and `Ctrl+C` is not an editor action.

---

## 22. Functions (hook for Apricot Logic)

`FunctionEvaluator` and `Session::evaluate_and_ingest`:
- the output of the function is ApriWave text and is re-ingested atomically;
- multiple operands are written with `absi`, **never** with `§`;
- the protocol contains no arithmetic functions.

---

## 23. Crate architecture, export and CLI

Cargo workspace with two crates:

| Crate | Environment | Contents |
|---|---|---|
| `apriwave-core` | `#![no_std]` + `alloc`, zero external dependencies. Builds for bare-metal microcontrollers (`thumbv6m-none-eabi` for Cortex-M0+, `thumbv7em-none-eabihf` for Cortex-M4F) with a global allocator | lexer, parser, stage 2, printer, stage 3 (indexed and expanded forms), DTI model and `.dti.bin` on buffers, `DtiKey::session_symbol` with the internal SHAKE256, functions, plan, policy, Legacy Shield |
| `apriwave` | `std` | Re-exports the whole core under the same paths; adds the file API of `.dti.bin`, the editor, the JSON export, the CLI and Ghost (frozen) |

The maps and sets of the core are `BTreeMap`/`BTreeSet`: deterministic iteration and O(log n) worst-case lookups.

**Memory (microcontrollers with < 50 KB of RAM):**
- **Single-copy interner:** every text is allocated only once (`Vec<Box<str>>`, a `Symbol` is resolved in O(1)).
  - The index is a `Vec<Symbol>` sorted by text: binary search in O(log n) worst case; an insertion shifts at most n 4-byte symbols.
  - It uses no `Arc`/`Rc`, so the `Session` stays `Send + Sync` also on Cortex-M0+, which has no atomic compare-and-swap.
  - An ingestion rollback truncates the texts and removes from the index the symbols created after the restore point.
- **`Session::compact()`:** releases periods, analyses, bindings and interned texts, keeping the configuration.
  - It is allowed only on an empty session or on one whose last period is closed by `^^/`. That close has already cleared anaphora, `def` variables and synchronization (§3), so later ingestion behaves as without compaction.
  - Otherwise it returns `CompactError::NotHardClosed`.
  - Identifiers obtained before the call (`PeriodId`, `BindingId`, `Symbol`) become invalid.
- **Stack:** anaphora, variable and sema chains are resolved with iterative loops. The only recursions of the core are:
  - the visit of the DSP tree in the execution plan, with depth bounded by `max_dsp_depth` (default 64, configurable for small stacks);
  - variable operands in sets and in the policy, at most 2–3 levels.

  Printer, expanded form and indexed form are not recursive.

| Tool | Description |
|---|---|
| `export::export_analyzed_json` | Deterministic JSON: clauses, depths, tree, conjunction of every clause (no coordination tree), effective signal with the inheritance flags (quantifier, `ref`, comparative, tense, number), pairs with resolutions, numbers and glyph/label choice (`slot_kind`), warnings of the period |
| CLI | `apriwave-cli [--ast] [--json] [--warn=<CODE>]… "<period>"`: `--warn` downgrades a code to a warning; warnings are printed as `WARN` |

---

## 24. Implementation notes (adopted interpretations)

| # | Note |
|---|---|
| I1–I11 | As approved for stage 2 (I2 and I3 superseded by pair symmetry, §11.3) |
| I12 | After `abdur` only `~` is a marker; any other glyph is resolved normally but does not count for the loop |
| I13 | The `abdur` marker requires a **written or inherited** conditional (v4.1) and depth ≥ 1 |
| I14 | Set targets exist only with a quantifier or with `cup`/`cua`/`cuo`. Base = Accusative pairs; without Accusatives, Nominative pairs (§14). Without a quantifier the mode is `All`. |
| I15 | The loop limit counts the steps repeated beyond the first occurrence of every clause |
| I16 | Policy: the `#default allow|deny` header is mandatory; `require … zato` accepts any written conditional |
| I17 | Ghost: for Ascon and Skein the MAC uses the suite XOF with a prefixed key; the `permute_grammar_tiles` flag is bound in the MAC; at most 128 rejections per draw |
| I18 | Editor: the detailed choices are documented in the `editor` module (brace depth, malformed trains left to the parser, shortcuts with Shift tolerated after the prefix) |
| I19 | Rule 1B: an anaphora inherits the **effective number** of the antecedent pair; if the antecedent is the signal pair, that number is the one written on or inherited by the signal, otherwise Dual |
| I20 | `def` names and anchor ids are session symbols of the whole token: DTI key if they are the primary form of an enabled entry, otherwise an ephemeral key (decisions D1 and D2); the receiver accepts them as local symbols |
| I21 | Synonyms and `display` exist only in the editor: stage 3 knows only the primary forms, unique per language, so a lexical sender and an indexed receiver resolve anaphora identically |
| I22 | `morph`: a record with `base:` and `param:` and an atomic form is an agglutination if the form is the concatenation of the primary forms (local or exchange language) of its components, otherwise a contraction; the flags are checked on the components, like `+UNION` on the segments and `+AGGL_NUM` on the digits |
| I23 | `~~` behaves identically in every even slot: no pair merging, no special treatment of slot 2 (slot symmetry, §1.5) |
| I24 | Ghost: ciphertext valid at stage 2 through the injective table of visible symbols and the per-position *chain walk* bijection (decisions B, C, F, G1, G2); ephemeral keys use Feistel with cycle-walking outside the snapshot |
| I25 | v4.1 (amendment): the Glyph/Label selection is mandatory when writing; every slot is deterministic (unmarked = glyph); `IndexError::GlyphOrLabel` is removed; an explicit "label" choice also applies to a form that is not registered (with the `UncensusedLabel` note) |
| I26 | v4.1 (amendment): an `[A-Z]{3}` subtype other than `AAA` is a valid label, registered dynamically in the session with an informative note and kept on retransmission; `UnknownLabelType` is removed; only truncated or corrupted tokens give `MalformedLabel` |
| I27 | v4.1: `.dti.bin` format version 2 with the `label` record kind = 2; census warnings take no part in model equality |
| I28 | v4.1: structural errors that cannot be downgraded = every ERR_SYNTAX and ERR_ORDER case, `DefWithoutOperations`, `PortabilityOutsideSubordination`, `LimitExceeded`, `DspLevelSkip`, `EndWithoutAnchor`, `UnclosedAnchor`, `CaseTileWithoutGlyph` |
| I29 | v4.1: under Slot Locking a glyph is missing only at the end of the input; a delimiter in an even slot is a glyph, leaves the period open and is always checked against the DTI as well (double check: `MissingPeriodClose` + `UnknownGlyph { structural: true }`) |
| I30 | v4.1 editor: a comment starts only at the beginning of a token or after a delimiter; suggestions capped at `MAX_SUGGESTIONS`; copy ids generated as `copy<N>` |
| I31 | v4.1: retransmission: already indexed glyphs (enabled keys, numerals) and session symbols that are already keys stay unchanged in the indexed form |
| I32 | v4.1: single even-slot rule: every sequence of visible characters (`~` and `~~` included) is a valid DTI form and the role of the slot (Glyph, Label, Anaphora) is fixed by the annotation (`ingest_with_choices`), with the stage-1 role for unannotated slots; in the editor a single note `SlotRoleMenu` replaces `Label`, `GlyphOrLabel` and `AnaphoraOrForm`. The lexical and expanded forms write the token as it is: re-reading them needs the same annotation, whereas the indexed form has no ambiguity |
| Limits | Ascon and Skein without known test vectors (consistency tests only); the default Argon2id parameters are not exercised in the tests (64 MiB). |
