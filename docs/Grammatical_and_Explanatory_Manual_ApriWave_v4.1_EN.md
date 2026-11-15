# Grammatical and Explanatory Manual — ApriWave · GHLIP Protocol v4.1

**Edition of 15/11/2026, version 4.1.** This manual explains the grammar of the GHLIP protocol and the architecture of the ApriWave software to an external reader, without assuming any knowledge of the working documents. Every rule follows the same pattern:

1. **The problem**: which ambiguity or communication difficulty (between different species, between a person and a machine, between machines) has to be removed;
2. **The solution**: what the protocol chose and why it leaves no exception;
3. **The rule**: the complete, formal statement, with tables;
4. **The example**: one or two realistic cases, explained token by token.

The normative reference remains the Technical Manual (`Technical_Manual_ApriWave_v4.1_EN.md`, and its Italian original); this text is its reasoned explanation. In case of doubt the Technical Manual prevails.

---

## Contents

1. What GHLIP is and what everything is called
2. Writing: one space, tiles and glyphs, Slot Locking
3. The period and its three closes
4. The three kinds of block
5. The signal tile and the determiners
6. The case tiles
7. The quantum number
8. Glyph slots: glyphs, the `§` qualifier, labels, anaphora
9. Connectors and the depth tree
10. Inheritance in subordinate clauses
11. The anaphora `~` and `~~`
12. `def` variables
13. `abdur` loops and the execution plan
14. Structural sets
15. Diagnostics and warnings
16. The DTI dictionary, keys and numerals
17. Labels
18. Stage 3: expanded form, indexed form, views
19. The policy manager
20. The Legacy Shield
21. The Ghost Protocol
22. The assisted editor
23. The contract with Apricot Logic
24. Software architecture
25. Glossary

---

## 1. What GHLIP is and what everything is called

### 1.1 The problem
Two parties that share neither a language nor a culture (two species, a person and a device, two machines from different makers) must exchange sentences that each of them reads **in one way only**. Natural languages are full of ambiguity: the same word order can have several structures, a word can be a verb or a noun, punctuation changes the meaning. Moreover, a system that "understands" the meaning of words carries the biases of whoever built it.

### 1.2 The solution
GHLIP sharply separates **structure** from **content**:
- structure is given by a closed and immutable set of grammatical tools, the **tiles**;
- content is given by the **glyphs**, which the protocol never interprets: they are entries of an external dictionary, the **DTI**.

The grammar is a mechanical, deterministic and neutral engine: it neither knows nor judges the content of glyphs, and it guides the user without imposing arbitrary choices or bans.

### 1.3 The rule: the names

| Level | Project / container | Protocol / program | Role |
|---|---|---|---|
| **Level 0** — structural and relational | **ApriWave** (*Apricot Wave*): the ecosystem, the repository, the Rust crates `apriwave-core` and `apriwave` | **GHLIP** (*Ghlip*) | Deterministic grammar: structure, references, execution plan |
| **Level 1** — computational and inferential | **Apricot Logic** (twin, separate project) | **LEMMA** | Logic engine reasoning on periods already validated by GHLIP |

**GHLIP** stands for *Grammatical Hierarchical Linear Inter-species Protocol*:

| Letter | Meaning | In practice |
|---|---|---|
| G — Grammatical | Closed frame of the universal grammatical tiles | Tiles never change; the vocabulary lives in the DTI |
| H — Hierarchical | Tree hierarchy without closing tags | Depth is written with `:`, `::`, `:::` |
| L — Linear | Deterministic linear scan | A tile, then a glyph, then a tile… (Slot Locking) |
| I — Inter-species | No cultural or anthropocentric assumption | Even the base of numbers is declared (chapter 16) |
| P — Protocol | Pure protocol | Computation belongs to Level 1 |

The term **glyph** (Italian *glifo*, invariable in the plural in Italian) denotes only the single element registered in the DTI. The founding triad is **Tiles + Glyph = Sema**: a *sema* is a complete block of meaning (invariable term).

Three principles run through the whole manual:

| Principle | Statement |
|---|---|
| Golden rule | The protocol governs only the grammar of the tiles. The number of repetitions of a loop is read only from the `abdur=<N>` tile; sets are computed only on the structural relations of the DTI |
| Determinism | Every valid string has exactly one structure |
| Self-contained engine | Loops and sets are resolved internally into a validated **execution plan**; only the physical effect is left to the host device |

### 1.4 Example
`/^ ta vedere nu io au canis ^/`

| Token | Kind | Meaning |
|---|---|---|
| `/^` | tile (open) | Starts a period |
| `ta` | signal tile | Affirmation |
| `vedere` | glyph | The concept "see" (from the DTI) |
| `nu` | case tile | Nominative, Singular |
| `io` | glyph | The concept "I" |
| `au` | case tile | Accusative, Singular |
| `canis` | glyph | The concept "dog" |
| `^/` | tile (close) | Closes the period |

Reading: "I see a dog". The protocol does not know what a dog or sight is: it only knows that `io` is the subject and `canis` the object of the relation `vedere`.

---

## 2. Writing: one space, tiles and glyphs, Slot Locking

### 2.1 The problem
If a sentence may contain multiple spaces, line breaks, symbols with a double role or invisible characters, two readers may split it differently. Worse: an invisible character (for example one that reverses the text direction) can make a different command *appear* than the real one.

### 2.2 The solution
- One separator only: **one ASCII space**.
- **Slot Locking** (`1 Tile -> 1 Glyph`): positions alternate rigidly. Odd positions always hold a tile, even positions always its content. Whether a token is a tile or content depends **only on its position**, never on its appearance.
- Total freedom of glyphs: since the position is enough to recognize a glyph, a glyph may contain **any visible character**, even those that are tiles elsewhere. Only invisible characters are forbidden.
- **Single rule for the role of the even slot:** any sequence of visible characters (`canis`, `roma`, `^/`, `:`, `§`, `~`, `~~`…) can be registered in the DTI as a glyph or in the local DTI as a label. The role of the token in the even slot — **Glyph**, **Label** or **Anaphora** — is fixed by the **annotation of the slot**, which the user chooses from a menu among the roles the DTI allows. A slot without annotation has the role given by its position: Anaphora for the exact tokens `~` and `~~`, Glyph for every other token. There are no special cases: the same mechanism tells a glyph from a label written the same way and an anaphora from a glyph or a label written `~` or `~~`.

### 2.3 The rule

| Element | Rule |
|---|---|
| Separator | Exactly one ASCII space between two tokens. Double, leading or trailing spaces, tabs and line breaks are syntax errors. Free-form writing is allowed only in the assisted editor (chapter 22), which normalizes it |
| Tiles | Closed set; lowercase only; `.`, `|`, `(`, `)` forbidden |
| Glyphs | Any visible character: every script, uppercase, digits, punctuation, symbols (`-`, `_`, `"`, `§`, `:`, `/`, `^`, `~`…) |
| Characters forbidden in glyphs | Only invisible ones: control characters and format or text-direction characters (U+061C, U+200B–U+200F, U+202A–U+202E, U+2060–U+2064, U+2066–U+2069, U+FEFF) |
| Even slots | Hold **any token** of visible characters: also `/^`, `^/`, `/^/`, `^^/`, `:`, `::`, tile names (`a`, `ta`, `y`…), `"`, `§`, `~` and `~~` |
| Role of the token in the even slot | Slot annotated **Glyph** → glyph; slot annotated **Label** → label; unannotated slot → role given by the position: **Anaphora** for the exact tokens `~` and `~~` (chapter 11), **Glyph** for every other token. The rule is the same for every token |

**Why no token creates ambiguity.** The role of every even slot (Glyph, Label or Anaphora) is fixed by the annotation, chosen in the editor from a menu, which accompanies the text at ingestion. On display every role has its own colour; in machine-to-machine transmission the difference is visible in the text itself: a glyph becomes a key of 32 lowercase characters, a label becomes `LVAAAA` followed by its form (for example `LVAAAA^/`, `LVAAAA~`, `LVAAAA~~`), an anaphora travels as `~` or `~~`. The same holds for any token: `^/` annotated Label travels as `LVAAAA^/` exactly as `~~` annotated Label travels as `LVAAAA~~`.

**The missing glyph and the double check.** Since an even slot accepts any token, a glyph is "missing" only at the end of the text. If the user writes `/^ ta vedere nu ^/` forgetting the glyph after `nu`, the token `^/` becomes the glyph of `nu` and the period stays open. **Two independent checks** then apply:

| Check | What it sees | Outcome |
|---|---|---|
| Syntactic | The period has no close | "Unclosed period" error |
| Dictionary | The glyph `^/` is not registered in the DTI | "Unregistered glyph, structural" note: the editor highlights it and asks to register it as a Glyph or as a Label, or to fix the text |

### 2.4 Examples
**Example 1 — a delimiter used as a glyph.** In a technical lexicon the sign `^/` may name a physical connector. `/^ ta collegare nu tecnico au ^/ ^/`:

| Position | Token | Role by position |
|---|---|---|
| 1 | `ta` | signal tile |
| 2 | `collegare` | glyph |
| 3 | `nu` | case tile |
| 4 | `tecnico` | glyph |
| 5 | `au` | case tile |
| 6 | `^/` | **glyph** (even position) |
| 7 | `^/` | close tile (odd position) |

**Example 2 — a glyph in another script.** `/^ ta visitare nu io au 東京 ^/`: `東京` is a valid glyph; the DTI will say which concept it stands for.

---

## 3. The period and its three closes

### 3.1 The problem
A conversation is made of linked sentences: some must remember what was said (to recall it with an anaphora), others must "start from scratch" for safety. An explicit way to say how much memory to keep is needed.

### 3.2 The solution
The period opens and closes with dedicated tiles. The **three closes** decide what survives the period; two **synchronized opens** link the period to the previous one.

### 3.3 The rule

| Open | Meaning | Condition |
|---|---|---|
| `/^` | Start of a period | — |
| `/^≈` | Strict simultaneity with the previous period | A previous period not closed by `^^/` is required |
| `/^≃` | Logical proximity with the previous period | As above |

| Close | Level-0 references (for anaphora) | `def` variables | Synchronization |
|---|---|---|---|
| `/^/` soft | **kept** and accumulated | kept | kept |
| `^/` standard | cleared | kept | kept |
| `^^/` hard | cleared | **cleared** | **cleared** |

Several periods are written one after another, separated by a single space.

### 3.4 Example
`/^ ta x nu a /^/ /^ ta z au ~ ^/`
- The first period affirms `x` with subject `a` and closes with `/^/`: its level-0 blocks stay visible.
- The second period uses `au ~`: the anaphora `~` finds `a` in the previous period.
- With `^/` instead of `/^/`, `~` would find nothing and a reference error would follow.

---

## 4. The three kinds of block

### 4.1 The problem
A period must be able to hold statements, definitions of names to reuse and structural instructions (where to insert a group of clauses). Mixing them without distinction would make reading ambiguous.

### 4.2 The solution
Every block has a form recognizable from its first tile.

### 4.3 The rule

| Block | Form | Chapter |
|---|---|---|
| Relational | `[signal+determiners] [glyph] { [case] [glyph] }` | 5–8 |
| Definition | `[def…] [name] [operation] [value] { [operation] [value] }` | 12 |
| Portability | `anchor` / `anchorrel` / `rel` / `reloff` `<glyph>`, or `end ~`, only after `:` | 9 |

The first block of the period has no connector and cannot be a portability block.

### 4.4 Example
`/^ defu dev a sensore y ta leggere nu dev ^/`: a definition block (`defu dev a sensore`), the connector `y`, a relational block (`ta leggere nu dev`).

---

## 5. The signal tile and the determiners

### 5.1 The problem
Every block must say **what kind of act** it performs (affirm, deny, ask…) and with which nuances (time, quantity, comparison), without extra words that could be mistaken for content.

### 5.2 The solution
All the information about the act lives in **one tile**, the signal tile: a head followed by determiners agglutinated in a **fixed order**. The fixed order makes reading unambiguous.

### 5.3 The rule: the heads

| Head | Meaning |
|---|---|
| `ta` | Affirmation (default) |
| `to` | Negation |
| `ka` | Question |
| `ko` | Negative question |
| `ot` | Possibility |
| `oh` | Vocative; places the block as a sibling of the last active block (chapter 9) |
| `qua`, `que`, `quo`, `quu` | Secondary signals (indirect, hypothetical, imperative, indeterminate), allowed only in subordinate clauses and always followed by a primary head: `quata`, `quotazato` |
| `deep` | Machine-to-machine channel; places the block at level 0 when there are no `:`. Accepts determiners (`deepall`), never a primary head |
| `def`, `defu`, `defm`, `defnum` | Variable definition; accepts only the number |

### 5.4 The rule: the determiners

Fixed order: `[head][quantifier][ref][comparative][temporal | conditional][number]`

| Class | Codes |
|---|---|
| Quantifier | `all` (for all), `some` (there exists) |
| Reflexive | `ref` |
| Comparative | `cha` greater, `chu` smaller, `cla` different, `col` equal, `cal` similar, `cua` included, `cuo` excluded, `cup` united |
| Temporal | `ato` present (default), `alt` remote past, `alv` remote future, `att` recent past, `atv` near future, `asu` indeterminate time |
| Conditional | `z` + temporal: `zato`, `zalt`, `zalv`, `zatt`, `zatv`, `zasu` |
| Number | `u`, `m`, `num`, always last (chapter 7) |

- A determiner out of order or repeated is an order error; temporal and conditional share the same position and cannot appear together.
- **Reflexive `ref`**: the action falls back on the subject. The direct object is not mandatory (direct reflexive) but fully allowed (indirect or transitive reflexive).
- **`ref` and comparatives** combine freely: `tarefcha` is valid.

### 5.5 Examples
**Example 1.** `tachazatou` splits into `ta` (affirmation) + `cha` (greater) + `zato` (present conditional) + `u` (Singular): "if it is greater…", with the signal pair in the Singular.

**Example 2 — transitive reflexive.** `/^ taref aggiornare nu nodo a firmware ^/`: "the node updates its firmware". `ref` says the action falls back on the node; the Accusative `a firmware` gives its object.

---

## 6. The case tiles

### 6.1 The problem
In a natural language the role of a word (who acts, on what, for whom, with what) depends on word order or on ambiguous prepositions. An explicit and closed way to declare the role of each glyph is needed.

### 6.2 The solution
Every glyph of a relational block is preceded by a **case tile** that declares its role. The tiles form a closed list. The same case may repeat, and the written order is kept.

### 6.3 The rule

Form: `[case][determiner][number]` and, only for the loop, `=<N>`.

| Tile | Case | Use |
|---|---|---|
| `n` | Nominative | subject |
| `a` | Accusative | object; object of a comparison |
| `g` / `gpa` | Genitive / genitive partitive | belonging |
| `d` / `dad` | Dative / allative | recipient / motion towards |
| `ab` | Generic ablative | origin, means |
| `abef` | Elative | motion out of |
| `aben` | Inessive | location inside |
| `abel` | Illative | motion into |
| `abse` | Abessive | "without" |
| `abfi` | Translative | change into |
| `abcu` | Instructive | by means of |
| `absi` | Comitative | "together with" |
| `abdu` | Continuous temporal | "while, during" (never a loop) |
| `abpa` | Partitive | part of |
| `abdur=<N>` | Loop operator | repeat N times (`N` from 1 upwards, no leading zeros) |

- **Number** at the end of the tile: omitted (Dual), `u` Singular, `m` Plural, `num` Non-Dual (`nu`, `am`, `abenu`, `abduru=5`).
- **Repeated cases**: the same case may appear several times in the same block.
- **Stable canonical order**: internally the pairs are ordered as signal → `n` → `a` → `d` → `g` → `ab`; for equal cases **the order written by the user** is kept.

### 6.4 Example
`/^ ta inviare nu centrale du nodo1 du nodo2 au avviso ^/`

| Pair | Case | Canonical order |
|---|---|---|
| `nu centrale` | Nominative | 1st after the signal |
| `du nodo1` | Dative | 3rd |
| `du nodo2` | Dative | 4th (after `nodo1`, as written) |
| `au avviso` | Accusative | 2nd |

---

## 7. The quantum number

### 7.1 The problem
Many systems take for granted that a quantity is "one" or "many". But an observer may know that a quantity *exists* without being able to classify it yet, or may know nothing.

### 7.2 The solution
Four states, inspired by superposition: the default state (no marker) is indeterminacy; the markers make it "collapse".

### 7.3 The rule

| State | Marker | Meaning |
|---|---|---|
| Dual | omitted | Superposition: undetermined before observation |
| Singular | `u` | Collapse onto unity |
| Plural | `m` | Collapse onto multiplicity |
| Non-Dual | `num` | Quantity detected but not yet classified |

Every pair has an **effective number**:

| Pair | Number written | Number omitted |
|---|---|---|
| Signal pair | prevails | In a subordinate clause it inherits the number of the parent signal; otherwise Dual |
| Case pair | prevails | Dual; before `~` it inherits the number of the recalled pair; before a `def` variable it takes the variable's number |

### 7.4 Example
`/^ tau misurare nu sonda : ta registrare au ~ ^/`: the subordinate `ta registrare` writes no number and **inherits** the Singular of its parent `tau`. The pair `au ~` writes the Singular explicitly.

---

## 8. Glyph slots: glyphs, the `§` qualifier, labels, anaphora

### 8.1 The problem
A concept may need qualifications ("video, red"), but a qualification must never be mistaken for a new concept and, conversely, a compound name already registered must not be split.

### 8.2 The solution
The sign `§` joins distinct DTI glyphs; but if the whole token (even containing `§`) is registered as one glyph, **the dictionary has priority**.

### 8.3 The rule

| Form in the even slot | Example | Meaning |
|---|---|---|
| Glyph | `canis`, `Roma`, `x-y`, `30` | Any token of visible characters |
| Qualified glyph | `video§rosso` | If `video§rosso` is not registered as one glyph: `video` qualified by `rosso` (every joined concept must allow it, chapter 16) |
| One glyph with `§` | `rosso§vino` registered | One concept, one key |
| Label | `Roma` annotated as Label | Inert string registered in the local DTI (chapter 17) |
| Glyph anaphora | `~` not annotated | Recalls the last glyph of the antecedent block (chapter 11) |
| Sema anaphora | `~~` not annotated | Recalls the whole antecedent block |

An empty segment (`x§`, `§x`) is accepted by the syntax but, resolved by segments, it is an unknown form. Old forms such as `video~` or `vedere~~` are not anaphora: they are ordinary glyphs.

### 8.4 Example
With a DTI holding `rosso§vino` as one concept, `/^ ta bere nu io au rosso§vino ^/` transmits **one** key; with a DTI holding only `video` and `rosso`, `/^ ta mostrare au video§rosso ^/` transmits two keys joined by `§`.

---

## 9. Connectors and the depth tree

### 9.1 The problem
Subordinate clauses (conditions, consequences, details) form a tree. Languages with opening and closing markers (like brackets) force the reader to remember distant closes and break easily.

### 9.2 The solution
Depth is written **once, at the start of the block**, with a run of `:`. A block at depth T is a child of the last active block at depth T−1. No close is needed: the stack "prunes" itself. Conjunctions link siblings in written order.

### 9.3 The rule

| Connector | Depth of the following block |
|---|---|
| `:` repeated N times | N |
| `y`, `vel`, `aut` without `:` | the depth of the last active block |
| `sed` without `:` | level 0 |
| conjunction + `:`×N | N |

- Going from depth D to a depth greater than D+1 is a scope error. The maximum depth is configurable (64 by default).
- **Forced roles**: `oh` without `:` is a sibling of the last active block; `deep` without `:` goes to level 0; `def` is transparent (it does not enter the tree). With explicit `:` the `:` always win.
- **Coordination**: siblings keep the written order; every block carries its own conjunction (`y` "and", `vel` "or", `aut` "exclusive or", `sed` "but"). The protocol presumes no logical grouping: the logical reading belongs to Apricot Logic.
- **Anchors**: they insert a group of blocks at a reference depth.

| Portability tile | Effect |
|---|---|
| `: anchor <id>` | Registers an anchor at the given depth; absolute mode |
| `: anchorrel <id>` | Like `anchor`, relative mode: a block with C `:` goes to depth K + (C − 1), where K is the anchor depth |
| `: rel <id>` | Reactivates the relative mode on an open anchor |
| `: reloff <id>` | Deactivates the relative mode |
| `: end ~` | Closes the last anchor and returns to its depth |

Anchors are valid only within the period: an unclosed anchor is an error.

### 9.4 Examples
**Example 1.** `/^ ta vedere nu io au canis : ta abbaiare nu ~ :: tazato fame nu ~ : ta osservare nu gatto ^/`

| Block | `:` | Depth | Parent |
|---|---|---|---|
| `ta vedere…` | — | 0 | — |
| `ta abbaiare…` | `:` | 1 | vedere |
| `tazato fame…` | `::` | 2 | abbaiare |
| `ta osservare…` | `:` | 1 | vedere (the stack prunes itself) |

**Example 2 — conjunctions.** `/^ ta p : ta a y : ta b sed : ta c ^/`: the children of `p` are `a`, `y b`, `sed c`, exactly in this order.

---

## 10. Inheritance in subordinate clauses

### 10.1 The problem
Repeating in every subordinate clause the tense, the quantifier or the comparison of the parent makes messages longer and multiplies errors.

### 10.2 The solution
A subordinate clause inherits everything it does not write; what it writes always prevails.

### 10.3 The rule

| What is inherited | Conditions |
|---|---|
| Tense or conditional, number of the signal, quantifier, `ref`, comparative | Only if the subordinate has a primary or secondary head and does not write that determiner. Inheritance is transitive (the effective value of the parent is inherited) |
| Nothing | For `deep` blocks (except the determiners written on `deep`) |

Checks that remain:

| Check | Outcome |
|---|---|
| A **written** comparative without any Accusative in the block | Semantic error (an inherited comparative imposes nothing) |
| Secondary signal at level 0 | Scope error |

### 10.4 Example
`/^ taallalt x nu y : ta z nu w ^/`: the subordinate `ta z` inherits `all` (for all) and `alt` (remote past) from its parent.

---

## 11. The anaphora `~` and `~~`

### 11.1 The problem
Repeating a glyph already said is long and, with homonyms, may raise doubts about *which* glyph was meant. A short reference with a unique search rule is needed.

### 11.2 The solution
Only two operators, with a fixed search order: `~` recalls a glyph, `~~` recalls a whole sema.

### 11.3 The rule

**Search order of the antecedent**: 1) the parent; 2) the ancestors up to the root; 3) the preceding level-0 blocks, most recent first; 4) the level-0 blocks kept by `/^/`. Pruned branches are invisible; `~` does not see its own block.

| Operator | What it recalls |
|---|---|
| `~` | The last glyph of the antecedent in canonical order (signal → `n` → `a` → `d` → `g` → `ab`, written order for equal cases), skipping pairs that hold a `~~` |
| `~~` | The whole antecedent sema, in the same way in any even slot |

- Chains of `~` are followed down to the final value.
- Number: when omitted, the case pair with `~` inherits the number of the recalled pair.
- Without an antecedent: reference error.
- The Anaphora role belongs only to unannotated slots written `~` or `~~`: a slot annotated Glyph or Label, whatever its token, searches no antecedent (chapter 2).

### 11.4 Example
`/^ ta vedere nu io au humanum : tazato venire n ~ ^/`

| Step | Explanation |
|---|---|
| `~` | In the subordinate clause, the antecedent is the parent `ta vedere…` |
| Canonical order of the parent | signal `vedere`, `nu io`, `au humanum` |
| Last glyph | `humanum` |
| Number | `n ~` inherits the Singular of `au humanum` |

Reading: "I see a man; if the man comes…".

---

## 12. `def` variables

### 12.1 The problem
A set of values (for example a chosen sensor) may be needed in many blocks; rewriting it is long and risky.

### 12.2 The solution
The `def` block gives a name to one or more values; while the variable is live, every glyph equal to the name is a reference to it.

### 12.3 The rule

| Operation | Tile | Effect |
|---|---|---|
| Assignment | `a`, `au`, `am`, `anum` | Creates the variable (error if already live) |
| Modification | `abfi` | Replaces the value |
| Union | `absi` | Adds a value |
| Deletion | `abse ~` | Removes the variable (`~` is a marker here) |

- `def`, `defu`, `defm`, `defnum` set the number of the variable.
- The name is the whole token; a `def` must always be followed by a connector (`y` or `:`).
- Using a non-existent variable is a reference error; `^^/` clears every variable.

### 12.4 Example
`/^ defu dev a sensore§ottico y defu dev abfi sensore§termico y ta leggere nu dev y defu dev abse ~ ^/`

| Block | Effect |
|---|---|
| `defu dev a sensore§ottico` | `dev` is `sensore§ottico` |
| `defu dev abfi sensore§termico` | `dev` is now `sensore§termico` |
| `ta leggere nu dev` | the thermal sensor is read |
| `defu dev abse ~` | `dev` no longer exists |

---

## 13. `abdur` loops and the execution plan

### 13.1 The problem
A device must repeat an action an exact number of times. If the number were deduced from the meaning of a word ("three", "often"), every machine could interpret it differently.

### 13.2 The solution
The number of repetitions lives **only** in the `abdur=<N>` tile. The engine produces an **execution plan**: the ordered list of the steps the device must perform, each with the indication of the round it belongs to.

### 13.3 The rule
- `abdur=<N>` (also `abduru=<N>`, `abdurm=<N>`, `abdurnum=<N>`) repeats **the clause and its whole subtree** N times. The following glyph (usually `~`) is never read for counting.
- Several `abdur=<N>` in the **same** clause form loops **nested in written order**: the first written is the outer round.
- `abdur` without `=<N>` is a **control marker**: allowed in a subordinate clause whose signal has a written or inherited conditional; the protocol checks its form, evaluating the condition belongs to Apricot Logic. In the plan, control subordinates appear once, before the repeated body.
- A configurable limit (1000 by default) applies both to N and to the number of generated steps.

**What a step is.** Every step of the plan holds: the clause to execute, the **iteration index** (one number per enclosing loop, from the outer round to the inner one, counting from 0) and the possible targets (chapter 14).

### 13.4 Examples
**Example 1 — a loop inside a loop.** `/^ ta scansione abdur=2 ~ : ta misura abdur=2 ~ ^/`: "perform the scan 2 times; at every scan perform the measurement 2 times".

| Step | Clause | Index `[outer round, inner round]` | Meaning |
|---|---|---|---|
| 1 | scansione | `[0]` | first scan |
| 2 | misura | `[0, 0]` | first measurement of the first scan |
| 3 | misura | `[0, 1]` | second measurement of the first scan |
| 4 | scansione | `[1]` | second scan |
| 5 | misura | `[1, 0]` | first measurement of the second scan |
| 6 | misura | `[1, 1]` | second measurement of the second scan |

**Example 2 — two loops on the same clause.** `/^ ta scansione abdur=2 ~ abdur=3 ~ ^/`: "repeat the scan 2 × 3 = 6 times, as 2 outer rounds of 3 inner rounds".

| Step | Index `[outer round, inner round]` |
|---|---|
| 1 | `[0, 0]` |
| 2 | `[0, 1]` |
| 3 | `[0, 2]` |
| 4 | `[1, 0]` |
| 5 | `[1, 1]` |
| 6 | `[1, 2]` |

---

## 14. Structural sets

### 14.1 The problem
"Activate all the sensors except the optical ones" requires knowing which concepts are sensors. An engine that deduced it from the meaning of words would be arbitrary.

### 14.2 The solution
Sets are computed **only** on the relations declared in the DTI (every concept may name its more general concept with `base:`) and on the values of `def` variables.

### 14.3 The rule
A clause has targets only if it has a quantifier (`all`, `some`) or a `cup`, `cua`, `cuo` comparative.

| Element | Operation |
|---|---|
| Accusative pairs | Base set |
| Nominative pairs, if the block has no Accusative | Base set |
| `all` / `some` | Extend base and unions to the enabled descendants |
| `cup` / `absi` | Union |
| `cua` / `abpa` | Intersection (with the operand and its descendants) |
| `cuo` / `abse` | Subtraction (of the operand and its descendants) |

Order: union → intersection → subtraction; the result is sorted by key, without duplicates. With `all` or no quantifier the mode is "all"; with `some` it is "at least one". A compound operand `K1§K2` denotes the enabled DTI record with `base:K1` and `param:K2`.

### 14.4 Example
`/^≈ deepall attivare nu sistema am sensore abse sensore§ottico ^/` (written after a previous period, because `/^≈` requires an antecedent):

| Step | Set |
|---|---|
| Base | `sensore` and all its enabled descendants (because of `all`) |
| Subtraction | `sensore§ottico` and its descendants |
| Result | the other sensors, for example the thermal ones, in key order |

---

## 15. Diagnostics and warnings

### 15.1 The problem
An error must be recognized identically by every implementation, and it must not leave the system in a half-done state.

### 15.2 The solution
Six fixed error codes, **atomic** ingestion (all or nothing) and the possibility, at the user's own risk, of turning into warnings the codes that do not compromise the structure.

### 15.3 The rule

| Code | Examples |
|---|---|
| `ERR_SYNTAX` | Invalid token or structure; non-canonical spaces; unclosed period; invisible characters |
| `ERR_ORDER` | Determiners out of order or repeated |
| `ERR_REFERENCE` | Anaphora without antecedent; unknown anchor; non-existent variable |
| `ERR_SEMANTIC` | Written comparative without Accusative; `def` without operations; number conflict |
| `ERR_SCOPE` | Level skip; synchronization without antecedent; unclosed anchors; exceeded limits; secondary signal at level 0; `abdur` marker without conditional |
| `ERR_CASE_AMBIGUITY` | Case tile without glyph at the end of the text |

- With any error the session stays exactly as before; all errors are returned, sorted by position.
- **Downgrade to warning**: by default every code is blocking. The user may declare single codes as warnings (in the configuration or in the `settings.toml` file). **Structural** errors, which prevent building a coherent tree, stay blocking anyway (every syntax and order error, limits, level skips, unbalanced anchors, case tile without glyph). Warnings are read from the accepted period.

### 15.4 Example
`/^ quata x ^/` (secondary signal at level 0) is an `ERR_SCOPE`. If the user declares `ERR_SCOPE = "warning"`, the period is accepted and the warning stays available; `/^ Ta x ^/` (uppercase in a tile) stays an error, because it is structural.

---

## 16. The DTI dictionary, keys and numerals

### 16.1 The problem
The same idea is said `porta` in Italian, `uscio` in a dialect, `door` in English. If machines exchanged words they would have to share a language; if they exchanged sequence numbers, two different dictionaries would clash.

### 16.2 The solution
Every **concept** has a universal, immutable and meaningless **key**; the words of each language are only forms pointing to the key. The dictionary has **two levels**: a read-only universal snapshot and a local overlay with the user's choices.

### 16.3 The rule: the key

| Property | Value |
|---|---|
| Length | 32 characters |
| Alphabet | `[a-z0-9]` |
| First character | always a letter `[a-z]` |
| Characters 2–31 | 30 random characters `[a-z0-9]` |
| Last character | **Luhn mod 36** check character, which reveals any single-character substitution |
| Identity | one key per concept, never modified, reused or deleted (deprecated with `superseded_by:`) |

**Why the first letter.** A token starting with a letter is a key; one starting with a digit is a number. The distinction is made by looking at one character, without ever confusing a key with a number, not even with a 32-digit one.

### 16.4 The rule: the two levels and the `.awdti` format

| Level | Content |
|---|---|
| Universal snapshot | Concepts, forms of the exchange language, `base:`/`param:` relations, morphological permissions, numeral system. Replaceable as a whole |
| Local overlay | Enabled concepts (`use`), local forms, local concepts (`new`), labels (`label`), policy for unknown keys |

```text
#awdti 1
#role universal
#version 42
#lang en
#numeral base:10 digits:<K0>,…,<K9>
rec <KEY> <form> [display:"…"] [syn:a,b] [base:<KEY>] [param:<KEY>] [morph:+AGGL,+CONTR,+UNION,+AGGL_NUM] [superseded_by:<KEY>] [attr:k§v]
```
```text
#awdti 1
#role local
#universal 42
#lang it
#policy degrade|unknown|reject
use <KEY> [form:<form>] [display:"…"] [syn:…]
new <KEY> <form> [base:<KEY>] [display:"…"] [syn:…] [morph:…]
label <KEY> <form>
```

- A form is any sequence of visible characters without spaces, `§`, `~` and `~~` included.
- The primary form is unique per language; synonyms (`syn:`) and readable forms with spaces (`display:`) serve the editor.
- Loading checks duplicate keys, missing references, cycles in `base:` chains, the ban on reusing a universal key for a local concept, and the snapshot version.
- A deterministic binary form (`.dti.bin`) exists, validated with the same rules as the text.

### 16.5 The rule: the "mind of a child" — numerals and morphological permissions

**The problem.** A human system takes base 10 and the freedom to compose words for granted. A different intelligence may count in base 2 or 8, and may not accept that two concepts merge into one.

**The solution.** The protocol behaves like the mind of a child: **it knows nothing until it is taught**. The base of numbers and every way of composing concepts must be declared in the DTI.

| Declaration | Meaning |
|---|---|
| `#numeral base:<B> digits:<K0>,…,<K(B−1)>` | The base B (from 2 to 10) and, for every digit from 0 to B−1, the concept of the digit. In transmission the digits use the ASCII characters `0`–`9` |
| Digit ≥ base | The numeral is invalid (`InvalidNumeral`) |

| Morphological permission | When it is needed | Who must carry it |
|---|---|---|
| `+UNION` | Union of concepts with `§` resolved by segments (`video§rosso`) | Every joined concept (numerals are exempt: in `30§celsius` only `celsius` needs it) |
| `+AGGL` | Compound form by agglutination: the form is the concatenation of the forms of its two components (`cagn` + `one` → `cagnone`) | Both components (`base:` and `param:`) |
| `+CONTR` | Compound form by contraction: a form different from the concatenation (`cucciolo` = dog + small) | Both components |
| `+AGGL_NUM` | Every digit of a numeral | The concept of the digit |

Compound forms must always be registered as records: the engine never splits an unknown word on its own.

### 16.6 Examples
**Example 1 — one key, three words.** The snapshot holds `rec door000…i door`; the Italian overlay holds `use door000…i form:porta syn:uscio`. Whether the user writes `porta` or `uscio` (the editor normalizes `uscio` into `porta`), the same 32-character key always travels on the network.

**Example 2 — three bases.**

| Declaration | Numeral | Outcome |
|---|---|---|
| `#numeral base:2 digits:<K0>,<K1>` | `101` | valid (digits 0 and 1) |
| `#numeral base:8 digits:<K0>,…,<K7>` | `17` | valid |
| `#numeral base:8 …` | `19` | invalid: digit 9 does not exist in base 8 |
| `#numeral base:10 digits:<K0>,…,<K9>` | `42` | valid if every digit carries `+AGGL_NUM` |

---

## 17. Labels

### 17.1 The problem
It is often necessary to transmit a name that is not a concept of the dictionary: a batch code, an acronym, a proper name. The old quotes (`"…"`) created an exception in the grammar and did not let the receiver know whether that string was expected.

### 17.2 The solution
Quotes no longer have any grammatical value (`"` is a character like any other). A **label** is an entry of the **local** DTI with the *Label* type. The slot holding it is **annotated** as a Label, and on transmission the label carries a reserved prefix.

### 17.3 The rule

| Aspect | Rule |
|---|---|
| Registration | `label <KEY> <form>` in the local DTI; never in the universal snapshot |
| Coexistence | The same form may be both a glyph and a label in the local DTI |
| Census warning | If the form of a local glyph or of a label is already a universal glyph in the exchange language, loading produces a warning (never an error), so that the user knows the glyph exists |
| Mandatory selection | To write a form registered as a label the user chooses **Glyph** or **Label** from the menu; without an annotation (programmatic flows) a slot is a glyph |
| Content | A label is an inert string: the protocol does not interpret it and the editor does not normalize it. Two identical labels denoting different real-world objects are not a problem of the protocol: the context belongs to the writer and the reader |
| Transmission | `LVA<SUBTYPE><form>`: `LVA` identifies the label, the subtype is 3 uppercase letters (`AAA` = base label), then the form: `LVAAAAroma` |
| Reception | A token starting with `LVA` is always a label, even when absent from the receiver's DTI; if the form is also a glyph of the receiver's DTI, the homonymy is highlighted |
| New subtypes | A subtype other than `AAA` was generated by the sending machine: the receiver accepts it, registers it in its subtype table with an informative note and keeps it when retransmitting. Only a truncated or corrupted token is an error (fewer than 7 characters, or a subtype that is not 3 uppercase letters) |

> [!IMPORTANT]
> **Execution in a safe environment.** A label not registered in the receiver's local DTI, an external label admitted by the policy, or a label with a dynamic subtype not yet classified must be handled and executed **only inside an isolated and passive environment** (sandbox), with no privilege of direct execution on system commands or physical actuators.

### 17.4 Examples
**Example 1.** The local DTI holds `use <K_verde> form:verde` and `label <K> verde`. The user writes `ta dipingere au verde`: the editor opens the menu; if the user chooses Label, on transmission `verde` becomes `LVAAAAverde`; if the user chooses Glyph, it becomes the key of the concept "green".

**Example 2.** A machine receives `LVAXYZlotto42`: it is a label of subtype `XYZ`, unknown; the message is accepted, `XYZ` is registered, and the label must be handled in a sandbox.

---

## 18. Stage 3: expanded form, indexed form, views

### 18.1 The problem
The same period must be read by a person (who wants to see the resolved values), transmitted between machines (without depending on languages) and shown in several formats.

### 18.2 The solution
The engine works in three stages: **stage 1** syntax, **stage 2** analysis (tree, anaphora, variables, inheritance), **stage 3** resolution in the DTI and renderings.

### 18.3 The rule

| Rendering | What it contains | What it is for |
|---|---|---|
| Expanded form | No `def` or anchors; explicit `:`; variables and `~` replaced by their values (with the effective number in case pairs); `~~` stays `~~` | Human reading of the resolved values |
| Indexed form | The **written** form, with every glyph replaced by its key and every label by `LVA…`; `def` blocks, anchors, anaphoric `~` and `~~`, tiles and numerals stay | **The only exchange form between machines** |
| Views | Written structure with readable forms (`HumanOriginal`), with primary forms (`CanonicalLexical`) or indexed (`DtiIndexed`); on one line or one clause per line | Consultation |

- Resolution: the whole token is looked up first among the primary forms, then by `§` segments; a concept that is not enabled suggests `use <KEY>`.
- Variable names and anchor identifiers become the DTI key when they are enabled primary forms, otherwise an **ephemeral key** derived deterministically from the name.
- The receiver checks every key with its own policy (enabled, degraded to the more general concept, unknown, rejected).
- A period received in indexed form and retransmitted stays identical.

### 18.4 Example
`/^ ta vedere nu io au canis : ta abbaiare nu ~ ^/`
- Expanded form: `/^ ta vedere nu io au canis : ta abbaiare nu canis ^/` (the anaphora is replaced).
- Indexed form: `/^ ta <K_vedere> nu <K_io> au <K_canis> : ta <K_abbaiare> nu ~ ^/`, where every `<K_…>` is a 32-character key; `~` stays, because the receiver resolves it in the same way.

---

## 19. The policy manager

### 19.1 The problem
A device must not execute everything that is grammatically valid: some commands or roles must be forbidden, some thresholds respected.

### 19.2 The solution
A rule file written **on keys** (hence language-independent) checks every period after stage 3, with lookups of guaranteed and deterministic cost.

### 19.3 The rule
```text
#awpolicy 1
#universal <snapshot version>
#default allow|deny
signal <KEY> ta,ka,deep
role <KEY> n:<K>,<K> a:<K> d:… g:… ab:…
threshold <KEY> cha|chu|col min:<number> max:<number>
qualifier <KEY> <K>,<K>
require <KEY> zato|quo
label deny | label allow:<K>,<K>
```

| Filter | Rule |
|---|---|
| Allowed heads | Which signal heads may be used with that glyph |
| Role matrix | Which keys are allowed for every base case |
| Thresholds and qualifiers | Numeral of the Accusative within [min, max]; allowed `§` qualifiers |
| Mandatory subordinate | A child with a written conditional or with `quo` (confirmation) |
| Enhanced security | Labels forbidden, or allowed only for the listed signal glyphs |

With `#default deny` the policy is an allow-list. Rules apply to the signal glyph; values reached through variables and anaphora are checked already resolved; a sema recalled with `~~` counts as the glyph of its signal. Slots annotated as Label and `LVA…` tokens count as labels.

### 19.4 Example
With `signal <K_aprire> ta` and `#default deny`, `/^ ka aprire nu operatore au valvola ^/` (a question) violates the allowed heads: for `aprire` only the affirmation `ta` is permitted.

---

## 20. The Legacy Shield

### 20.1 The problem
An old device does not know GHLIP and cannot defend itself against malformed commands or commands built to exploit reading differences.

### 20.2 The solution
A shield placed in front of the device applies stages 1, 2, 3 and the policy, with enhanced security always active, and forwards **only the reconstructed canonical form**, never the original text.

### 20.3 The rule
- No network access: it is a pure filter.
- A blocked input leaves no trace in the shield session.
- Updating the protection means replacing the DTI and policy files.

### 20.4 Example
An input carrying a label `LVAAAArm` meant for a command is blocked by enhanced security; the device never receives it.

---

## 21. The Ghost Protocol

### 21.1 The problem
A message encrypted in the traditional way is recognizable as such and does not cross nodes that expect GHLIP.

### 21.2 The solution
The Ghost Protocol replaces every key and every digit of the indexed form with another of the same class: the result is **still a valid GHLIP period**, which stage 2 reads with the same structure. Decryption is exact.

### 21.3 The rule

| Element | Rule |
|---|---|
| Algorithms | Internal SHAKE256 without dependencies; optional Ascon, Skein-512, BLAKE3; key derivation with Argon2id |
| What changes | Snapshot keys (among themselves), digits of numerals (same base and length), ephemeral keys (among themselves); optionally some grammatical tiles within safe classes |
| What stays | `~`, `~~`, `abse ~`, `end ~`, the `def` tiles, the connectors |
| Consistency | Equal atoms visible in the same context receive the same ciphertext, so variables and anaphora resolve identically |
| Integrity | Envelope with suite, version, sequence number, nonce and authentication code; constant-time comparison |
| Ordering | Only the next message is accepted: replays and out-of-order messages are rejected |
| Labels | Rejected, so that no plaintext is exposed |

### 21.4 Example
`/^ ta <K1> au <K2> ^/` becomes, for example, `/^ ta <K7> au <K4> ^/`: a valid period, indistinguishable from a normal message, that only the holder of the key brings back to the original text.

---

## 22. The assisted editor

### 22.1 The problem
Writing canonical text (one space, determiners in order, primary forms) is tiring for a person.

### 22.2 The solution
A pre-processor accepts free writing and turns it into canonical text, reporting what is needed without deciding in the user's place.

### 22.3 The rule

| Function | Behaviour |
|---|---|
| Comments | `//` at the start of a token or after a period delimiter |
| Free spacing | Tabs and line breaks become a single space |
| Braces `{ }` | Open the children of the clause just written and generate the `:` |
| Signal train | `ta_zato_cha` becomes `tachazato` (automatic reordering) |
| Aliases | `>` `<` `=` `!=` become `cha` `chu` `col` `cla` |
| Multiword phrases | "verde smeraldo" becomes the primary form of the concept |
| Normalization | Synonyms and readable forms become primary forms |
| DTI check | Every unregistered glyph (also a delimiter placed in an even slot) is reported, with the proposal to register it as a Glyph or a Label |
| Annotations | One rule for every token: when an unannotated token has more than one possible role (the role given by its position plus the roles registered in the DTI), the menu shows the allowed roles among **Anaphora**, **Glyph** and **Label**; the choices accompany the text at ingestion |
| Suggestions | Menu of the recent glyphs and semas starting with the typed letters; automatic or with `Alt+Down` |
| Copy with anchors | With a selection: `Ctrl+C` copies the subordinate clauses as an `anchor` block (absolute depths), `Alt+C` as an `anchorrel` block (relative depths). Without a selection `Alt+C` opens the comparative insertion |

```toml
[editor]
autocomplete = "auto"     # or "manual"

[shortcuts.actions]
suggest = "Alt+Down"
copy_absolute = "Ctrl+C"
copy_relative = "Alt+C"

[warnings]
ERR_SCOPE = "warning"     # at your own risk
```

### 22.4 Example
The user writes on two lines:
```text
/^ ta_zato_> grandis n canis   // comment
   a 30 ^/
```
The editor produces `/^ tachazato grandis n canis a 30 ^/`: the signal train is reordered, the alias `>` becomes `cha`, the comment and the extra spaces disappear.

---

## 23. The contract with Apricot Logic

### 23.1 The problem
Level 1 must be able to compute (for example to add values) without introducing arithmetic functions or exceptions into the grammar.

### 23.2 The solution
A transactional hook: an external function receives a clause and returns **GHLIP text**, which is re-ingested atomically with all the checks.

### 23.3 The rule
- The `FunctionEvaluator` interface and the `Session::evaluate_and_ingest` method.
- Multiple operands are written with `absi`, never with `§`.
- The protocol contains no arithmetic functions; determinism and termination are obligations of the external function.

### 23.4 Example
`/^ ka somma nu sistema a 10 absi 20 ^/`: the function reads the operands `10` and `20` and answers with a period such as `/^ ta somma nu sistema a 30 ^/`, which enters the session only if valid.

---

## 24. Software architecture

### 24.1 The problem
The same protocol must run on a server and on a microcontroller with a few tens of kilobytes of memory, without unpredictable behaviour.

### 24.2 The solution
Two Rust crates: a core without operating system and without dependencies, and a host crate with files, editor, export and command line.

### 24.3 The rule

| Crate | Environment | Contents |
|---|---|---|
| `apriwave-core` | `no_std` + allocator, zero dependencies; builds for Cortex-M0+ and Cortex-M4F | Lexer, parser, analysis, renderings, DTI and binary format on buffers, plan, policy, Legacy Shield |
| `apriwave` | standard library | The whole core, plus the DTI file cache, editor, JSON export, command line, Ghost Protocol |

- No `unsafe` code, no abrupt stop: every error is a structured result.
- Ordered maps: deterministic iteration and lookups with guaranteed cost.
- Memory: every text is stored once; a session closed with `^^/` can be compacted.
- Command line: `apriwave-cli [--ast] [--json] [--warn=<CODE>]… "<period>"`.

### 24.4 Example
A sensor with a Cortex-M0+ includes only `apriwave-core` and an allocator: it receives an indexed period, validates it, builds the execution plan and drives the actuator following the steps of the plan.

---

## 25. Glossary

| Term | Definition |
|---|---|
| Tile | Grammatical tool of the closed set (signal, case, connector, delimiter…) |
| Glyph | Entry of the DTI dictionary; content never interpreted by the protocol |
| Sema | Complete block of meaning: tiles + glyphs |
| Slot Locking | Rigid alternation: odd position = tile, even position = glyph |
| DTI | Two-level dictionary: universal snapshot and local overlay |
| Key | 32-character identifier of a concept |
| Label | Inert string registered in the local DTI, transmitted as `LVA<SUBTYPE><form>` |
| Slot annotation | Anaphora / Glyph / Label choice for an even-slot token |
| Anaphora | Reference to a previous glyph (`~`) or sema (`~~`) |
| Depth stack | Structure that places every block in the tree according to its `:` |
| Execution plan | Ordered list of the steps to execute, with iteration indices and targets |
| Indexed form | Exchange form between machines: written structure with keys and `LVA…` labels |
| Policy | Security rules written on keys |
| Legacy Shield | Filter placed in front of a non-GHLIP device |
| Ghost Protocol | Encryption producing valid GHLIP periods |
| Apricot Logic / LEMMA | Level-1 project and logic engine |
