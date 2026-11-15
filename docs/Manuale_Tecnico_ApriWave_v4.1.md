# Manuale Tecnico — ApriWave · Protocollo GHLIP v4.1

**Edizione consolidata del 15/11/2026 — versione 4.1.** Riferimento normativo unico per la grammatica, il motore e i moduli del workspace ApriWave (crate `apriwave-core` e `apriwave`, versione `4.1.0`). Integra il Manuale v4.0, il Quadro delle Evolutive e le decisioni Q1–Q28, D1–D14, R1–R14, H1–H8, B1–B12, L1–L5, S1–S3, G1–G7, P1–P5 e V1–V10.

La v4.1 elimina le restrizioni arbitrarie introdotte nelle iterazioni precedenti: glifo con qualsiasi carattere visibile e Slot Locking puro; `§` con priorità al DTI; etichette censite nel DTI locale al posto delle virgolette; `ref` compatibile con l'Accusativo e con i comparativi; casi ripetuti con ordine canonico stabile; anafore ridotte a `~` e `~~`; coordinazione nell'ordine scritto; ereditarietà completa nelle subordinate; cicli `abdur` annidati e con condizionale ereditato; declassamento dei singoli codici d'errore a warning. Non contiene elementi eliminati, come `§ack§req`/`§ack§opt` (B1), `sys§`, le etichette tra virgolette, `base~`, `base§mod~`, `glifo~~` o l'albero di coordinazione.

---

## 1. Principi

1. **Telaio relazionale neutro.** GHLIP non è una lingua naturale. Le *tile* sono un insieme chiuso e fisso di strumenti grammaticali; i *glifo* sono il contenuto e appartengono al DTI.
2. **Regola aurea.** Il protocollo governa **esclusivamente la grammatica delle tile** e non interpreta mai il significato dei glifo del DTI. Il conteggio dei cicli si legge solo dalla tile `abdur=<N>`; gli insiemi si calcolano solo sulle relazioni strutturali `base:` delle chiavi.
3. **Determinismo assoluto.** A ogni stringa valida corrisponde una sola struttura.
4. **Motore auto-contenuto (aggiornamento di M§1.1).** Il motore non delega a moduli esterni la risoluzione dei cicli `abdur=<N>` né delle operazioni insiemistiche (`all`, `some`, `cup`, `absi`, `cua`, `abpa`, `cuo`, `abse`). Le risolve internamente e produce un **piano di esecuzione** deterministico e validato. All'apparato ospite resta solo l'effetto fisico.
5. **Simmetria degli slot.** Tutti gli slot pari hanno lo stesso statuto: nessun glifo è "speciale" per posizione. I soli vincoli diversi derivano dalla **tile** che precede il glifo: `def` (Q11), le tile di portabilità (Q6), `abse ~` (D2/R14), `abdur ~` (B2).
6. **Termini.** Glifo e sema sono invariabili al plurale.
7. **Neutralità (v4.1).** La grammatica è un motore meccanico, deterministico e neutrale: non conosce né giudica il contenuto dei glifo e guida l'utente senza imporgli scelte o divieti arbitrari. Restano solo i vincoli necessari alla struttura (Slot Locking, DSP, tile) e alla sicurezza (caratteri invisibili, D9).

### 1.1 Nota di nomenclatura architetturale

Il nome dei **progetti e contenitori** (repository e crate Rust) è distinto dal nome del **protocollo** e del **programma**.

**Livello 0 — strutturale e relazionale (questo repository)**

- **Nome del progetto / contenitore: ApriWave** (*Apricot Wave*). Identifica l'ecosistema, il repository e i crate Rust (`apriwave-core` e `apriwave`). Unisce l'omaggio storico ad *Apricot Computers* (ACT) al concetto di onda portante e di funzione d'onda quantistica.
- **Nome del protocollo: GHLIP** (**Ghlip**), acronimo tecnico di **G**rammatical **H**ierarchical **L**inear **I**nter-species **P**rotocol. Identifica formalmente il protocollo grammaticale deterministico di Livello 0:
  - **G — Grammatical:** telaio chiuso e immutabile delle *Universal Grammatical Tiles* (*Tile Matrix*).
  - **H — Hierarchical:** gerarchia ad albero senza tag di chiusura, basata sul *Dynamic Stack Pruning* (`:`, `::`, `:::`).
  - **L — Linear:** scansione lineare deterministica basata sullo *Slot Locking* (`1 Tile -> 1 Glifo`).
  - **I — Inter-species:** framework neutrale privo di bias cognitivi o antropocentrici, nato per i log di contatto inter-specie e per la comunicazione M2M deterministica tramite il dizionario DTI.
  - **P — Protocol:** protocollo puro di Livello 0, separato dallo strato computazionale.
- **Disambiguazione terminologica:**
  - `GHLIP` (`Ghlip`) è l'acronimo formale che denomina l'intero protocollo di Livello 0.
  - Il termine `glifo` (in inglese `glyph`; invariabile al plurale in italiano: *i glifo*) indica esclusivamente il singolo elemento atomico censito nel dizionario DTI, all'interno della triade fondativa *Tiles* + *Glifo* = *Sema*.

**Livello 1 — computazionale e inferenziale (secondo progetto dell'ecosistema)**

- **Nome del secondo progetto: Apricot Logic.** È il progetto gemello di Livello 1, separato dal Livello 0.
- **Nome del programma / motore: LEMMA.** È il programma e motore logico e omoiconico di Livello 1. Opera sui periodi `/^ ... ^/` già validati dal protocollo `GHLIP` e si interfaccia con `apriwave-core` tramite il gancio transazionale `FunctionEvaluator` (`Session::evaluate_and_ingest`, §22).

---

## 2. Scrittura

| Regola | Dettaglio |
|---|---|
| Separatore | Esattamente **uno spazio ASCII** tra i token. Spazi doppi, iniziali o finali, tab e a capo danno ERR_SYNTAX (Q12). La scrittura libera è ammessa solo nell'editor (§21). |
| Tile | Minuscole; vietati `.` `\|` `(` `)` (M§1.4). |
| Glifo (v4.1) | **Qualsiasi carattere visibile**: ogni alfabeto, maiuscole, cifre, punteggiatura, simboli (`-`, `_`, `"`, `§`, `~`, `:`, `/`, `^`…). Sono vietati solo i caratteri invisibili (D9): controllo (`Cc`) e formato/bidi U+061C, U+200B–200F, U+202A–202E, U+2060–2064, U+2066–2069, U+FEFF (`MalformedGlyph { InvisibleCharacter }`). |
| Alternanza | Ogni blocco è una sequenza di coppie **[Tile] [Glifo]**: le tile occupano gli slot dispari, i glifo quelli pari. |
| Slot Locking (Q8, V2) | **Ogni token in uno slot pari è un glifo**, compresi i delimitatori di periodo (`/^`, `/^≈`, `^/`, `/^/`, `^^/`), le sequenze di `:`, i nomi delle tile (`a`, `ta`, `y`…), `"` e `§`. Fanno eccezione solo i due operatori esatti `~` e `~~`. Stadio 1 e Stadio 2 restano indipendenti dal DTI: il ruolo di un token dipende solo dalla posizione. |
| Glifo mancante | Poiché lo slot pari accetta qualsiasi token, un glifo manca solo a fine input (`/^ ta vedere nu` → ERR_CASE_AMBIGUITY). In `/^ ta vedere nu ^/` il token `^/` è il glifo di `nu` e il periodo resta aperto (ERR_SYNTAX `MissingPeriodClose`). **Doppio controllo:** oltre alla mancata chiusura, il token finito nello slot pari viene **sempre controllato sul DTI** come ogni glifo: se `^/` non è censito, l'editor lo evidenzia come non censito (`UnknownGlyph { structural: true }`) e chiede all'utente di censirlo nel DTI locale come **Glifo** o come **Etichetta** (§21.1); in un periodo valido (`/^ ta vedere nu ^/ ^/`) lo Stadio 3 lo risolve nel DTI come qualsiasi glifo (`UnknownForm` se non censito, §17.1). |

---

## 3. Periodo

```ebnf
session = period , { SP , period } ;
period  = open , SP , block , { SP , link , SP , block } , SP , close ;
```

| Apertura | Significato | Vincolo |
|---|---|---|
| `/^` | Inizio periodo; azzera il contesto DSP | — |
| `/^≈` | Simultaneità stretta con il periodo precedente | Richiede un antecedente (Q23): un periodo precedente, anche nello stesso input, **non** chiuso da `^^/` (D5, R12) |
| `/^≃` | Prossimità logica | Come sopra |

| Chiusura | Anafore di livello 0 | Variabili `def` | Sincronia |
|---|---|---|---|
| `/^/` Soft | **conservate**: restano visibili nei periodi successivi e si accumulano | conservate | conservata |
| `^/` Standard | cancellate | conservate | conservata |
| `^^/` Hard | cancellate | **cancellate** | **cancellata** |

---

## 4. Blocchi

| Blocco | Forma |
|---|---|
| Relazionale | `[segnale+determinanti] [glifo] { [caso] [glifo] }` |
| Definizione | `[def…] [nome] [op] [valore] { [op] [valore] }` (§12) |
| Portabilità | `anchor|anchorrel|rel|reloff <glifo>` oppure `end ~`, solo dopo `:` (§9.5) |

Il primo blocco del periodo non ha connettore e non può essere di portabilità.

---

## 5. Tile di segnale

### 5.1 Teste
| Codice | Significato |
|---|---|
| `ta` | Affermazione (default) |
| `to` | Negazione |
| `ka` | Domanda |
| `ko` | Domanda negativa |
| `ot` | Possibilità |
| `oh` | Vocativo (forza fratello nel DSP, §9.3) |
| `qua` / `que` / `quo` / `quu` | Secondari, solo nelle subordinate (Q22): indiretta base, ipotetica, impositiva, indeterminata. Sono seguiti **obbligatoriamente** da un primario (`quata`, `quotazato`). |
| `deep` | Canale M2M (forza livello 0 nel DSP, §9.3). Accetta determinanti (`deepall`), mai un primario (Q4). |
| `def`, `defu`, `defm`, `defnum` | Definizione: agglutina **solo** il numero (Q4) |

### 5.2 Determinanti (agglutinati, ordine fisso)
`[segnale][quantificatore][ref][comparativo][temporale | condizionale][numero]`

| Classe | Codici |
|---|---|
| Quantificatore | `all` (∀), `some` (∃) |
| Riflessivo | `ref`. Ammette l'Accusativo (v4.1): l'oggetto diretto non è obbligatorio (riflessivo diretto), ma è pienamente ammesso (riflessivo indiretto o transitivo: «il nodo si aggiorna il firmware», `taref aggiornare nu nodo a firmware`). |
| Comparativo | `cha` maggiore, `chu` minore, `cla` diverso, `col` uguale, `cal` simile, `cua` incluso, `cuo` escluso, `cup` unito |
| Temporale | `ato` presente (default), `alt` passato remoto, `alv` futuro remoto, `att` passato prossimo, `atv` futuro prossimo, `asu` tempo indeterminato |
| Condizionale | `z` + temporale: `zato`, `zalt`, `zalv`, `zatt`, `zatv`, `zasu` |
| Numero (regola 1B) | `u`, `m`, `num`, sempre in coda: `tau`, `tam`, `tanum`, `deepallu`, `tachazatou`. Se omesso in una subordinata si eredita dal segnale del padre (§6.1, §10). |

- Nessun determinante inizia per `u`, `m` o `n`: la lettura del numero in coda resta univoca.
- Un determinante fuori sequenza dà **ERR_ORDER**, anche dopo il numero (`tauzato`).
- Uno slot occupato due volte dà ERR_ORDER: è il caso dei duplicati (Q24), di temporale e condizionale insieme, come `taaltzato` (Q5), e del numero ripetuto (`tauu`, `deepmu`).
- `ref` si combina con qualsiasi comparativo (`tarefcha`, `taallrefchaato`): l'incompatibilità D6 è abrogata (v4.1).

---

## 6. Tile di caso

`[caso][determinante][numero]` e, solo per `abdur`, `=<N>`.

| Codice | Caso | Uso |
|---|---|---|
| `n` | Nominativo | Soggetto |
| `a` | Accusativo | Oggetto, oggetto di paragone |
| `g` / `gpa` | Genitivo / partitivo genitivo | |
| `d` / `dad` | Dativo / allativo (**solo** `dad`, Q2) | |
| `ab` | Ablativo generico | |
| `abef` | Elativo | |
| `aben` | Inessivo | |
| `abel` | Illativo | |
| `abse` | Abessivo ("senza") | |
| `abfi` | Translativo | |
| `abcu` | Istruttivo | |
| `absi` | Comitativo | |
| `abdu` | Temporale continuativo ("mentre / durante") — **mai un ciclo** | |
| `abpa` | Partitivo | |
| **`abdur=<N>`** | **Operatore di ciclo** (B2) | `N = [1-9][0-9]*` |

- **Numero**: omesso (Duale), `u` Singolare, `m` Plurale, `num` Non Duale. Esempi: `nu`, `am`, `abenu`, `dadnum`, `abduru=5`.
- **Casi ripetuti (v4.1).** Lo stesso caso può comparire più volte nello stesso blocco: più `nu`, più `au`, più `du` coordinati o elencati, più `abdur=<N>` (§13). Il divieto Q21 e l'eccezione riservata ad `absi` sono abrogati. L'ordine canonico è stabile e rispetta l'ordine scritto a parità di caso (§11.2).

### 6.1 Numero quantistico (H5)
| Stato | Marcatore | Significato |
|---|---|---|
| Duale | omesso | Sovrapposizione / indeterminazione (dualismo onda-particella) prima del collasso |
| Singolare | `u` | Collasso sull'unità |
| Plurale | `m` | Collasso sulla molteplicità |
| Non Duale | `num` | Quantità rilevata ma non ancora classificata |

Ogni coppia ha un numero effettivo (**regola 1B**):

| Coppia | Numero scritto nella tile | Numero omesso |
|---|---|---|
| Coppia del segnale (slot 1–2) | Prevale (`tau ~`, `tam gruppo`) | In una subordinata con segnale primario o secondario **eredita il numero del segnale del padre** (v4.1, §10); altrimenti Duale. Un `~` o una variabile `def` nello slot 2 non impongono il proprio numero. |
| Coppia di caso (slot 3–4 e seguenti) | Prevale | Duale; davanti a `~` eredita il numero della coppia antecedente, davanti a una variabile `def` il numero della variabile (Q18, R9.4) |

La regola è simmetrica a `def`/`defu`/`defm`/`defnum` e alle tile di caso: un numero esplicito scritto dall'utente prevale sempre.

---

## 7. Slot dei glifo

| Forma | Esempio | Note |
|---|---|---|
| Glifo | `canis`, `Roma`, `x-y`, `東京`, `30`, `^/`, `"` | Qualsiasi token di caratteri visibili in uno slot pari, tranne `~` e `~~` (§2). Nell'editor ha il colore del glifo. |
| Glifo qualificato | `video§rosso`, `verde§smeraldo§142` | `§` separa glifo distinti del DTI. Se però l'intero token (contenente o coincidente con `§`) è censito nel DTI come singolo glifo, **il DTI ha priorità** e il token è un glifo unico (§17.1). Lo Stadio 1 ammette anche segmenti vuoti (`§`, `x§`, `§x`); allo Stadio 3 un segmento vuoto è una forma sconosciuta. |
| Etichetta | `Roma` | Slot che l'utente ha marcato come etichetta con la selezione obbligatoria (§16.6); la forma si censisce nel DTI locale (`label`). Nel testo si scrive senza virgolette né prefissi e l'editor la colora come etichetta; in trasmissione indicizzata viaggia come `LVAAAA<forma>`. Le virgolette `"…"` non hanno più alcun valore grammaticale: `"` è un carattere come gli altri. |
| Anafora di glifo | `~` | Ultimo glifo canonico del blocco antecedente (§11) |
| Anafora di sema | `~~` | Intero sema antecedente, in qualsiasi slot pari (§11.5) |

Le forme `base~`, `base§mod~` e `glifo~~` sono **abrogate** (v4.1): un token come `video~` o `vedere~~` è un glifo ordinario.

---

## 8. Connettori

```ebnf
link = colon_run | conjunction , [ SP , colon_run ] ;
```

| Connettore | Profondità |
|---|---|
| `:`×N | N (con ancora relativa attiva: K + (N−1), §9.5) |
| `y`, `vel`, `aut` senza `:` | Profondità corrente dell'ultimo blocco attivo |
| `sed` senza `:` | Livello 0 (chiude i rami aperti) |
| Congiunzione + `:`×N | N |

Passare da profondità D a una profondità > D+1 dà **ERR_SCOPE** (Q14). Limite configurabile `max_dsp_depth`, default 64 (Q13).

---

## 9. Albero DSP

### 9.1 Collocazione
Lo stack DSP contiene il blocco attivo a ogni profondità. Collocare un blocco a profondità T tronca lo stack a T; il padre è `stack[T−1]`.

### 9.2 Coordinazione (Q15, R4, v4.1)
- I fratelli sotto lo stesso padre mantengono l'**ordine esatto di esposizione** dell'utente. Non esiste alcun albero di coordinazione né raggruppamento booleano presunto (`CoordNode` è rimosso): l'ordine lo decide l'utente, non la grammatica.
- Ogni blocco porta la propria congiunzione: `y`, `vel`, `aut` legano il gruppo dei fratelli, `sed` lega l'ultimo elemento. La lettura logica delle congiunzioni appartiene ad Apricot Logic.
- Fratelli scritti senza congiunzione sono rami distinti.
- `sed` senza `:` riporta al livello 0 (§8).
- Una congiunzione senza fratello a sinistra (ad esempio dopo un `def` trasparente) resta sulla propria clausola.

### 9.3 Ruoli forzati (H4, Q16 — "Explicit colons win")
| Segnale | Senza `:` | Con `:` espliciti |
|---|---|---|
| `oh` (anche nel secondario) | Fratello dell'ultimo blocco attivo | Rispetta i `:` |
| `deep` | Livello 0 | Rispetta i `:` |
| `def` | Trasparente: non entra nello stack | I `:` sono solo verificati (salto di livello) |

### 9.4 Esempio
`/^ ta vedere nu io au canis : ta abbaiare nu ~ :: tazato fame nu ~ : ta osservare nu gatto ^/` produce: vedere(0) → abbaiare(1) → fame(2); osservare(1) è figlio di vedere.

### 9.5 Ancore (Q6, R1–R3, H3)
Le tile di portabilità compaiono solo dopo `:`, mai in testa né dopo una congiunzione (D1). Non accettano coppie di caso.

| Tile | Effetto |
|---|---|
| `: anchor <id>` | Verifica i `:`, tronca lo stack, registra l'ancora con profondità di inserimento, modalità **assoluta** |
| `: anchorrel <id>` | Come `anchor`, ma attiva la modalità **relativa** |
| `: rel <id>` | Riattiva la modalità relativa su `<id>` (ERR_REFERENCE se assente) |
| `: reloff <id>` | Disattiva la modalità relativa; l'ancora resta aperta |
| `: end ~` | Chiude l'ultima ancora, riporta lo stack alla sua profondità (ERR_SCOPE se non c'è un'ancora aperta) |

- In modalità relativa con un'ancora di profondità K, una clausola con C `:` va a profondità **K + (C−1)**.
- Un'ancora scritta in modalità relativa salva come proprio K la profondità effettiva (R1).
- Le ancore sono locali al periodo: un'ancora non chiusa dà ERR_SCOPE (R3).
- I blocchi di portabilità non sono nodi dell'albero (R2).
- Nell'editor `Ctrl+C` e `Alt+C` copiano le subordinate selezionate come blocco `anchor` o `anchorrel` (§21.4).

---

## 10. Ereditarietà e controlli

| Regola | Contenuto |
|---|---|
| Q17 / R5 (v4.1) | Una subordinata con segnale primario o secondario eredita dal padre (valori effettivi, quindi in modo transitivo) **tempo/condizionale, numero del segnale e tutti i determinanti omessi**: quantificatore (`all`/`some`), riflessivo `ref` e comparativo. Un determinante scritto sulla subordinata prevale sempre (override). `deep` non eredita (salvo i determinanti scritti su `deep`). |
| Q19 | Un comparativo **scritto** richiede almeno un Accusativo (`a`, `au`, `am`, `anum`), altrimenti ERR_SEMANTIC. Un comparativo ereditato non impone nulla. |
| Q22 | Segnale secondario a livello 0: ERR_SCOPE. |

Abrogati in v4.1: D6 (`ref` + comparativo), D8 (`ref` + Accusativo), Q20 (`QuantifierMisuse`: quantificatore senza coppie di caso), Q21 (casi duplicati).

---

## 11. Anafore

### 11.1 Antecedenti (R6)
L'ordine di ricerca è:
1. il padre;
2. gli antenati fino alla radice;
3. i blocchi di livello 0 precedenti, dal più recente;
4. i blocchi di livello 0 conservati da `/^/`.

I rami potati sono invisibili. `~` non vede il proprio blocco.

### 11.2 Ordine canonico
Prima la coppia del segnale (posizione 0), poi le coppie di caso in ordine `n → a → d → g → ab`. L'ordinamento è **stabile** e rispetta l'ordine scritto a parità di caso: in `au 2 au 1` la coppia `au 2` precede `au 1`.

### 11.3 Candidati (stessa regola in ogni coppia)
I candidati sono tutte le coppie del blocco antecedente, in ordine canonico inverso. Per un dato blocco:

| Anafora | Regola |
|---|---|
| `~` | La prima coppia che non contiene un'anafora di sema, cioè l'ultimo glifo canonico del blocco antecedente |

- Le catene di `~` vengono seguite fino al valore finale (R9.1).
- Ogni glifo, etichette comprese, è un antecedente valido.
- Il numero di un candidato è il numero effettivo della sua coppia (§6.1): per una variabile `def` in una coppia di caso è il numero della variabile (R9.4).

### 11.4 Numero
Il numero scritto nella tile prevale. Se è omesso, una coppia di caso eredita quello dell'antecedente (Q18); la coppia del segnale segue la regola 1B (§6.1).

### 11.5 Anafore di sema (R8)
- `~~` si riferisce al primo blocco dell'ordine di ricerca.
- Senza antecedente: ERR_REFERENCE.
- `~~` si comporta allo stesso modo in **ogni** slot pari, compreso lo slot 2: risolve al sema antecedente (`SemaAnaphora { clause }`, valore `Sema(clause)`) senza mai copiare o fondere nel blocco corrente le coppie di caso del sema richiamato. Ogni blocco si valida solo sulle proprie coppie scritte.

---

## 12. Variabili `def` (H1, Q11, R11, R13, R14, D2)

| Operazione | Tile | Effetto |
|---|---|---|
| Assegnazione | `a`, `au`, `am`, `anum` | Crea la variabile. Su una variabile già viva dà ERR_SEMANTIC; un numero in conflitto con `def[n]` dà ERR_SEMANTIC. |
| Modifica | `abfi` (senza numero, D2) | Nuovo valore |
| Unione | `absi` (senza numero) | Aggiunge un valore |
| Cancellazione | `abse ~` | Rimuove la variabile; `~` è un marcatore, non un'anafora (R14) |

- Il nome è il **token intero** dello slot (qualsiasi glifo, anche con `§`); `~` e `~~` non possono essere nomi (`DefNameNotAGlyph`, ERR_SYNTAX).
- È richiesta almeno un'operazione, altrimenti ERR_SEMANTIC.
- Dopo un `def` serve sempre un connettore (`y` o `:`).
- Usare `abfi`, `absi` o `abse` su una variabile inesistente dà ERR_REFERENCE. Usarli con un `def[n]` di numero diverso da quello della variabile dà ERR_SEMANTIC.
- **R13**: finché la variabile è viva, ogni glifo uguale al suo nome (token intero), in qualsiasi coppia di un blocco relazionale, è un riferimento alla variabile.

Esempio canonico: `/^ defu dev a sensore§ottico y defu dev abfi sensore§termico y ta leggere nu dev y defu dev abse ~ ^/`

---

## 13. Cicli `abdur=<N>` (B2, L1–L5)

- `abdu` è **solo** il complemento temporale.
- `abdur=<N>` è la forma base; per simmetria morfologica sono ammesse `abduru=<N>`, `abdurm=<N>`, `abdurnum=<N>`. Ripete nel piano **la clausola e tutto il suo sottoalbero** per N volte.
- **Più cicli nello stesso blocco (v4.1):** più tile `abdur=<N>` nella stessa clausola formano cicli **annidati nell'ordine scritto** (N₁ × N₂ × …): il primo scritto è il più esterno.
- Il glifo che segue la tile è `~` (marcatore strutturale) oppure un glifo qualsiasi. Il suo valore **non** viene mai letto per il conteggio.
- La forma `abdur` senza `=<N>` è un **marcatore di controllo del ciclo**. È ammessa in una subordinata (profondità ≥ 1) il cui segnale porta un condizionale **scritto o ereditato** dal padre (v4.1, §10): `tazato x : ta c abdur ~` è valido. Un temporale indicativo scritto sulla subordinata annulla il condizionale ereditato (override). Fuori da questo contesto dà ERR_SCOPE. Il protocollo ne valida la struttura; la valutazione della condizione appartiene ad Apricot Logic (L4).
- Superare `max_loop_iterations` (default 1000) con N, o con i passi generati dai cicli annidati, dà `LimitExceeded{LoopIterations}`, cioè ERR_SCOPE.

**Piano di esecuzione** (`plan::build_execution_plan`):
- `ExecutionPlan { steps: Vec<Step { clause, iteration, targets }> }`. `iteration` contiene un indice per ogni ciclo che racchiude il passo, dal più esterno; i cicli della stessa clausola seguono l'ordine scritto.
- I passi seguono l'ordine dell'albero: prima ogni blocco, poi i suoi figli, radici in ordine.
- Le subordinate di controllo vengono emesse una sola volta, prima del corpo ripetuto.
- `def` e i blocchi di portabilità non generano passi.

Esempi:
- `/^ ta x abdur=2 ~ : ta y abdur=2 ~ ^/` produce x[0], y[0,0], y[0,1], x[1], y[1,0], y[1,1];
- `/^ ta x abdur=2 ~ abdur=3 ~ ^/` produce x[0,0], x[0,1], x[0,2], x[1,0], x[1,1], x[1,2].

---

## 14. Insiemi strutturali (B3, S1–S3)

L'insieme di riferimento è la tassonomia `base:` delle chiavi abilitate più i valori delle variabili `def`. Una clausola ha bersagli solo se ha un quantificatore oppure un comparativo `cup`, `cua` o `cuo`.

| Elemento | Operazione |
|---|---|
| Coppie in Accusativo (`a`, `au`, `am`, `anum`) | Insieme base |
| Coppie in Nominativo (`n`, `nu`, `nm`, `nnum`), **solo se il blocco non ha Accusativi** | Insieme base (clausole intransitive o riflessive quantificate) |
| `all` / `some` | Espandono base e unioni con i discendenti abilitati |
| `cup` / `absi` | Unione |
| `cua` / `abpa` (con `cua`, gli operandi `absi`) | Intersezione con l'operando e i suoi discendenti |
| `cuo` / `abse` (con `cuo`, gli operandi `absi`) | Sottrazione dell'operando e dei suoi discendenti |

- **Operando segmentato** `<K1>§<K2>`: individua il record **abilitato** del DTI con `base:K1` e `param:K2`; le catene più lunghe `<K1>§<K2>§…§<Kn>` si riducono da sinistra (`(K1§K2)§K3`…). Se il record composto non esiste tra le voci abilitate, o non è unico, il piano restituisce `PlanError::Composite`. Esempio: `sensore§ottico` → il record `base:sensore param:ottico`.
- L'ordine di applicazione è: unione → intersezione → sottrazione. Il risultato è ordinato per chiave, senza duplicati.
- Modalità: `all` o nessun quantificatore → `All`; `some` → `AtLeastOne` (l'elenco resta deterministico).

Esempi canonici:
- `/^≈ deepall attivare nu sistema am sensore abse sensore§ottico ^/`
- `/^≈ deepall avvio nm sensore abse sensore§ottico ^/` (base sul Nominativo)
- `/^≈ deepall attivare nu sistema am sensore abpa sensore§ottico ^/`
- `/^≈ deepallcuo attivare nu sistema a sensore absi sensore§ottico ^/`
- `/^≈ deepallcup attivare nu sistema a sensore§ottico absi sensore§termico ^/`

---

## 15. Diagnostica

| Codice | Esempi |
|---|---|
| ERR_SYNTAX | Token o struttura invalidi; spazi non canonici; tile di segnale senza glifo (a fine input); periodo non chiuso; vincoli di `def` e delle tile di portabilità; glifo con caratteri invisibili (D9) |
| ERR_ORDER | Determinanti fuori sequenza o duplicati (anche temporale + condizionale) |
| ERR_REFERENCE | Anafora senza antecedente; ancora sconosciuta; variabile non definita |
| ERR_SEMANTIC | Comparativo scritto senza Accusativo; `def` senza operazioni; riassegnazione; conflitto di numero |
| ERR_SCOPE | Salto di livello; sincronia senza antecedente; portabilità fuori posto; ancore non chiuse; limiti (profondità, cicli, dimensioni); Q22; `abdur` senza condizionale scritto o ereditato |
| ERR_CASE_AMBIGUITY | **Solo** tile di caso priva del glifo successivo (a fine input, §2) |

- L'ingestione è **atomica**: con qualsiasi diagnostica la sessione resta invariata e gli errori di entrambi gli stadi tornano ordinati per posizione.
- DTI, policy, Ghost, piano ed editor hanno tipi d'errore **separati** dai 6 codici grammaticali (Q26).

### 15.1 Declassamento a warning (v4.1, V9)
- **Di default tutti i codici sono errori bloccanti** (`Severity::Error`).
- L'utente può declassare, **a proprio rischio e pericolo**, ogni singolo codice a warning (`Severity::Warning`): `ParserConfig::warnings` (`WarningCodes`, con `with(code)` / `without(code)`) e la sezione `[warnings]` di `settings.toml` (§21.2), dove ogni codice si imposta individualmente. Un codice non elencato resta un errore.
- Restano **sempre bloccanti** gli errori strutturali, che impediscono di costruire un albero completo e coerente (`DiagnosticKind::is_structural`): tutti i casi ERR_SYNTAX e ERR_ORDER, `def` senza operazioni, portabilità fuori dalla subordinazione, limiti superati, salto di livello DSP, `end` senza ancora, ancora non chiusa, tile di caso senza glifo.
- Un codice declassato non rifiuta il periodo in `Session::ingest` (la firma non cambia): la diagnostica si legge da `AnalyzedPeriod::warnings()` (`Diagnostic { severity, .. }`, `is_warning()`), assegnata al periodo il cui testo la contiene, e compare nell'export JSON e nella CLI (`--warn=<CODICE>`).

---

## 16. DTI a due livelli (Fase B)

### 16.1 Architettura (D1)
| Livello | Contenuto |
|---|---|
| **Snapshot universale** | In sola lettura, sostituibile in blocco, una lingua di interscambio |
| **Overlay locale** | Voci abilitate (`use`), forme locali, concetti locali (`new`), etichette (`label`, v4.1), policy delle chiavi |

Il registro universale esterno contiene tutte le lingue. Lo snapshot locale si aggiorna con la sola lingua di interscambio, perché le chiavi sono immutabili.

### 16.2 Chiave (D2, B1, B2, B4)
- 32 caratteri `[a-z0-9]`: 31 casuali più **un carattere di controllo Luhn mod 36**, che rileva ogni sostituzione di un singolo carattere.
- **Il primo carattere è sempre una lettera `[a-z]`** (poi 30 caratteri `[a-z0-9]` e il controllo). In forma indicizzata un token che inizia con una lettera è una chiave e un token che inizia con una cifra è un numerale letterale: la distinzione è O(1) e non collide nemmeno con numeri di 32 cifre. `DtiKey::parse` e `DtiKey::from_random_body` rifiutano una cifra iniziale con `KeyError::FirstCharacter`.
- La chiave è **per concetto**, non per glifo.
- Una chiave non si modifica, non si riusa e non si cancella; la deprecazione si esprime con `superseded_by:`.
- La generazione casuale avviene **fuori dal crate**. `DtiKey::from_random_body` calcola il carattere di controllo.

### 16.3 Formato `.awdti`
```text
#awdti 1
#role universal
#version 42
#lang en
#numeral base:10 digits:<K0>,…,<K9>
rec <KEY> <primaria> [display:"…"] [syn:a,b] [base:<KEY>] [param:<KEY>] [morph:+AGGL,+CONTR,+UNION,+AGGL_NUM] [superseded_by:<KEY>] [attr:k§v]
```
```text
#awdti 1
#role local
#universal 42
#lang it
#policy degrade|unknown|reject
use <KEY> [form:<primaria>] [display:"…"] [syn:…]
new <KEY> <primaria> [base:<KEY>] [display:"…"] [syn:…] [morph:…]
label <KEY> <forma>
```

Un campo è delimitato da un singolo spazio. Solo i valori di `display:` e `attr:` possono contenere spazi tra `"…"`; ogni altro campo (tipo, chiave, forme, `form:`, `syn:`) si legge alla lettera, quindi una forma può contenere qualsiasi carattere visibile, `"` compreso.

**Controlli al caricamento:**
- header obbligatori e versione del formato;
- forme valide (v4.1): la forma primaria, la `form:` locale, i sinonimi `syn:` e le etichette sono token di **qualsiasi carattere visibile**, `§` compreso (una forma con `§` è un glifo unico, §17.1); sono rifiutati solo gli operatori esatti `~` e `~~` e i caratteri invisibili (`DtiErrorKind::InvalidForm`). `QualifiedForm` è rimosso. Le locuzioni umane usano `display:"verde smeraldo"`; la composizione concettuale usa `base:` e `param:`;
- **forma primaria unica per lingua** (B3); sinonimi e `display` possono valere per più concetti;
- chiavi duplicate e riferimenti orfani;
- cicli nelle catene `base`;
- **anti-spoofing**: un `new` o un `label` non può riusare una chiave universale;
- `label`: solo nel file locale (nell'universale dà `UnknownRecordKind`), nessun tag ammesso, chiave unica nell'overlay, forma unica tra le etichette (`DuplicateLabel`); la stessa forma può esistere anche come glifo (`use`/`new`);
- **avviso di censimento** (mai un errore): se la forma di un glifo locale (`use … form:`, `new`) o di un'etichetta è già la forma primaria di un altro glifo universale nella lingua di interscambio, `LocalOverlay::census_warnings()` restituisce un `CensusWarning { line, kind, form, universal }`, così l'utente non invia inconsapevolmente un'etichetta senza sapere che il glifo esiste;
- `use` su una chiave deprecata: errore che indica la sostituta (B6);
- `#universal` diverso da `#version`: errore (B11);
- `#numeral` con base 2–10, tante cifre quanta è la base, chiavi esistenti.

Il tag `attr` è opaco (B8). I flag `morph` vengono **validati attivamente** nello Stadio 3 e nell'editor (§17.1, B9). Una lingua per overlay (B7).

### 16.4 Formato binario `.dti.bin` (architettura duale C1)
Accanto al parser testuale `.awdti` il modulo `dti` compila e carica un formato binario deterministico (Rust puro, nessuna dipendenza, `#![forbid(unsafe_code)]`). Tutti i valori sono little-endian.

| Parte | Contenuto |
|---|---|
| Header | magic `AWDTIBIN` (8 byte), versione del formato `u32` (= **2** in v4.1), tipo `u8` (0 universale, 1 locale), **hash FNV-1a 64** dei byte del sorgente `.awdti` |
| Metadati universali | `#version` `u64`, `#lang`, `#numeral` (flag, base, chiavi delle cifre), numero di record `u32` |
| Metadati locali | `#universal` `u64`, `#lang`, `#policy` `u8`, numero di record `u32` |
| Sezioni universali | Una colonna per campo, nell'ordine dei record: chiavi, forme primarie, `display`, sinonimi, `base`, `param`, `morph`, `superseded_by`, `attr` |
| Sezioni locali | Tipo (`0` `use`, `1` `new`, `2` `label`), chiavi, forme, `display`, sinonimi, `base`, `morph`. I record `label` seguono i record di concetto, in ordine di chiave. |

- **Codifiche:** chiave = 32 byte ASCII; stringa = lunghezza `u32` + UTF-8; opzionale = tag `u8` + valore; lista = conteggio `u32` + elementi; `morph` = bit (1 AGGL, 2 CONTR, 4 UNION, 8 AGGL_NUM).
- **Indici:** non vengono memorizzati e si ricostruiscono al caricamento.
- **Stessa validazione del testo, senza testo:** il caricamento costruisce il modello direttamente dalle colonne decodificate, senza mai produrre, formattare o riparsare testo `.awdti`. Usa le **stesse funzioni** del parser testuale:
  - controlli per record: tag ammessi dal tipo di record, forme valide (`InvalidForm`), sinonimi unici, `display` e `attr` ben formati;
  - valori di testata: `#lang` e `#numeral` (base 2–10, una cifra per base);
  - controlli incrociati: riferimenti esistenti (`base`, `param`, `superseded_by`, `use`), divieto di `use` su chiavi deprecate, assenza di cicli `base`, anti-spoofing (`new`/`label` su chiave universale), forma primaria unica, etichette uniche. Gli avvisi di censimento si ricalcolano (con il numero d'ordine del record come riga) e non partecipano all'uguaglianza del modello.
- Le diagnostiche di un binario (`Invalid`) indicano come riga il numero d'ordine del record (1, 2, …) e 0 per i metadati.
- Il modello viene poi ricodificato e deve coincidere byte per byte con l'input, altrimenti `NonCanonical`: si accetta solo la codifica canonica.
- **Determinismo:** lo stesso modello produce sempre gli stessi byte, e `from_binary(to_binary(m)) == m`.
- **Robustezza:** ogni lettura è controllata nei limiti; i conteggi sono limitati dai byte rimanenti prima di ogni allocazione; i byte in coda sono rifiutati; dimensione massima `MAX_BINARY_BYTES`.

**API** (`apriwave::dti`):
- `UniversalSnapshot::to_binary(&self, source_hash) -> Vec<u8>` e `UniversalSnapshot::from_binary(bytes) -> Result<(Self, u64), DtiLoadError>`;
- `LocalOverlay::to_binary(&self, source_hash)` e `LocalOverlay::from_binary(bytes, &universal)`: l'overlay viene sempre rivalidato contro lo snapshot fornito;
- `UniversalSnapshot::parse`, `LocalOverlay::parse`, `Dti::from_parts` (che ricontrolla l'accoppiamento snapshot/overlay) e `fnv1a_64`;
- `load_or_compile_universal(sorgente, Option<binario>)` e `load_or_compile_local(sorgente, Option<binario>, &universale)`: se il binario è valido e ha lo stesso hash FNV-1a del sorgente si carica dal binario (`None`); altrimenti, se è assente, corrotto o obsoleto, si compila il testo e si restituiscono i nuovi byte (`Some`);
- varianti su file `load_or_compile_universal_path` e `load_or_compile_local_path`, **solo nel crate host `apriwave`** (`apriwave::dti`, errore `DtiFileError { Io, Load(DtiLoadError) }`): la cache sta accanto al sorgente (`foo.awdti` → `foo.dti.bin`, `binary_cache_path`), un errore di lettura della cache vale come cache mancante, e la riscrittura avviene con file temporaneo e rinomina.
- Errori `DtiLoadError`: `BadMagic`, `UnsupportedVersion`, `WrongKind`, `Truncated`, `TrailingBytes`, `InvalidUtf8`, `LengthOverflow`, `TooLarge`, `InvalidKey`, `Malformed`, `NonCanonical`, `Invalid`, `Source`. Le funzioni su buffer stanno in `apriwave-core` e non usano il file system.

### 16.5 Ricerca
| Funzione | Comportamento |
|---|---|
| `resolve_primary` | Solo le forme primarie (locale e di interscambio, nessuna priorità tra le lingue). È l'unica ricerca usata dallo Stadio 3: il testo canonico ammette solo forme primarie. |
| `resolve_form` | Prima le forme primarie, poi sinonimi e `display`. La usa solo l'editor per normalizzare (§21.1). Restituisce un esito unico, ambiguo (l'editor chiede) o sconosciuto. |
| `compounds(base, param)` | Voci abilitate con quella coppia `base:`/`param:` (operandi segmentati degli insiemi, §14) |
| `label(forma)` | Chiave dell'etichetta locale con quella forma (v4.1); `overlay().labels()` elenca le etichette |
| `resolve_key` (D3) | Restituisce `Enabled` per le chiavi abilitate o locali. Per le altre segue la policy: `degrade` risale la catena `base:` fino alla prima voce abilitata, `unknown` le segnala come sconosciute, `reject` le rifiuta. |

### 16.6 Etichette (v4.1, V5)
- Un'**etichetta** è un elemento salvato nel **DTI locale** con il tipo *Etichetta* (`label <KEY> <forma>`), assente dal DTI universale. Sostituisce le vecchie etichette tra virgolette, abolite dalla grammatica.
- **Coesistenza:** lo stesso valore (es. `roma`) può esistere contemporaneamente nel DTI locale come glifo (`use`/`new`) e come etichetta.
- **Controllo sul DTI universale:** al censimento di un glifo o di un'etichetta, se il termine è già un glifo universale nella lingua di interscambio si produce un avviso di censimento (§16.3).
- **Selezione obbligatoria (V5, rettifica del 15/11/2026):** quando una forma esiste nel DTI locale come etichetta (sola, o insieme a un glifo), l'utente per scriverla **deve selezionare dalla tendina** se è **Glifo** o **Etichetta** (§21.3). La scelta si registra per slot nella sessione (`Session::set_slot_kind(slot, SlotKind::{Glyph, Label})`) e decide colore e trasmissione. Ogni slot ha quindi sempre una classificazione deterministica (`Session::effective_slot_kind`): nei flussi programmatici o da CLI senza selezione interattiva, uno slot non marcato come `Label` è un glifo (`SlotKind::Glyph`, default). Non esiste alcun caso «glifo o etichetta» irrisolto: `IndexError::GlyphOrLabel` è rimosso.
- **Editor:** l'etichetta si scrive e si visualizza senza virgolette né prefisso, con il colore delle etichette. Uno slot marcato come etichetta è una **stringa inerte**, che il pre-processore non tocca mai (nessuna normalizzazione, nessuna locuzione, nessun controllo da glifo); uno slot marcato come glifo segue le regole del DTI.
- **Omografi e sinonimi (falso problema):** il protocollo ignora il contenuto semantico. Anche se due etichette identiche (per esempio la stessa sigla) indicassero due oggetti diversi nel mondo reale, la contestualizzazione appartiene a mittente e ricevente: il protocollo trasmette comunque la stessa identica stringa `LVAAAA<forma>`. Allo stesso modo, una forma che è anche sinonimo di un glifo non crea conflitti: decide la selezione dello slot.
- **Trasmissione:** in forma indicizzata un glifo viaggia come `DtiKey` (32 caratteri minuscoli `[a-z][a-z0-9]{31}`), un'etichetta come **`LVA<SOTTOTIPO><forma>`** (es. `LVAAAAroma`): `LVA` (3 caratteri) identifica l'etichetta, il sottotipo è di 3 lettere maiuscole `[A-Z]{3}` (`AAA` = etichetta base, l'unico generato da questa implementazione). Il prefisso è generato dalla macchina mittente, mai scritto a mano dall'utente. Le chiavi sono minuscole e i numerali iniziano con una cifra, quindi il prefisso maiuscolo garantisce una distinzione O(1) senza collisioni.
- **Ricevente:** un token che inizia con `LVA` è **sempre e solo un'etichetta**, anche se assente dal proprio DTI. Se la forma è anche un glifo del DTI del ricevente, l'etichetta resta tale ma viene **evidenziata** per segnalare l'omonimia (`IndexedLabel { slot, form, subtype, homonym }`, §17.3).
- **Sottotipi sconosciuti e auto-aggiornamento:** un sottotipo diverso da `AAA` significa che era previsto nella grammatica della macchina che ha inviato. Il ricevente **non rifiuta il messaggio**: tratta il token come etichetta valida (`LabelSubtype::Known(codice)` se `AAA` o già registrato, altrimenti `LabelSubtype::Dynamic(codice)`), registra il nuovo codice nella tabella dei sottotipi della sessione (`Session::register_label_subtype`, `label_subtypes`; la tabella sopravvive a `compact`) con una sola nota informativa (`NewLabelSubtype { slot, code }`) e conserva i 3 caratteri se ri-renderizza il messaggio in forma indicizzata. Solo un token troncato o corrotto (più corto di 7 caratteri, o con sottotipo non `[A-Z]{3}`) è un errore (`MalformedLabel`).

> [!IMPORTANT]
> **Nota di sicurezza nella grammatica — esecuzione in ambiente safe per le etichette non censite.**
> - Le etichette sono stringhe opache che viaggiano con il prefisso `LVA<SOTTOTIPO><forma>` (default `LVAAAA<forma>`).
> - Se un'etichetta non è censita correttamente nel DTI locale del nodo ricevente (nota diagnostica `UncensusedLabel`, oppure etichetta esterna ricevuta in modalità `label allow`), oppure presenta un sottotipo dinamico non ancora classificato (`LabelSubtype::Dynamic`), la grammatica prescrive che il runtime o sistema ospite la tratti ed esegua **esclusivamente all'interno di un ambiente safe isolato** (sandbox / contesto dati puramente passivo), **privo di privilegi di esecuzione diretta** su comandi di sistema o attuatori fisici.

---

## 17. Stadio 3 (Fase C)

### 17.1 Risoluzione glifo → chiave (B4, B5, B9)
1. Il testo canonico (Stadi 1, 2 e 3) ammette **solo forme primarie**, univoche 1:1 per lingua (B3). Sinonimi e `display` vengono normalizzati esclusivamente dall'editor (§21.1); lo Stadio 3 cerca solo nelle forme primarie.
2. **Priorità al DTI (v4.1, V3):** si cerca prima il **token intero** tra le forme primarie, anche se contiene `§`: un `a§b` censito è un glifo unico con una sola chiave. Solo se il token intero non è una forma, un percorso `s1§s2§…§sn` si risolve **segmento per segmento** in `<K1>§<K2>§…§<Kn>` (con `+UNION`); un segmento vuoto è una forma sconosciuta (`UnknownForm`).
3. **Glifo o etichetta (deterministico):** un token già in forma di etichetta di trasmissione (`LVA<XYZ><forma>`, generato da una macchina) resta invariato; altrimenti decide la classificazione dello slot nella sessione: marcato `Label` → etichetta, altrimenti (marcato `Glyph` o non marcato) → glifo risolto nel DTI. Il controllo sul DTI si esegue quindi su ogni glifo, compresi i delimitatori finiti in uno slot pari (§2).
4. Se lo snapshot dichiara `#numeral`, un atomo di sole cifre è un **numerale letterale**: ogni cifra deve essere minore della base (`InvalidNumeral`).
5. Una voce non abilitata è un errore che suggerisce `use <KEY>`.

**Validazione attiva dei flag `morph`** (`ResolveError::MorphViolation { form, key, flag }`, con `MorphFlag::{Union, Aggl, Contr, AgglNum}`):

| Flag | Quando si verifica | Chi deve averlo |
|---|---|---|
| `+UNION` | Un glifo qualificato `a§b` risolto per segmenti (n ≥ 2) | Ogni concetto unito da `§` (i numerali non sono concetti e sono esenti: in `30§celsius` lo richiede solo `celsius`) |
| `+AGGL` | Record con `base:` e `param:` la cui forma è un atomo uguale alla concatenazione di una forma primaria di `base` e una di `param` (`cagn` + `one` → `cagnone`) | Entrambi i componenti (`base` e `param`) |
| `+CONTR` | Record con `base:` e `param:` la cui forma è un atomo diverso dalla concatenazione (`cucciolo` = cane + piccolo) | Entrambi i componenti |
| `+AGGL_NUM` | Ogni cifra di un numerale letterale | Il concetto della cifra (`#numeral … digits:`) |

- Le forme agglutinate o contratte vanno censite come record: lo Stadio 3 non segmenta mai automaticamente un atomo sconosciuto, perché in forma indicizzata un'agglutinazione segmentata non sarebbe distinguibile da un'unione `§`.
- La stessa validazione vale all'ingresso (`validate_indexed_input`: `+UNION` sulle chiavi unite da `§`, `+AGGL_NUM` sulle cifre) e nell'editor (§21.1).

### 17.2 Forma espansa (D8)
È una **vista di risoluzione** per le persone, non la forma di scambio. La forma espansa:
- non contiene blocchi `def` né di portabilità;
- usa `:` espliciti per ogni profondità ≥ 1;
- sostituisce variabili e `~` con il loro valore. Nelle **coppie di caso** riscrive la tile con il numero effettivo (`n ~` → `nu humanum`; `a gruppo` con `defm gruppo` → `am …`). La tile di segnale resta come è scritta: con il numero esplicito se c'era, altrimenti con il numero ereditato dal padre o Duale, che una nuova lettura ricostruisce identico (regola 1B). Sostituire `~` o una variabile nello slot 2 non fallisce mai;
- **variabile multi-valore in una coppia di caso**: la tile originale (con il numero effettivo) per il primo valore, poi una coppia `absi <valore_i>` per ogni valore successivo. Con `dev = [s1, s2]`: `au dev` → `au s1 absi s2` (V6);
- **`~~` resta `~~`** in qualsiasi slot pari: rimuovere i blocchi `def` e di portabilità non cambia mai il sema più recente, perché quei blocchi non sono clausole relazionali;

Quando qualcosa non è esprimibile produce un **errore di esportazione**, mai un output invalido:

| Errore | Causa |
|---|---|
| `MultiValuedVariable` | Variabile multi-valore nella coppia del segnale (slot 2) |
| `InexpressibleDepth` | Profondità che richiede ancore |
| `InexpressibleValue` | Valore che non può stare in una posizione di glifo |
| `EmptyExpansion` | Il periodo contiene solo `def` |

`InexpressibleNumber` e `InexpressibleSema` **non esistono più**.

### 17.3 Forma indicizzata e canale (D4)
- **La forma indicizzata M2M non dipende dalla forma espansa.** È la forma canonica **scritta** del periodo validato in cui ogni glifo del DTI è sostituito dalla sua `DtiKey`. Restano intatti i blocchi `def`, le tile di portabilità (`anchor`, `anchorrel`, `rel`, `reloff`, `end ~`), le anafore (`~`, `~~`), le tile e i numerali; le etichette diventano `LVAAAA<forma>`.

| Elemento scritto | Forma indicizzata |
|---|---|
| Glifo negli slot e nei valori `def` | La chiave del token intero se è una forma primaria (anche con `§`); altrimenti una chiave per segmento: `<K1>§<K2>…` |
| Etichetta (v4.1) | Slot marcato `Label` → `LVAAAA<forma>`; token già `LVA<XYZ><forma>` → invariato (sottotipo conservato) |
| Glifo già indicizzato (ritrasmissione) | Un glifo i cui segmenti sono già chiavi abilitate (o numerali) resta invariato; un simbolo di sessione che è già una chiave resta quella chiave: un periodo ricevuto in forma indicizzata si ri-renderizza identico |
| Nome di variabile `def` e suoi riferimenti (R13: glifo risolto dallo Stadio 2 come variabile viva) | **Simbolo di sessione**: la `DtiKey` se il nome è la forma primaria di una voce abilitata, altrimenti una **chiave effimera** (anche se la voce esiste ma non è abilitata) |
| Id di ancora (`anchor`, `anchorrel`, `rel`, `reloff`) | Simbolo di sessione del **token intero** (`porta§uno` → `<K_eff(porta§uno)>`; `porta` abilitata → `<K_porta>`) |
| `~`, `~~`, numerali | Invariati |

**Chiave effimera di sessione** (`DtiKey::session_symbol(nome)`): SHAKE256 interno su `len(dominio) u64 LE ‖ dominio ‖ nome`, con dominio `apriwave-session-symbol-v1`. I byte si leggono con estrazione per rifiuto senza bias: il primo carattere da `[a-z]` (byte < 234, mod 26), i 30 seguenti da `[a-z0-9]` (byte < 252, mod 36), poi il controllo Luhn mod 36. È deterministica: lo stesso nome dà sempre la stessa chiave, quindi nome e riferimenti coincidono. Una forma primaria condivisa da due voci abilitate nelle due lingue resta ambigua (B3).

- Il **100% dei periodi validi allo Stadio 2** (anche quelli di sola definizione `def` o con ancore relative) è trasmissibile, con la sola condizione che ogni glifo ordinario sia una forma primaria del DTI o uno slot marcato come etichetta. `~` e `~~` si risolvono in modo identico da entrambe le parti.
- `render_wire(WireMode::Lexical)` restituisce la forma canonica scritta con i glifo lessicali; `WireMode::Indexed` la stessa struttura con le chiavi. La modalità del canale è concordata **fuori banda**.
- Lo scambio M2M avviene solo in forma indicizzata.
- `validate_indexed_input` verifica un periodo ricevuto. Un token che inizia con `LVA` è un'etichetta: qualunque sottotipo `[A-Z]{3}` finisce in `IndexedReport::labels` (con `subtype` e con `homonym` se la forma è anche un glifo del DTI del ricevente); un sottotipo non ancora noto produce la nota `IndexedReport::new_subtypes`, mai un errore; un token troncato o corrotto dà `MalformedLabel`. Ogni altro glifo ordinario (slot, valori `def`) deve essere una chiave valida, risolta con la policy, oppure, se inizia con una cifra, un numerale valido (`InvalidNumeral` altrimenti). Controlla anche `+UNION` e `+AGGL_NUM` (`MorphViolation`). I **simboli locali** sono accettati come chiavi valide senza esigere che siano nello snapshot: le chiavi dichiarate in un blocco `def` e usate nelle sue operazioni, i riferimenti R13 alla variabile (riconosciuti dallo Stadio 2 del ricevente) e gli id delle ancore (`anchor`/`anchorrel` e i successivi `rel`/`reloff`). Una chiave effimera in una posizione ordinaria resta invece una chiave sconosciuta.
- In ricezione le scelte glifo/etichetta del mittente sono già codificate nel testo: nessuna tendina è necessaria.
- `receive_indexed_input(&mut Session, id, dti)` esegue `validate_indexed_input`, registra i nuovi sottotipi nella sessione e marca come `Label` gli slot delle etichette ricevute; in caso di errore la sessione non cambia.

### 17.4 Viste
| Modalità | Contenuto |
|---|---|
| `HumanOriginal` | Struttura scritta con le forme `display` ("verde smeraldo") |
| `CanonicalLexical` | Struttura scritta con le forme primarie (`verdesmeraldo`) |
| `DtiIndexed` | Forma indicizzata (struttura scritta con le chiavi) |

Impaginazione: `Linear` su una riga, oppure `CStyle { indent }` con una clausola per riga indentata per profondità.

---

## 18. DTI Policy Manager (B6, P1–P5)

### 18.1 Formato `.awpolicy` (scritto sulle chiavi, quindi indipendente dalla lingua)
```text
#awpolicy 1
#universal <versione snapshot>
#default allow|deny
signal <KEY> ta,ka,deep
role <KEY> n:<K>,<K> a:<K> d:… g:… ab:…
threshold <KEY> cha|chu|col min:<u64> max:<u64>
qualifier <KEY> <K>,<K>
require <KEY> zato|quo
label deny | label allow:<K>,<K>
```

### 18.2 I 5 filtri (lookup deterministico O(log n) nel caso pessimo)
Le regole sono indicizzate in `BTreeMap`/`BTreeSet`: ogni ricerca costa **O(log n) nel caso pessimo garantito**, con ordine di iterazione deterministico e senza vulnerabilità HashDoS. Una tabella hash offriva O(1) solo ammortizzato (caso pessimo O(n)) e un'iterazione non deterministica: nei sistemi hard real-time conta il caso pessimo. Il confronto tra atomi internati (`Symbol`) resta O(1).

Tutte le regole sono riferite al **glifo della coppia del segnale** (P1), tranne `qualifier`, che vale per ogni percorso con quella radice.

| # | Filtro | Regola |
|---|---|---|
| 1 | Signal Mask | Teste di segnale ammesse. Per un secondario devono essere ammessi sia il secondario sia il primario. |
| 2 | Matrice dei ruoli | Chiavi ammesse per ogni caso base (`n`, `a`, `d`, `g`, `ab`) |
| 3 | Limiti metadati e soglie | Qualificatori `§` ammessi; numerale dell'Accusativo entro [min, max] con `cha`, `chu` o `col` (P5) |
| 4 | Obbligo di subordinata | Figlio con condizionale scritto (`zato`) o con secondario `quo` |
| 5 | Enhanced Security | Le etichette (v4.1) sono vietate, oppure ammesse solo nelle clausole il cui glifo del segnale è elencato |

- Con `#default deny` la policy è una lista bianca: un glifo senza regola `signal` è un comando non autorizzato e un caso base assente dalla sua regola `role` è un ruolo non autorizzato. Con `allow` limitano solo le regole scritte.
- Senza una regola `label`, le etichette sono vietate se la Enhanced Security è attiva o se il default è `deny` (violazione `PolicyRule::Label`).
- È un'etichetta uno slot marcato `Label` nella sessione oppure un token `LVA<XYZ><forma>` (per esempio il traffico filtrato dal Legacy Shield); ogni altro slot è un glifo, anche se la forma è censita come etichetta.
- I valori raggiunti tramite variabili e anafore vengono controllati sui **valori risolti**; un glifo non risolvibile nel DTI è una violazione.
- Un sema richiamato (`~~`) vale, in qualsiasi slot, come il **glifo del segnale** del sema antecedente (seguendo le eventuali catene di sema). Per esempio `: deep ~~` viene controllato sulla chiave del segnale richiamato e non può aggirare la Signal Mask.
- Le violazioni hanno la forma `PolicyViolation { rule, span }` (P4).

> [!IMPORTANT]
> **Nota di sicurezza nella grammatica — esecuzione in ambiente safe per le etichette non censite.**
> - Le etichette sono stringhe opache che viaggiano con il prefisso `LVA<SOTTOTIPO><forma>` (default `LVAAAA<forma>`).
> - Se un'etichetta non è censita correttamente nel DTI locale del nodo ricevente (nota diagnostica `UncensusedLabel`, oppure etichetta esterna ricevuta in modalità `label allow`), oppure presenta un sottotipo dinamico non ancora classificato (`LabelSubtype::Dynamic`), la grammatica prescrive che il runtime o sistema ospite la tratti ed esegua **esclusivamente all'interno di un ambiente safe isolato** (sandbox / contesto dati puramente passivo), **privo di privilegi di esecuzione diretta** su comandi di sistema o attuatori fisici.

---

## 19. Apricot Legacy Shield (B5)

`LegacyShield::filter(&mut self, input) -> Result<String, Vec<ShieldViolation>>`:
- è un filtro **senza I/O di rete**;
- applica Stadio 1, Stadio 2, Stadio 3 e la policy, con **Enhanced Security sempre attiva**;
- se blocca un input annulla tutte le modifiche alla propria sessione, perché l'ingestione è transazionale (`Session::ingest_checked`);
- se lo accetta riemette la **forma canonica ricostruita**, mai il testo originale.

Il *virtual patching* consiste nell'aggiornare solo `.awdti` e `.awpolicy`.

---

## 20. Ghost Protocol v2 (B4, G1–G7)

È un cifrario a flusso steganografico per mappatura semantica: un periodo indicizzato diventa un altro periodo GHLIP **valido allo Stadio 1 e allo Stadio 2**, che lo Stadio 2 risolve in modo identico (stessa struttura, stessi antecedenti, stesse variabili). La decifratura è esatta.

| Elemento | Specifica |
|---|---|
| Suite | `CryptoSuite { Ascon, Skein512, Blake3, Shake256 }`. **SHAKE256** (FIPS 202) è interno, senza dipendenze, verificato contro `hashlib`. Ascon, Skein-512 e BLAKE3 sono feature opzionali (`ghost-ascon`, `ghost-skein`, `ghost-blake3`); una suite non compilata dà `SuiteUnavailable`. |
| KDF | **Argon2id** (feature `ghost-argon2`). Default RFC 9106 *seconda raccomandazione* (t=3, p=4, m=64 MiB, sale 16 B, tag 32 B); disponibile anche la prima (t=1, p=4, m=2 GiB). `Shake256KeyDerivation` va usato solo con segreti ad alta entropia, non è un hash di password. |
| Keystream | XOF(etichetta, chiave master, nonce, versione dello snapshot, classe, posizione P), con campi a lunghezza prefissata. Ogni classe ha un contatore di posizione proprio. |
| Dominio (G4) | Chiavi non deprecate dello snapshot universale S, ordinate per byte, permutate con **Fisher-Yates** (estrazioni uniformi per rifiuto). |
| Numerali | Ogni cifra si cifra con una permutazione Fisher-Yates su `0..B-1`, dove B è la base di `#numeral base:B`, classe `numeral-digit`. Il cifrato è un numerale valido della stessa base e lunghezza. Senza `#numeral` → `NoNumeralSystem`; cifra ≥ B → `DigitOutOfBase`. |
| Chiavi effimere (F) | Le chiavi fuori dallo snapshot (K \ S: nomi `def`, atomi di id d'ancora e riferimenti R13 senza voce DTI abilitata) si cifrano con una **rete di Feistel a 10 round** sul corpo di 31 caratteri, in radice mista: metà L ∈ Z₂₆·₃₆¹⁵, metà R ∈ Z₃₆¹⁵, ciascuna in un `u128`. La funzione di round è il keystream della suite con classe `ephemeral-key`. Il carattere Luhn si ricalcola; il **cycle-walking** tiene ogni immagine fuori da S. In decifratura un'appartenenza O(1) a S sceglie l'inversa Fisher-Yates o l'inversa Feistel. Una chiave effimera ammessa solo in posizione legata (nome `def`, atomo di id d'ancora, riferimento R13); altrove → `UnboundEphemeralKey`. |
| Tabella dei simboli (regola 5, G1) | Corrispondenza **iniettiva** in chiaro ↔ cifrato, con conteggio dei riferimenti. Contiene gli atomi: del blocco in scrittura; dei blocchi visibili come antecedenti (stack DSP attivo, blocchi di livello 0 del periodo, blocchi conservati da `/^/`), con i percorsi dei valori delle loro coppie; delle variabili `def` vive (nome e valori, fino a `abse ~` o `^^/`); delle ancore aperte (atomi dell'id, fino a `end ~` o alla fine del periodo). Un ramo potato o la chiusura con `^/`/`^^/` rilasciano le voci. Due atomi con chiaro diverso non ricevono mai la stessa chiave cifrata finché sono visibili. |
| Biiezione per posizione | σ_P è un *chain walk* sulla permutazione di base τ_P (Fisher-Yates, cifre o Feistel), coerente con la tabella: σ(p_j) = c_j per le voci vive; altrimenti y = τ(x) e, finché y coincide con una cifra legata c_j, si prosegue con y = τ(p_j). È biiettiva e invertibile esattamente in al più \|B\|+1 passi; con tabella vuota σ = τ. Ne segue che nomi `def`, riferimenti R13, `abfi`/`absi`/`abse` e id di `anchor`/`rel`/`reloff` prendono automaticamente gli atomi cifrati di ciò a cui si riferiscono, e nessun altro atomo può catturare il riferimento. |
| Cosa si cifra (G5) | Tutti gli atomi chiave e i numerali della forma indicizzata. Restano invariati `~`, `~~`, `abse ~`, `end ~` e le tile `def` (mai permutate). Le etichette (`LVA…`, v4.1) restano **rifiutate** (`GhostError::Label`), per non esporre testo in chiaro. |
| `permute_grammar_tiles` (G2) | Permuta anche: il segnale primario solo nella classe {`ta`, `to`, `ka`, `ko`, `ot`} (`oh` resta fisso perché governa il DSP, §9.3); il tempo nelle due classi separate {omesso, `ato`, `alt`, `alv`, `att`, `atv`, `asu`} e {`zato`, `zalt`, `zalv`, `zatt`, `zatv`, `zasu`}; il numero delle tile di caso e il numero della tile di segnale (classe dei numeri: omesso, `u`, `m`, `num`). |
| `GhostSession` (C) | Un flusso ordinato tra due estremi. Ciascuno tiene: il contatore di sequenza, una `Session` in chiaro e una cifrata (Stadio 2 nel contesto dei periodi precedenti) e la tabella dei simboli. API: `GhostSession::new(params)`, `encrypt(plain, dti, key, nonce) -> GhostEnvelope`, `decrypt(&envelope, dti, key) -> String`. Ogni messaggio è atomico: in caso di errore lo stato resta invariato. Il mittente verifica che il cifrato sia valido allo Stadio 2 con risoluzioni identiche. Il chiaro deve essere valido allo Stadio 2 nel proprio contesto, altrimenti `Stage2 { diagnostics }`. `encrypt_period`/`decrypt_period` restano come involucri su una sessione nuova, validi solo per periodi autonomi. |
| Integrità (G3) | `GhostEnvelope { suite, snapshot_version, sequence, nonce, ciphertext, mac_tag }`. Il MAC copre sequenza, nonce e cifrato (più suite, versione e flag) e viaggia fuori banda. Confronto a tempo costante. |
| Anti-replay e ordine (G6) | Il ricevente accetta solo la sequenza successiva: inferiore → `Replay`, superiore → `OutOfOrder { expected, found }`. Il nonce di 128 bit resta fornito dal chiamante. La decifratura procede così: suite → versione → MAC → sequenza → decifrazione → registrazione. |

**Limite dichiarato:** finché sono nell'ambito di visibilità, atomi uguali in chiaro hanno lo stesso atomo cifrato. È il prezzo della validità allo Stadio 2 (decisione G1). Una volta rilasciati tornano indipendenti per posizione.

---

## 21. Editor (Fase D, H7)

### 21.1 Pre-processore (`preprocess_assisted_input`)
- **Commenti** `//`: iniziano solo all'inizio di un token o subito dopo un delimitatore di periodo completo (`^/// nota`). Dentro qualsiasi altro token `//` appartiene al token (v4.1: un glifo come `http://x` conserva i suoi `//`).
- **Slot Locking:** come nel parser, ogni token in uno slot pari è un glifo (delimitatori, `:`, graffe e `"` compresi), tranne `~` e `~~`.
- **Spazi liberi**, tab e a capo vengono ridotti a un unico spazio.
- **Graffe** `{ }`:
  - aprono i figli della clausola appena scritta;
  - ogni clausola interna riceve i `:` della propria profondità, e dopo una congiunzione si emettono la congiunzione e poi i `:`;
  - errori: `:` scritti a mano dentro le graffe, tile di portabilità dentro le graffe, graffe sbilanciate, connettore mancante a livello 0.
- **Treno del segnale con `_`**, solo nella tile di segnale: `ta_zato_cha` diventa `tachazato`, con riordino automatico; i duplicati restano visibili al parser.
- **Alias** `>` `<` `=` `!=` → `cha` `chu` `col` `cla`. Gli alias ambigui `~=`, `+` e `-` sono **rimossi** e producono una diagnostica.
- **Locuzioni con spazio** ("verde smeraldo"), con il DTI: si cerca la corrispondenza più lunga, purché nessuna parola della locuzione sia una tile. Esito ambiguo → errore; voce non abilitata → errore con il suggerimento `use <KEY>`.
- **Normalizzazione nella forma primaria.** È l'unico punto in cui si risolvono `syn:` e `display`, prima che il testo entri nello Stadio 1/2. Vale in modo uniforme in ogni posizione di glifo (slot relazionali, nomi e valori `def`, id di portabilità): prima sul token intero (priorità al DTI, anche con `§`), poi su ogni segmento `§`, così il nome `def` e i suoi riferimenti restano identici (R13). Gli slot marcati come etichetta non si normalizzano mai; quelli non marcati sono glifo e si normalizzano:
  - forma primaria (anche di una voce non abilitata) → invariata;
  - sinonimo o `display` di una sola voce abilitata → forma primaria, con la nota `NormalizedForm`;
  - sinonimo o `display` di una voce non abilitata → `DisabledEntry` con il suggerimento `use <KEY>`;
  - sinonimo condiviso da più voci → `AmbiguousForm`;
  - identificatore libero, del tutto assente dal DTI → invariato.
- **Selezioni Glifo/Etichetta (v4.1):** `preprocess_with_choices(input, settings, dti, &[SlotChoice { span, kind }])` riceve le selezioni dell'utente (intervallo di byte del token nell'input). Un token marcato `Label` resta una stringa inerte e il suo intervallo nel testo prodotto compare in `AssistOutput::labels`; dopo l'ingestione di quel testo, `apply_label_choices(&mut Session, &ids, &labels)` registra le scelte nella sessione. `preprocess_assisted_input` equivale a nessuna selezione (ogni slot è un glifo).
- **Controllo sul DTI di ogni glifo:** ogni token di uno slot pari ordinario si controlla sul DTI, anche quando lo Slot Locking ha reso glifo un delimitatore di periodo o una sequenza di `:`. Un token non censito produce la nota `UnknownGlyph { token, span, structural }` (`structural` per delimitatori e `:`, insieme alla mancata chiusura segnalata dalla sessione): l'editor lo evidenzia come non censito e chiede di censirlo nel DTI locale come glifo (`new`) o come etichetta (`label`). Un token marcato `Label` ma non censito come etichetta produce `UncensusedLabel`. `reclassify_as_label` è rimosso.
- **Selezione obbligatoria:** per un token non marcato che il DTI locale contiene come etichetta, la nota `Label` (solo etichetta) o `GlyphOrLabel` (glifo ed etichetta) chiede all'utente la selezione dalla tendina (§21.3).
- **Flag `morph`** (con il DTI): un glifo che viola `+UNION`, `+AGGL`, `+CONTR` o `+AGGL_NUM` produce la diagnostica `MorphViolation { form, key, flag }`, con le stesse regole dello Stadio 3 (§17.1).
- Il treno del segnale riconosce anche il numero (`ta_u_zato` → `tazatou`), che va sempre in coda.

### 21.2 `settings.toml`
```toml
[editor]
braces = true
comments = true
signal_underscore = true
symbol_aliases = true
multiword = true
autocomplete = "auto"          # oppure "manual" (v4.1)

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

[warnings]                      # v4.1, a proprio rischio
ERR_SYNTAX = "error"
ERR_ORDER = "error"
ERR_REFERENCE = "error"
ERR_SEMANTIC = "error"
ERR_SCOPE = "error"
ERR_CASE_AMBIGUITY = "error"
```
- `ShortcutResolver` risolve `Alt+C` seguito dal simbolo nella sillaba canonica. `@live_variables` restituisce l'elenco ordinato delle variabili `def` vive della sessione; ogni altra stringa è un inserimento letterale.
- `autocomplete`: `"auto"` apre la tendina dei suggerimenti sotto il testo durante la digitazione; `"manual"` la apre solo su richiesta, con l'azione `suggest` (`Alt+Down`).
- `[shortcuts.actions]`: tasti delle azioni dell'editor; i tasti accettano anche `Down`. `copy_relative` può coincidere con il prefisso dei comparativi, perché i due contesti sono disgiunti (§21.4). Una scorciatoia personalizzata uguale al prefisso o a un'azione dà `ChordConflict`.
- `[warnings]`: ogni codice si imposta singolarmente a `"warning"` o `"error"` (§15.1); un codice non elencato resta un errore. Il valore (`EditorSettings::warnings`) va passato a `ParserConfig::warnings`.

### 21.3 Tendina dei suggerimenti (v4.1)
- `suggestions(prefisso, &Session, Option<&Dti>) -> Vec<Suggestion { kind, text }>`, con `SuggestionKind::{Glyph, Label, Sema}` e al massimo `MAX_SUGGESTIONS` voci, senza duplicati. Ordine: glifo della sessione dal più recente (nomi `def` compresi), etichette del DTI locale, sema della sessione dal più recente il cui testo canonico o glifo del segnale inizia con il prefisso.
- Selezionando una voce l'editor inserisce `text`: il glifo o l'etichetta così come sono, il sema come **testo canonico dell'intera clausola** (`ta vedere nu io`).
- **Disambiguazione Glifo/Etichetta (obbligatoria):** per un valore che il DTI locale contiene come etichetta la tendina mostra le voci ammesse (entrambe se è anche un glifo) e l'utente deve sceglierne una per scriverlo; se la sessione ha già una scelta per uno slot con quel valore, la voce scelta viene prima. La scelta passa al pre-processore come `SlotChoice` e alla sessione con `apply_label_choices` / `Session::set_slot_kind`.
- Attivazione: automatica (`autocomplete = "auto"`) oppure con `Alt+Down` (`ShortcutOutcome::Suggest`).

### 21.4 Copia con ancore (v4.1)
- `copy_subordinates(testo, selezione, CopyMode, Option<id>) -> CopiedBlock { text, id }` copia le subordinate selezionate (intervallo di byte di clausole intere di testo canonico; la prima introdotta da `:`, le altre anche da una congiunzione). Lo Slot Locking è rispettato: un `:` in uno slot pari è un glifo.
- `CopyMode::Absolute` (`Ctrl+C`): `<:…> anchor <id> <clausole> <:…> end ~`, con i `:` delle clausole come scritti.
- `CopyMode::Relative` (`Alt+C` con selezione): `<:…> anchorrel <id> <clausole> : end ~`, con i `:` riscritti rispetto alla clausola meno profonda, che riceve un solo `:`.
- L'ancora riceve i `:` della clausola meno profonda, quindi il blocco incollato sul posto descrive lo stesso albero. Se l'id è omesso si genera il primo `copy<N>` assente dal testo.
- Errori `CopyError`: `EmptySelection`, `InvalidSelection`, `NotSubordinate`, `IncompleteClause`, `InvalidId`.
- **Contesti disgiunti:** `ShortcutResolver::resolve_in(sessione, ShortcutContext { selection }, sequenza)`. Con una selezione attiva `Alt+C` → `CopyRelative` e `Ctrl+C` → `CopyAbsolute`; senza selezione, durante la digitazione, `Alt+C` apre l'inserimento dei comparativi (`> < = ! ~ + -`) e `Ctrl+C` non è un'azione dell'editor.

---

## 22. Functions (aggancio per Apricot Logic)

`FunctionEvaluator` e `Session::evaluate_and_ingest`:
- l'output della funzione è testo ApriWave e viene reingerito in modo atomico;
- gli operandi multipli si scrivono con `absi`, **mai** con `§`;
- il protocollo non contiene funzioni aritmetiche.

---

## 23. Architettura dei crate, export e CLI

Workspace Cargo con due crate:

| Crate | Ambiente | Contenuto |
|---|---|---|
| `apriwave-core` | `#![no_std]` + `alloc`, zero dipendenze esterne. Compila per microcontrollori bare-metal (`thumbv6m-none-eabi` per Cortex-M0+, `thumbv7em-none-eabihf` per Cortex-M4F) con un allocatore globale | lexer, parser, Stadio 2, printer, Stadio 3 (forma indicizzata ed espansa), modello DTI e `.dti.bin` su buffer, `DtiKey::session_symbol` con SHAKE256 interno, functions, piano, policy, Legacy Shield |
| `apriwave` | `std` | Riesporta tutto il core con gli stessi percorsi; aggiunge l'API su file di `.dti.bin`, l'editor, l'export JSON, la CLI e Ghost (congelato) |

Mappe e insiemi del core sono `BTreeMap`/`BTreeSet`: iterazione deterministica e ricerche O(log n) nel caso pessimo.

**Memoria (microcontrollori < 50 KB di RAM):**
- **Interner a copia singola:** ogni testo è allocato una sola volta (`Vec<Box<str>>`, risoluzione di un `Symbol` in O(1)). L'indice è un `Vec<Symbol>` ordinato per testo: ricerca binaria O(log n) nel caso pessimo, inserimento con spostamento di al più n simboli da 4 byte. Non usa `Arc`/`Rc`, quindi la `Session` resta `Send + Sync` anche su Cortex-M0+, che non ha atomici di confronto-e-scambio. Il rollback dell'ingestione tronca i testi e rimuove dall'indice i simboli successivi al punto di ripristino.
- **`Session::compact()`:** libera periodi, analisi, binding e testi internati, mantenendo la configurazione. È ammesso solo su una sessione vuota o il cui ultimo periodo è chiuso da `^^/`, perché quella chiusura ha già cancellato anafore, variabili `def` e sincronia (§3): l'ingestione successiva si comporta come senza compattazione. Altrimenti restituisce `CompactError::NotHardClosed`. Gli identificatori ottenuti prima (`PeriodId`, `BindingId`, `Symbol`) diventano invalidi.
- **Stack:** le catene di anafore, di variabili e di sema sono risolte con cicli iterativi. Le sole ricorsioni del core sono:
  - la visita dell'albero DSP nel piano di esecuzione, con profondità limitata da `max_dsp_depth` (default 64, configurabile per stack ridotti);
  - gli operandi di variabile negli insiemi e nella policy, al massimo 2–3 livelli.
  
  Printer, forma espansa e forma indicizzata non sono ricorsivi.

| Strumento | Descrizione |
|---|---|
| `export::export_analyzed_json` | JSON deterministico: clausole, profondità, albero, congiunzione di ogni clausola (nessun albero di coordinazione), segnale effettivo con i flag di ereditarietà (quantificatore, `ref`, comparativo, tempo, numero), coppie con risoluzioni, numeri e scelta glifo/etichetta (`slot_kind`), warning del periodo |
| CLI | `apriwave-cli [--ast] [--json] [--warn=<CODICE>]… "<periodo>"`: `--warn` declassa un codice a warning; i warning si stampano come `WARN` |

---

## 24. Note di implementazione (interpretazioni adottate)

| # | Nota |
|---|---|
| I1–I11 | Come approvate per lo Stadio 2 (I2 e I3 superate dalla simmetria delle coppie, §11.3) |
| I12 | Dopo `abdur` il solo `~` è un marcatore; un altro glifo viene risolto normalmente, ma non conta per il ciclo |
| I13 | Il marcatore `abdur` richiede un condizionale **scritto o ereditato** (v4.1) e profondità ≥ 1 |
| I14 | I bersagli insiemistici esistono solo con un quantificatore o con `cup`/`cua`/`cuo`. Base = coppie in Accusativo; senza Accusativi, coppie in Nominativo (§14). Senza quantificatore la modalità è `All`. |
| I15 | Il limite dei cicli conta i passi ripetuti oltre la prima occorrenza di ogni clausola |
| I16 | Policy: l'header `#default allow|deny` è obbligatorio; `require … zato` accetta qualsiasi condizionale scritto |
| I17 | Ghost: per Ascon e Skein il MAC usa la XOF della suite con chiave prefissata; il flag `permute_grammar_tiles` è vincolato nel MAC; massimo 128 rifiuti per estrazione |
| I18 | Editor: le scelte puntuali sono documentate nel modulo `editor` (profondità delle graffe, treni malformati lasciati al parser, scorciatoie con Shift tollerato dopo il prefisso) |
| I19 | Regola 1B: un'anafora eredita il **numero effettivo** della coppia antecedente; se l'antecedente è la coppia del segnale, quel numero è quello scritto o ereditato dal segnale, altrimenti Duale |
| I20 | Nomi `def` e id di ancora sono simboli di sessione del token intero: chiave DTI se forma primaria di una voce abilitata, altrimenti chiave effimera (decisioni D1 e D2); al ricevente sono accettati come simboli locali |
| I21 | Sinonimi e `display` esistono solo nell'editor: lo Stadio 3 conosce solo le forme primarie, univoche per lingua, quindi mittente lessicale e ricevente indicizzato risolvono le anafore in modo identico |
| I22 | `morph`: un record con `base:` e `param:` e forma atomica è un'agglutinazione se la forma è la concatenazione delle forme primarie (locali o di interscambio) dei componenti, altrimenti una contrazione; i flag si verificano sui componenti, come `+UNION` sui segmenti e `+AGGL_NUM` sulle cifre |
| I23 | `~~` ha comportamento identico in tutti gli slot pari: nessuna fusione di coppie, nessun trattamento speciale dello slot 2 (simmetria degli slot, §1.5) |
| I24 | Ghost: cifrato valido allo Stadio 2 tramite la tabella iniettiva dei simboli visibili e la biiezione *chain walk* per posizione (decisioni B, C, F, G1, G2); le chiavi effimere usano Feistel con cycle-walking fuori dallo snapshot |
| I25 | v4.1 (rettifica): la selezione Glifo/Etichetta è obbligatoria in scrittura; ogni slot è deterministico (non marcato = glifo); `IndexError::GlyphOrLabel` è rimosso; la scelta esplicita «etichetta» vale anche per una forma non censita (con la nota `UncensusedLabel`) |
| I26 | v4.1 (rettifica): un sottotipo `[A-Z]{3}` diverso da `AAA` è un'etichetta valida, registrata dinamicamente nella sessione con una nota informativa e conservata nella ritrasmissione; `UnknownLabelType` è rimosso; solo i token troncati o corrotti danno `MalformedLabel` |
| I27 | v4.1: formato `.dti.bin` versione 2 con il tipo di record `label` = 2; gli avvisi di censimento non fanno parte dell'uguaglianza del modello |
| I28 | v4.1: errori strutturali non declassabili = tutti i casi ERR_SYNTAX ed ERR_ORDER, `DefWithoutOperations`, `PortabilityOutsideSubordination`, `LimitExceeded`, `DspLevelSkip`, `EndWithoutAnchor`, `UnclosedAnchor`, `CaseTileWithoutGlyph` |
| I29 | v4.1: per lo Slot Locking un glifo manca solo a fine input; un delimitatore in uno slot pari è un glifo e lascia il periodo aperto, ed è sempre controllato anche sul DTI (doppio controllo: `MissingPeriodClose` + `UnknownGlyph { structural: true }`) |
| I31 | v4.1: ritrasmissione: glifo già indicizzati (chiavi abilitate, numerali) e simboli di sessione che sono già chiavi restano invariati nella forma indicizzata |
| I30 | v4.1 editor: commento solo a inizio token o dopo un delimitatore; suggerimenti limitati a `MAX_SUGGESTIONS`; id di copia generati come `copy<N>` |
| Limiti | Ascon e Skein senza vettori di test noti (solo test di coerenza); i parametri di default di Argon2id non sono esercitati nei test (64 MiB). |

