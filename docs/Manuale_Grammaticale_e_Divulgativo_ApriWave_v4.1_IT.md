# Manuale Grammaticale e Divulgativo — ApriWave · Protocollo GHLIP v4.1

**Edizione del 15/11/2026, versione 4.1.** Questo manuale spiega la grammatica del protocollo GHLIP e l'architettura del software ApriWave a un lettore esterno, senza presupporre la conoscenza dei documenti di lavoro. Ogni regola segue lo stesso schema:

1. **Il problema**: quale ambiguità o difficoltà di comunicazione (tra specie diverse, tra persona e macchina, tra macchine) si vuole eliminare;
2. **La soluzione**: che cosa ha scelto il protocollo e perché non lascia eccezioni;
3. **La regola**: l'enunciato formale e completo, con tabelle;
4. **L'esempio**: uno o due casi realistici, spiegati token per token.

Il riferimento normativo resta il Manuale Tecnico (`Manuale_Tecnico_ApriWave_v4.1.md`); questo testo ne è la spiegazione ragionata. In caso di dubbio prevale il Manuale Tecnico.

---

## Indice

1. Che cos'è GHLIP e come si chiama ogni cosa
2. Scrittura: uno spazio, tile e glifo, Slot Locking
3. Il periodo e le tre chiusure
4. I tre tipi di blocco
5. La tile di segnale e i determinanti
6. Le tile di caso
7. Il numero quantistico
8. Gli slot dei glifo: glifo, qualificatore `§`, etichette, anafore
9. Connettori e albero di profondità
10. Ereditarietà nelle subordinate
11. Le anafore `~` e `~~`
12. Le variabili `def`
13. I cicli `abdur` e il piano di esecuzione
14. Gli insiemi strutturali
15. La diagnostica e i warning
16. Il dizionario DTI, le chiavi e i numerali
17. Le etichette
18. Lo Stadio 3: forma espansa, forma indicizzata, viste
19. Il gestore delle policy
20. Il Legacy Shield
21. Il Ghost Protocol
22. L'editor assistito
23. Il contratto con Apricot Logic
24. Architettura del software
25. Glossario

---

## 1. Che cos'è GHLIP e come si chiama ogni cosa

### 1.1 Il problema
Due interlocutori che non condividono né una lingua né una cultura (due specie, una persona e un apparato, due macchine di costruttori diversi) devono scambiarsi frasi che ciascuno legga **in un solo modo**. Le lingue naturali sono piene di ambiguità: lo stesso ordine di parole può avere più strutture, una parola può essere verbo o nome, la punteggiatura cambia il senso. Inoltre un sistema che «capisce» il significato delle parole porta con sé i pregiudizi di chi l'ha costruito.

### 1.2 La soluzione
GHLIP separa in modo netto **la struttura** dal **contenuto**:
- la struttura è data da un insieme chiuso e immutabile di strumenti grammaticali, le **tile**;
- il contenuto è dato dai **glifo**, che il protocollo non interpreta mai: sono voci di un dizionario esterno, il **DTI**.

La grammatica è un motore meccanico, deterministico e neutrale: non conosce né giudica il contenuto dei glifo e guida l'utente senza imporgli scelte o divieti arbitrari.

### 1.3 La regola: i nomi

| Livello | Progetto / contenitore | Protocollo / programma | Ruolo |
|---|---|---|---|
| **Livello 0** — strutturale e relazionale | **ApriWave** (*Apricot Wave*): l'ecosistema, il repository, i crate Rust `apriwave-core` e `apriwave` | **GHLIP** (*Ghlip*) | Grammatica deterministica: struttura, riferimenti, piano di esecuzione |
| **Livello 1** — computazionale e inferenziale | **Apricot Logic** (progetto gemello e separato) | **LEMMA** | Motore logico che ragiona sui periodi già validati da GHLIP |

**GHLIP** è l'acronimo di *Grammatical Hierarchical Linear Inter-species Protocol*:

| Lettera | Significato | In pratica |
|---|---|---|
| G — Grammatical | Telaio chiuso delle tile grammaticali universali | Le tile non cambiano mai; il vocabolario sta nel DTI |
| H — Hierarchical | Gerarchia ad albero senza tag di chiusura | La profondità si scrive con `:`, `::`, `:::` |
| L — Linear | Scansione lineare deterministica | Una tile, poi un glifo, poi una tile… (Slot Locking) |
| I — Inter-species | Nessun presupposto culturale o antropocentrico | Anche la base dei numeri si dichiara (capitolo 16) |
| P — Protocol | Protocollo puro | Il calcolo appartiene al Livello 1 |

Il termine **glifo** (in inglese *glyph*; in italiano invariabile: *i glifo*) indica solo il singolo elemento censito nel DTI. La triade fondativa è **Tile + Glifo = Sema**: un *sema* è un blocco di significato completo. Anche *sema* è invariabile.

Tre principi accompagnano tutto il manuale:

| Principio | Enunciato |
|---|---|
| Regola aurea | Il protocollo governa solo la grammatica delle tile. Il numero di ripetizioni di un ciclo si legge solo dalla tile `abdur=<N>`; gli insiemi si calcolano solo sulle relazioni strutturali del DTI |
| Determinismo | A ogni stringa valida corrisponde una sola struttura |
| Motore auto-contenuto | Cicli e insiemi si risolvono internamente in un **piano di esecuzione** validato; all'apparato ospite resta solo l'effetto fisico |

### 1.4 Esempio
`/^ ta vedere nu io au canis ^/`

| Token | Tipo | Significato |
|---|---|---|
| `/^` | tile (apertura) | Inizia un periodo |
| `ta` | tile di segnale | Affermazione |
| `vedere` | glifo | Il concetto «vedere» (dal DTI) |
| `nu` | tile di caso | Nominativo, Singolare |
| `io` | glifo | Il concetto «io» |
| `au` | tile di caso | Accusativo, Singolare |
| `canis` | glifo | Il concetto «cane» |
| `^/` | tile (chiusura) | Chiude il periodo |

Lettura: «io vedo un cane». Il protocollo non sa che cosa siano un cane o la vista: sa solo che `io` è il soggetto e `canis` l'oggetto della relazione `vedere`.

---

## 2. Scrittura: uno spazio, tile e glifo, Slot Locking

### 2.1 Il problema
Se una frase può contenere spazi multipli, a capo, simboli con doppio ruolo o caratteri invisibili, due lettori possono suddividerla in modo diverso. Peggio: un carattere invisibile (per esempio un segno che inverte la direzione del testo) può far *apparire* un comando diverso da quello reale.

### 2.2 La soluzione
- Un solo separatore: **uno spazio ASCII**.
- **Slot Locking** (`1 Tile -> 1 Glifo`): le posizioni si alternano in modo rigido. Le posizioni dispari contengono sempre una tile, quelle pari sempre il suo contenuto. Che un token sia una tile o un contenuto dipende **solo dalla posizione**, mai dal suo aspetto.
- Libertà totale dei glifo: poiché la posizione basta a riconoscere un glifo, un glifo può contenere **qualsiasi carattere visibile**, anche quelli che altrove sono tile. Sono vietati solo i caratteri invisibili.
- **Regola unica del ruolo dello slot pari:** qualsiasi sequenza di caratteri visibili (`canis`, `roma`, `^/`, `:`, `§`, `~`, `~~`…) può essere censita nel DTI come glifo o nel DTI locale come etichetta. Il ruolo del token nello slot pari — **Glifo**, **Etichetta** o **Anafora** — è fissato dall'**annotazione dello slot**, che l'utente sceglie da una tendina tra i ruoli ammessi dal DTI. Uno slot senza annotazione ha il ruolo dato dalla posizione: Anafora per i token esatti `~` e `~~`, Glifo per ogni altro token. Non esistono casi speciali: lo stesso meccanismo distingue un glifo da un'etichetta che si scrive allo stesso modo e un'anafora da un glifo o da un'etichetta scritti `~` o `~~`.

### 2.3 La regola

| Elemento | Regola |
|---|---|
| Separatore | Esattamente uno spazio ASCII tra due token. Spazi doppi, iniziali o finali, tabulazioni e a capo sono errori di sintassi. La scrittura libera è ammessa solo nell'editor assistito (capitolo 22), che la normalizza |
| Tile | Insieme chiuso; solo minuscole; vietati `.`, `|`, `(`, `)` |
| Glifo | Qualsiasi carattere visibile: ogni alfabeto, maiuscole, cifre, punteggiatura, simboli (`-`, `_`, `"`, `§`, `:`, `/`, `^`, `~`…) |
| Caratteri vietati nei glifo | Solo gli invisibili: caratteri di controllo e caratteri di formato o di direzione del testo (U+061C, U+200B–U+200F, U+202A–U+202E, U+2060–U+2064, U+2066–U+2069, U+FEFF) |
| Slot pari | Ospita **qualsiasi token** di caratteri visibili: anche `/^`, `^/`, `/^/`, `^^/`, `:`, `::`, i nomi delle tile (`a`, `ta`, `y`…), `"`, `§`, `~` e `~~` |
| Ruolo del token nello slot pari | Slot annotato **Glifo** → glifo; slot annotato **Etichetta** → etichetta; slot non annotato → ruolo dato dalla posizione: **Anafora** per i token esatti `~` e `~~` (capitolo 11), **Glifo** per ogni altro token. La regola è la stessa per ogni token |

**Perché nessun token crea ambiguità.** Il ruolo di ogni slot pari (Glifo, Etichetta o Anafora) è fissato dall'annotazione, scelta nell'editor da una tendina, che accompagna il testo all'ingestione. In visualizzazione ogni ruolo ha il suo colore; in trasmissione tra macchine la differenza è visibile nel testo stesso: un glifo diventa una chiave di 32 caratteri minuscoli, un'etichetta diventa `LVAAAA` seguito dalla sua forma (per esempio `LVAAAA^/`, `LVAAAA~`, `LVAAAA~~`), un'anafora viaggia come `~` o `~~`. Lo stesso vale per qualsiasi token: `^/` annotato Etichetta viaggia come `LVAAAA^/` esattamente come `~~` annotato Etichetta viaggia come `LVAAAA~~`.

**Il glifo mancante e il doppio controllo.** Poiché lo slot pari accetta qualsiasi token, un glifo «manca» solo alla fine del testo. Se l'utente scrive `/^ ta vedere nu ^/` dimenticando il glifo dopo `nu`, il token `^/` diventa il glifo di `nu` e il periodo resta aperto. Scattano allora **due controlli indipendenti**:

| Controllo | Che cosa vede | Esito |
|---|---|---|
| Sintattico | Il periodo non ha chiusura | Errore «periodo non chiuso» |
| Dizionario | Il glifo `^/` non è censito nel DTI | Nota «glifo non censito, di tipo strutturale»: l'editor lo evidenzia e chiede di censirlo come Glifo o come Etichetta, oppure di correggere il testo |

### 2.4 Esempi
**Esempio 1 — un delimitatore usato come glifo.** In un lessico tecnico il segno `^/` può essere il nome di un connettore fisico. `/^ ta collegare nu tecnico au ^/ ^/`:

| Posizione | Token | Ruolo per posizione |
|---|---|---|
| 1 | `ta` | tile di segnale |
| 2 | `collegare` | glifo |
| 3 | `nu` | tile di caso |
| 4 | `tecnico` | glifo |
| 5 | `au` | tile di caso |
| 6 | `^/` | **glifo** (posizione pari) |
| 7 | `^/` | tile di chiusura (posizione dispari) |

**Esempio 2 — glifo di un'altra scrittura.** `/^ ta visitare nu io au 東京 ^/`: `東京` è un glifo valido; sarà il DTI a dire a quale concetto corrisponde.

---

## 3. Il periodo e le tre chiusure

### 3.1 Il problema
Una conversazione è fatta di frasi collegate: alcune devono ricordare ciò che è stato detto (per riprenderlo con un'anafora), altre devono «ripartire da zero» per sicurezza. Serve un modo esplicito per dire quanta memoria conservare.

### 3.2 La soluzione
Il periodo si apre e si chiude con tile dedicate. Le **tre chiusure** stabiliscono che cosa sopravvive al periodo; due **aperture sincronizzate** legano il periodo al precedente.

### 3.3 La regola

| Apertura | Significato | Condizione |
|---|---|---|
| `/^` | Inizio di un periodo | — |
| `/^≈` | Simultaneità stretta con il periodo precedente | Serve un periodo precedente non chiuso da `^^/` |
| `/^≃` | Prossimità logica con il periodo precedente | Come sopra |

| Chiusura | Riferimenti di livello 0 (per le anafore) | Variabili `def` | Sincronia |
|---|---|---|---|
| `/^/` morbida | **conservati** e accumulati | conservate | conservata |
| `^/` standard | cancellati | conservate | conservata |
| `^^/` forte | cancellati | **cancellate** | **cancellata** |

Più periodi si scrivono uno dopo l'altro separati da un solo spazio.

### 3.4 Esempio
`/^ ta x nu a /^/ /^ ta z au ~ ^/`
- Il primo periodo afferma `x` con soggetto `a` e si chiude con `/^/`: i suoi blocchi di livello 0 restano visibili.
- Il secondo periodo usa `au ~`: l'anafora `~` ritrova `a` nel periodo precedente.
- Con `^/` al posto di `/^/`, `~` non troverebbe nulla e si avrebbe un errore di riferimento.

---

## 4. I tre tipi di blocco

### 4.1 Il problema
Un periodo deve poter contenere affermazioni, definizioni di nomi da riusare e indicazioni di struttura (dove inserire un gruppo di frasi). Mescolarle senza distinzione renderebbe la lettura ambigua.

### 4.2 La soluzione
Ogni blocco ha una forma riconoscibile dalla sua prima tile.

### 4.3 La regola

| Blocco | Forma | Capitolo |
|---|---|---|
| Relazionale | `[segnale+determinanti] [glifo] { [caso] [glifo] }` | 5–8 |
| Definizione | `[def…] [nome] [operazione] [valore] { [operazione] [valore] }` | 12 |
| Portabilità | `anchor` / `anchorrel` / `rel` / `reloff` `<glifo>`, oppure `end ~`, solo dopo `:` | 9 |

Il primo blocco del periodo non ha connettore e non può essere di portabilità.

### 4.4 Esempio
`/^ defu dev a sensore y ta leggere nu dev ^/`: un blocco di definizione (`defu dev a sensore`), il connettore `y`, un blocco relazionale (`ta leggere nu dev`).

---

## 5. La tile di segnale e i determinanti

### 5.1 Il problema
Ogni blocco deve dire **che tipo di atto** compie (affermare, negare, chiedere…) e con quali sfumature (tempo, quantità, confronto), senza parole aggiuntive che potrebbero essere scambiate per contenuto.

### 5.2 La soluzione
Tutte le informazioni sull'atto stanno in **una sola tile**, la tile di segnale: una testa seguita da determinanti agglutinati in un **ordine fisso**. L'ordine fisso rende la lettura univoca.

### 5.3 La regola: le teste

| Testa | Significato |
|---|---|
| `ta` | Affermazione (predefinita) |
| `to` | Negazione |
| `ka` | Domanda |
| `ko` | Domanda negativa |
| `ot` | Possibilità |
| `oh` | Vocativo; colloca il blocco come fratello dell'ultimo blocco attivo (capitolo 9) |
| `qua`, `que`, `quo`, `quu` | Segnali secondari (indiretta, ipotetica, impositiva, indeterminata), ammessi solo nelle subordinate e sempre seguiti da una testa primaria: `quata`, `quotazato` |
| `deep` | Canale macchina-macchina; colloca il blocco al livello 0 se non ci sono `:`. Accetta determinanti (`deepall`), mai una testa primaria |
| `def`, `defu`, `defm`, `defnum` | Definizione di variabile; accetta solo il numero |

### 5.4 La regola: i determinanti

Ordine fisso: `[testa][quantificatore][ref][comparativo][temporale | condizionale][numero]`

| Classe | Codici |
|---|---|
| Quantificatore | `all` (per ogni), `some` (esiste) |
| Riflessivo | `ref` |
| Comparativo | `cha` maggiore, `chu` minore, `cla` diverso, `col` uguale, `cal` simile, `cua` incluso, `cuo` escluso, `cup` unito |
| Temporale | `ato` presente (predefinito), `alt` passato remoto, `alv` futuro remoto, `att` passato prossimo, `atv` futuro prossimo, `asu` tempo indeterminato |
| Condizionale | `z` + temporale: `zato`, `zalt`, `zalv`, `zatt`, `zatv`, `zasu` |
| Numero | `u`, `m`, `num`, sempre in coda (capitolo 7) |

- Un determinante fuori ordine o ripetuto è un errore d'ordine; temporale e condizionale occupano la stessa casella e non possono stare insieme.
- **Riflessivo `ref`**: indica che l'azione ricade sul soggetto. L'oggetto diretto non è obbligatorio (riflessivo diretto) ma è pienamente ammesso (riflessivo indiretto o transitivo).
- **`ref` e comparativi** si combinano liberamente: `tarefcha` è valido.

### 5.5 Esempi
**Esempio 1.** `tachazatou` si scompone in `ta` (affermazione) + `cha` (maggiore) + `zato` (condizionale presente) + `u` (Singolare): «se è maggiore…», con la coppia del segnale al Singolare.

**Esempio 2 — riflessivo transitivo.** `/^ taref aggiornare nu nodo a firmware ^/`: «il nodo si aggiorna il firmware». `ref` dice che l'azione ricade sul nodo; l'Accusativo `a firmware` ne indica l'oggetto.

---

## 6. Le tile di caso

### 6.1 Il problema
In una lingua naturale il ruolo di una parola (chi fa, che cosa, a chi, con che cosa) dipende dall'ordine o da preposizioni ambigue. Serve un modo esplicito e chiuso per dichiarare il ruolo di ogni glifo.

### 6.2 La soluzione
Ogni glifo di un blocco relazionale è preceduto da una **tile di caso** che ne dichiara il ruolo. Le tile sono un elenco chiuso. Lo stesso caso può ripetersi, e l'ordine scritto si conserva.

### 6.3 La regola

Forma: `[caso][determinante][numero]` e, solo per il ciclo, `=<N>`.

| Tile | Caso | Uso |
|---|---|---|
| `n` | Nominativo | soggetto |
| `a` | Accusativo | oggetto; oggetto di un confronto |
| `g` / `gpa` | Genitivo / genitivo partitivo | appartenenza |
| `d` / `dad` | Dativo / allativo | destinatario / moto verso |
| `ab` | Ablativo generico | provenienza, mezzo |
| `abef` | Elativo | moto da dentro |
| `aben` | Inessivo | stato in luogo |
| `abel` | Illativo | moto verso dentro |
| `abse` | Abessivo | «senza» |
| `abfi` | Translativo | trasformazione in |
| `abcu` | Istruttivo | per mezzo di |
| `absi` | Comitativo | «insieme a» |
| `abdu` | Temporale continuativo | «mentre, durante» (mai un ciclo) |
| `abpa` | Partitivo | parte di |
| `abdur=<N>` | Operatore di ciclo | ripeti N volte (`N` da 1 in su, senza zeri iniziali) |

- **Numero** in coda alla tile: omesso (Duale), `u` Singolare, `m` Plurale, `num` Non Duale (`nu`, `am`, `abenu`, `abduru=5`).
- **Casi ripetuti**: lo stesso caso può comparire più volte nello stesso blocco.
- **Ordine canonico stabile**: internamente le coppie si ordinano come segnale → `n` → `a` → `d` → `g` → `ab`; a parità di caso si conserva **l'ordine scritto dall'utente**.

### 6.4 Esempio
`/^ ta inviare nu centrale du nodo1 du nodo2 au avviso ^/`

| Coppia | Caso | Ordine canonico |
|---|---|---|
| `nu centrale` | Nominativo | 1° dopo il segnale |
| `du nodo1` | Dativo | 3° |
| `du nodo2` | Dativo | 4° (dopo `nodo1`, come scritto) |
| `au avviso` | Accusativo | 2° |

---

## 7. Il numero quantistico

### 7.1 Il problema
Molti sistemi danno per scontato che una quantità sia «uno» o «molti». Ma un osservatore può sapere che *c'è* una quantità senza saperla ancora classificare, o non sapere nulla.

### 7.2 La soluzione
Quattro stati, ispirati alla sovrapposizione: lo stato predefinito (nessun marcatore) è l'indeterminazione; i marcatori la fanno «collassare».

### 7.3 La regola

| Stato | Marcatore | Significato |
|---|---|---|
| Duale | omesso | Sovrapposizione: indeterminato prima dell'osservazione |
| Singolare | `u` | Collasso sull'unità |
| Plurale | `m` | Collasso sulla molteplicità |
| Non Duale | `num` | Quantità rilevata ma non ancora classificata |

Ogni coppia ha un **numero effettivo**:

| Coppia | Numero scritto | Numero omesso |
|---|---|---|
| Coppia del segnale | prevale | In una subordinata eredita il numero del segnale del padre; altrimenti Duale |
| Coppia di caso | prevale | Duale; davanti a `~` eredita il numero della coppia richiamata; davanti a una variabile `def` prende il numero della variabile |

### 7.4 Esempio
`/^ tau misurare nu sonda : ta registrare au ~ ^/`: la subordinata `ta registrare` non scrive il numero e **eredita** il Singolare del padre `tau`. La coppia `au ~` scrive il Singolare esplicitamente.

---

## 8. Gli slot dei glifo: glifo, qualificatore `§`, etichette, anafore

### 8.1 Il problema
Un concetto può aver bisogno di precisazioni («video, rosso»), ma una precisazione non deve mai essere scambiata per un concetto nuovo, e viceversa un nome composto già censito non deve essere spezzato.

### 8.2 La soluzione
Il segno `§` unisce glifo distinti del DTI; ma se l'intero token (anche contenente `§`) è censito come glifo unico, **il dizionario ha priorità**.

### 8.3 La regola

| Forma nello slot pari | Esempio | Significato |
|---|---|---|
| Glifo | `canis`, `Roma`, `x-y`, `30` | Un token qualsiasi di caratteri visibili |
| Glifo qualificato | `video§rosso` | Se `video§rosso` non è censito come unico glifo: `video` qualificato da `rosso` (ogni concetto unito deve permetterlo, capitolo 16) |
| Glifo unico con `§` | `rosso§vino` censito | Un solo concetto, una sola chiave |
| Etichetta | `Roma` annotato Etichetta | Stringa inerte censita nel DTI locale (capitolo 17) |
| Anafora di glifo | `~` non annotato | Riprende l'ultimo glifo del blocco antecedente (capitolo 11) |
| Anafora di sema | `~~` non annotato | Riprende l'intero blocco antecedente |

Un segmento vuoto (`x§`, `§x`) è ammesso dalla sintassi ma, risolto per segmenti, è una forma sconosciuta. Le vecchie forme del tipo `video~` o `vedere~~` non sono anafore: sono glifo ordinari.

### 8.4 Esempio
Con il DTI che contiene `rosso§vino` come unico concetto, `/^ ta bere nu io au rosso§vino ^/` trasmette **una** chiave; con un DTI che contiene solo `video` e `rosso`, `/^ ta mostrare au video§rosso ^/` trasmette due chiavi unite da `§`.

---

## 9. Connettori e albero di profondità

### 9.1 Il problema
Le frasi subordinate (condizioni, conseguenze, dettagli) formano un albero. Le lingue a marcatori di apertura e chiusura (come le parentesi) costringono a ricordare chiusure lontane e si rompono facilmente.

### 9.2 La soluzione
La profondità si scrive **una volta, all'inizio del blocco**, con una sequenza di `:`. Un blocco a profondità T è figlio dell'ultimo blocco attivo a profondità T−1. Non servono chiusure: lo stack si «pota» da solo. Le congiunzioni collegano fratelli nell'ordine scritto.

### 9.3 La regola

| Connettore | Profondità del blocco che segue |
|---|---|
| `:` ripetuto N volte | N |
| `y`, `vel`, `aut` senza `:` | la profondità dell'ultimo blocco attivo |
| `sed` senza `:` | livello 0 |
| congiunzione + `:`×N | N |

- Passare da profondità D a una profondità maggiore di D+1 è un errore di ambito. La profondità massima è configurabile (64 per default).
- **Ruoli forzati**: `oh` senza `:` è fratello dell'ultimo blocco attivo; `deep` senza `:` va al livello 0; `def` è trasparente (non entra nell'albero). Con `:` espliciti vincono sempre i `:`.
- **Coordinazione**: i fratelli conservano l'ordine scritto; ogni blocco porta la propria congiunzione (`y` «e», `vel` «o», `aut` «o esclusivo», `sed` «ma»). Il protocollo non presume raggruppamenti logici: la lettura logica spetta ad Apricot Logic.
- **Ancore**: permettono di inserire un gruppo di blocchi a una profondità di riferimento.

| Tile di portabilità | Effetto |
|---|---|
| `: anchor <id>` | Registra un'ancora alla profondità indicata; modalità assoluta |
| `: anchorrel <id>` | Come `anchor`, modalità relativa: un blocco con C `:` va a profondità K + (C − 1), dove K è la profondità dell'ancora |
| `: rel <id>` | Riattiva la modalità relativa su un'ancora aperta |
| `: reloff <id>` | Disattiva la modalità relativa |
| `: end ~` | Chiude l'ultima ancora e torna alla sua profondità |

Le ancore valgono solo nel periodo: un'ancora non chiusa è un errore.

### 9.4 Esempi
**Esempio 1.** `/^ ta vedere nu io au canis : ta abbaiare nu ~ :: tazato fame nu ~ : ta osservare nu gatto ^/`

| Blocco | `:` | Profondità | Padre |
|---|---|---|---|
| `ta vedere…` | — | 0 | — |
| `ta abbaiare…` | `:` | 1 | vedere |
| `tazato fame…` | `::` | 2 | abbaiare |
| `ta osservare…` | `:` | 1 | vedere (lo stack si pota) |

**Esempio 2 — congiunzioni.** `/^ ta p : ta a y : ta b sed : ta c ^/`: i figli di `p` sono `a`, `y b`, `sed c`, esattamente in quest'ordine.

---

## 10. Ereditarietà nelle subordinate

### 10.1 Il problema
Ripetere in ogni subordinata il tempo, il quantificatore o il confronto del padre allunga i messaggi e moltiplica gli errori.

### 10.2 La soluzione
Una subordinata eredita tutto ciò che non scrive; ciò che scrive prevale sempre.

### 10.3 La regola

| Che cosa si eredita | Condizioni |
|---|---|
| Tempo o condizionale, numero del segnale, quantificatore, `ref`, comparativo | Solo se la subordinata ha una testa primaria o secondaria e non scrive quel determinante. L'eredità è transitiva (si eredita il valore effettivo del padre) |
| Nulla | Per i blocchi `deep` (salvo i determinanti scritti su `deep`) |

Controlli che restano:

| Controllo | Esito |
|---|---|
| Un comparativo **scritto** senza alcun Accusativo nel blocco | Errore semantico (un comparativo ereditato non impone nulla) |
| Segnale secondario al livello 0 | Errore di ambito |

### 10.4 Esempio
`/^ taallalt x nu y : ta z nu w ^/`: la subordinata `ta z` eredita `all` (per ogni) e `alt` (passato remoto) dal padre.

---

## 11. Le anafore `~` e `~~`

### 11.1 Il problema
Ripetere un glifo già detto è lungo e, se ci sono omonimi, può creare dubbi su *quale* glifo si intendesse. Serve un rinvio breve con una regola di ricerca univoca.

### 11.2 La soluzione
Due soli operatori, con un ordine di ricerca fisso: `~` riprende un glifo, `~~` riprende un intero sema.

### 11.3 La regola

**Ordine di ricerca dell'antecedente**: 1) il padre; 2) gli antenati fino alla radice; 3) i blocchi di livello 0 precedenti, dal più recente; 4) i blocchi di livello 0 conservati da `/^/`. I rami potati non sono visibili; `~` non vede il proprio blocco.

| Operatore | Che cosa riprende |
|---|---|
| `~` | L'ultimo glifo dell'antecedente nell'ordine canonico (segnale → `n` → `a` → `d` → `g` → `ab`, ordine scritto a parità di caso), saltando le coppie che contengono un `~~` |
| `~~` | L'intero sema antecedente, allo stesso modo in qualsiasi slot pari |

- Le catene di `~` si seguono fino al valore finale.
- Il numero: se omesso, la coppia di caso con `~` eredita il numero della coppia richiamata.
- Senza antecedente: errore di riferimento.
- Il ruolo Anafora spetta solo agli slot non annotati scritti `~` o `~~`: uno slot annotato Glifo o Etichetta, qualunque sia il suo token, non cerca antecedenti (capitolo 2).

### 11.4 Esempio
`/^ ta vedere nu io au humanum : tazato venire n ~ ^/`

| Passo | Spiegazione |
|---|---|
| `~` | Nella subordinata, l'antecedente è il padre `ta vedere…` |
| Ordine canonico del padre | segnale `vedere`, `nu io`, `au humanum` |
| Ultimo glifo | `humanum` |
| Numero | `n ~` eredita il Singolare di `au humanum` |

Lettura: «io vedo un uomo; se l'uomo viene…».

---

## 12. Le variabili `def`

### 12.1 Il problema
Un insieme di valori (per esempio un sensore scelto) può servire in molti blocchi; riscriverlo è lungo e rischioso.

### 12.2 La soluzione
Il blocco `def` dà un nome a uno o più valori; finché la variabile è viva, ogni glifo uguale al nome è un riferimento a lei.

### 12.3 La regola

| Operazione | Tile | Effetto |
|---|---|---|
| Assegnazione | `a`, `au`, `am`, `anum` | Crea la variabile (errore se è già viva) |
| Modifica | `abfi` | Sostituisce il valore |
| Unione | `absi` | Aggiunge un valore |
| Cancellazione | `abse ~` | Elimina la variabile (`~` qui è un marcatore) |

- `def`, `defu`, `defm`, `defnum` fissano il numero della variabile.
- Il nome è il token intero; dopo un `def` serve sempre un connettore (`y` o `:`).
- Usare una variabile inesistente è un errore di riferimento; `^^/` cancella tutte le variabili.

### 12.4 Esempio
`/^ defu dev a sensore§ottico y defu dev abfi sensore§termico y ta leggere nu dev y defu dev abse ~ ^/`

| Blocco | Effetto |
|---|---|
| `defu dev a sensore§ottico` | `dev` vale `sensore§ottico` |
| `defu dev abfi sensore§termico` | `dev` ora vale `sensore§termico` |
| `ta leggere nu dev` | si legge il sensore termico |
| `defu dev abse ~` | `dev` non esiste più |

---

## 13. I cicli `abdur` e il piano di esecuzione

### 13.1 Il problema
Un apparato deve ripetere un'azione un numero preciso di volte. Se il numero fosse dedotto dal significato di una parola («tre», «spesso») ogni macchina potrebbe interpretarlo diversamente.

### 13.2 La soluzione
Il numero di ripetizioni sta **solo** nella tile `abdur=<N>`. Il motore produce un **piano di esecuzione**: l'elenco ordinato dei passi che l'apparato deve compiere, ognuno con l'indicazione del giro a cui appartiene.

### 13.3 La regola
- `abdur=<N>` (anche `abduru=<N>`, `abdurm=<N>`, `abdurnum=<N>`) ripete **la clausola e tutto il suo sottoalbero** N volte. Il glifo che segue (di solito `~`) non viene mai letto per contare.
- Più `abdur=<N>` nella **stessa** clausola formano cicli **annidati nell'ordine scritto**: il primo scritto è il giro esterno.
- `abdur` senza `=<N>` è un **marcatore di controllo**: ammesso in una subordinata il cui segnale ha un condizionale scritto o ereditato; il protocollo ne controlla la forma, la valutazione della condizione spetta ad Apricot Logic. Nel piano le subordinate di controllo compaiono una volta, prima del corpo ripetuto.
- Un limite configurabile (1000 per default) vale sia per N sia per il numero di passi generati.

**Che cos'è un passo.** Ogni passo del piano contiene: la clausola da eseguire, l'**indice di iterazione** (un numero per ogni ciclo che la racchiude, dal giro esterno a quello interno, contando da 0) e gli eventuali bersagli (capitolo 14).

### 13.4 Esempi
**Esempio 1 — ciclo dentro un ciclo.** `/^ ta scansione abdur=2 ~ : ta misura abdur=2 ~ ^/`: «esegui la scansione 2 volte; a ogni scansione esegui la misura 2 volte».

| Passo | Clausola | Indice `[giro esterno, giro interno]` | Significato |
|---|---|---|---|
| 1 | scansione | `[0]` | prima scansione |
| 2 | misura | `[0, 0]` | prima misura della prima scansione |
| 3 | misura | `[0, 1]` | seconda misura della prima scansione |
| 4 | scansione | `[1]` | seconda scansione |
| 5 | misura | `[1, 0]` | prima misura della seconda scansione |
| 6 | misura | `[1, 1]` | seconda misura della seconda scansione |

**Esempio 2 — due cicli sulla stessa clausola.** `/^ ta scansione abdur=2 ~ abdur=3 ~ ^/`: «ripeti la scansione 2 × 3 = 6 volte, come 2 giri esterni da 3 giri interni».

| Passo | Indice `[giro esterno, giro interno]` |
|---|---|
| 1 | `[0, 0]` |
| 2 | `[0, 1]` |
| 3 | `[0, 2]` |
| 4 | `[1, 0]` |
| 5 | `[1, 1]` |
| 6 | `[1, 2]` |

---

## 14. Gli insiemi strutturali

### 14.1 Il problema
«Attiva tutti i sensori tranne quelli ottici» richiede di sapere quali concetti sono sensori. Un motore che lo deducesse dal significato delle parole sarebbe arbitrario.

### 14.2 La soluzione
Gli insiemi si calcolano **solo** sulle relazioni dichiarate nel DTI (ogni concetto può indicare il proprio concetto più generale con `base:`) e sui valori delle variabili `def`.

### 14.3 La regola
Una clausola ha bersagli solo se ha un quantificatore (`all`, `some`) o un comparativo `cup`, `cua`, `cuo`.

| Elemento | Operazione |
|---|---|
| Coppie in Accusativo | Insieme base |
| Coppie in Nominativo, se nel blocco non ci sono Accusativi | Insieme base |
| `all` / `some` | Estendono base e unioni ai discendenti abilitati |
| `cup` / `absi` | Unione |
| `cua` / `abpa` | Intersezione (con l'operando e i suoi discendenti) |
| `cuo` / `abse` | Sottrazione (dell'operando e dei suoi discendenti) |

Ordine: unione → intersezione → sottrazione; risultato ordinato per chiave, senza duplicati. Con `all` o senza quantificatore la modalità è «tutti»; con `some` è «almeno uno». Un operando composto `K1§K2` indica il record abilitato del DTI con `base:K1` e `param:K2`.

### 14.4 Esempio
`/^≈ deepall attivare nu sistema am sensore abse sensore§ottico ^/` (scritto dopo un periodo precedente, perché `/^≈` richiede un antecedente):

| Passo | Insieme |
|---|---|
| Base | `sensore` e tutti i suoi discendenti abilitati (per `all`) |
| Sottrazione | `sensore§ottico` e i suoi discendenti |
| Risultato | gli altri sensori, per esempio quelli termici, in ordine di chiave |

---

## 15. La diagnostica e i warning

### 15.1 Il problema
Un errore deve essere riconosciuto in modo identico da tutte le implementazioni, e non deve lasciare il sistema in uno stato a metà.

### 15.2 La soluzione
Sei codici d'errore fissi, ingestione **atomica** (o tutto o niente) e la possibilità, a rischio dell'utente, di trasformare in avvisi i codici che non compromettono la struttura.

### 15.3 La regola

| Codice | Esempi |
|---|---|
| `ERR_SYNTAX` | Token o struttura non validi; spazi non canonici; periodo non chiuso; caratteri invisibili |
| `ERR_ORDER` | Determinanti fuori ordine o ripetuti |
| `ERR_REFERENCE` | Anafora senza antecedente; ancora sconosciuta; variabile inesistente |
| `ERR_SEMANTIC` | Comparativo scritto senza Accusativo; `def` senza operazioni; conflitto di numero |
| `ERR_SCOPE` | Salto di livello; sincronia senza antecedente; ancore non chiuse; limiti superati; segnale secondario al livello 0; marcatore `abdur` senza condizionale |
| `ERR_CASE_AMBIGUITY` | Tile di caso senza glifo a fine testo |

- Con qualsiasi errore la sessione resta identica a prima; gli errori tornano tutti, ordinati per posizione.
- **Declassamento a warning**: per default ogni codice è bloccante. L'utente può dichiarare singoli codici come avvisi (nella configurazione o nel file `settings.toml`). Restano sempre bloccanti gli errori **strutturali**, che impediscono di costruire un albero coerente (tutti quelli di sintassi e d'ordine, i limiti, i salti di livello, le ancore sbilanciate, la tile di caso senza glifo). Gli avvisi si leggono dal periodo accettato.

### 15.4 Esempio
`/^ quata x ^/` (segnale secondario al livello 0) è un `ERR_SCOPE`. Se l'utente dichiara `ERR_SCOPE = "warning"`, il periodo è accettato e l'avviso resta consultabile; `/^ Ta x ^/` (maiuscola in una tile) resta invece un errore, perché strutturale.

---

## 16. Il dizionario DTI, le chiavi e i numerali

### 16.1 Il problema
La stessa idea si dice `porta` in italiano, `uscio` in un dialetto, `door` in inglese. Se le macchine si scambiassero parole, dovrebbero condividere una lingua; se si scambiassero numeri progressivi, due dizionari diversi entrerebbero in conflitto.

### 16.2 La soluzione
Ogni **concetto** ha una **chiave** universale, immutabile e priva di significato; le parole delle singole lingue sono solo forme che puntano alla chiave. Il dizionario ha **due livelli**: uno snapshot universale in sola lettura e un overlay locale con le scelte dell'utente.

### 16.3 La regola: la chiave

| Proprietà | Valore |
|---|---|
| Lunghezza | 32 caratteri |
| Alfabeto | `[a-z0-9]` |
| Primo carattere | sempre una lettera `[a-z]` |
| Caratteri 2–31 | 30 caratteri `[a-z0-9]` casuali |
| Ultimo carattere | controllo **Luhn mod 36**, che rivela ogni sostituzione di un singolo carattere |
| Identità | una chiave per concetto, mai modificata, riusata o cancellata (si depreca con `superseded_by:`) |

**Perché la prima lettera.** Un token che inizia con una lettera è una chiave; uno che inizia con una cifra è un numero. La distinzione si fa guardando un solo carattere, senza confondere mai una chiave con un numero, nemmeno con uno di 32 cifre.

### 16.4 La regola: i due livelli e il formato `.awdti`

| Livello | Contenuto |
|---|---|
| Snapshot universale | Concetti, forme della lingua di interscambio, relazioni `base:`/`param:`, permessi morfologici, sistema numerico. Sostituibile in blocco |
| Overlay locale | Concetti abilitati (`use`), forme locali, concetti locali (`new`), etichette (`label`), politica per le chiavi sconosciute |

```text
#awdti 1
#role universal
#version 42
#lang en
#numeral base:10 digits:<K0>,…,<K9>
rec <KEY> <forma> [display:"…"] [syn:a,b] [base:<KEY>] [param:<KEY>] [morph:+AGGL,+CONTR,+UNION,+AGGL_NUM] [superseded_by:<KEY>] [attr:k§v]
```
```text
#awdti 1
#role local
#universal 42
#lang it
#policy degrade|unknown|reject
use <KEY> [form:<forma>] [display:"…"] [syn:…]
new <KEY> <forma> [base:<KEY>] [display:"…"] [syn:…] [morph:…]
label <KEY> <forma>
```

- Una forma è qualsiasi sequenza di caratteri visibili senza spazi, compresi `§`, `~` e `~~`.
- La forma primaria è unica per lingua; sinonimi (`syn:`) e forme leggibili con spazi (`display:`) servono all'editor.
- Al caricamento si controllano chiavi duplicate, riferimenti mancanti, cicli nelle catene `base:`, il divieto di riusare una chiave universale per un concetto locale, la versione dello snapshot.
- Esiste una forma binaria deterministica (`.dti.bin`), validata con le stesse regole del testo.

### 16.5 La regola: la «mente di un bambino» — numerali e permessi morfologici

**Il problema.** Un sistema umano dà per scontata la base 10 e la libertà di comporre parole. Un'intelligenza diversa può contare in base 2 o 8, e può non ammettere che due concetti si fondano in uno.

**La soluzione.** Il protocollo si comporta come la mente di un bambino: **non sa nulla finché non gli viene insegnato**. La base dei numeri e ogni modo di comporre i concetti vanno dichiarati nel DTI.

| Dichiarazione | Significato |
|---|---|
| `#numeral base:<B> digits:<K0>,…,<K(B−1)>` | La base B (da 2 a 10) e, per ogni cifra da 0 a B−1, il concetto della cifra. In trasmissione le cifre usano i caratteri ASCII `0`–`9` |
| Cifra ≥ base | Il numerale non è valido (`InvalidNumeral`) |

| Permesso morfologico | Quando serve | Chi deve averlo |
|---|---|---|
| `+UNION` | Unione di concetti con `§` risolta per segmenti (`video§rosso`) | Ogni concetto unito (i numerali sono esenti: in `30§celsius` serve solo a `celsius`) |
| `+AGGL` | Forma composta per agglutinazione: la forma è la concatenazione delle forme dei due componenti (`cagn` + `one` → `cagnone`) | Entrambi i componenti (`base:` e `param:`) |
| `+CONTR` | Forma composta per contrazione: forma diversa dalla concatenazione (`cucciolo` = cane + piccolo) | Entrambi i componenti |
| `+AGGL_NUM` | Ogni cifra di un numerale | Il concetto della cifra |

Le forme composte vanno sempre censite come record: il motore non spezza mai da solo una parola sconosciuta.

### 16.6 Esempi
**Esempio 1 — una chiave, tre parole.** Lo snapshot ha `rec door000…i door`; l'overlay italiano ha `use door000…i form:porta syn:uscio`. Che l'utente scriva `porta` o `uscio` (l'editor normalizza `uscio` in `porta`), sulla rete viaggia sempre la stessa chiave di 32 caratteri.

**Esempio 2 — tre basi.**

| Dichiarazione | Numerale | Esito |
|---|---|---|
| `#numeral base:2 digits:<K0>,<K1>` | `101` | valido (cifre 0 e 1) |
| `#numeral base:8 digits:<K0>,…,<K7>` | `17` | valido |
| `#numeral base:8 …` | `19` | non valido: la cifra 9 non esiste in base 8 |
| `#numeral base:10 digits:<K0>,…,<K9>` | `42` | valido se ogni cifra ha `+AGGL_NUM` |

---

## 17. Le etichette

### 17.1 Il problema
Spesso serve trasmettere un nome che non è un concetto del dizionario: un codice di lotto, una sigla, un nome proprio. Le vecchie virgolette (`"…"`) creavano un'eccezione nella grammatica e non permettevano a chi riceve di sapere se quella stringa fosse prevista.

### 17.2 La soluzione
Le virgolette non hanno più valore grammaticale (`"` è un carattere come gli altri). Un'**etichetta** è una voce del DTI **locale** con il tipo *Etichetta*. Lo slot che la contiene viene **annotato** come Etichetta, e in trasmissione l'etichetta porta un prefisso riservato.

### 17.3 La regola

| Aspetto | Regola |
|---|---|
| Censimento | `label <KEY> <forma>` nel DTI locale; mai nello snapshot universale |
| Coesistenza | La stessa forma può essere sia glifo sia etichetta nel DTI locale |
| Avviso di censimento | Se la forma di un glifo locale o di un'etichetta è già un glifo universale nella lingua di interscambio, il caricamento produce un avviso (mai un errore), così l'utente sa che il glifo esiste |
| Selezione obbligatoria | Per scrivere una forma censita come etichetta l'utente sceglie dalla tendina **Glifo** o **Etichetta**; senza annotazione (flussi programmatici) uno slot è un glifo |
| Contenuto | Un'etichetta è una stringa inerte: il protocollo non la interpreta e l'editor non la normalizza. Due etichette identiche che indicano oggetti diversi nel mondo reale non sono un problema del protocollo: il contesto spetta a chi scrive e a chi legge |
| Trasmissione | `LVA<SOTTOTIPO><forma>`: `LVA` identifica l'etichetta, il sottotipo è di 3 lettere maiuscole (`AAA` = etichetta base), poi la forma: `LVAAAAroma` |
| Ricezione | Un token che inizia con `LVA` è sempre un'etichetta, anche se assente dal DTI di chi riceve; se la forma è anche un glifo del DTI di chi riceve, viene evidenziata l'omonimia |
| Sottotipi nuovi | Un sottotipo diverso da `AAA` è stato generato dalla macchina mittente: chi riceve lo accetta, lo registra nella propria tabella dei sottotipi con una nota informativa e lo conserva se ritrasmette. È un errore solo un token troncato o corrotto (meno di 7 caratteri o sottotipo non di 3 lettere maiuscole) |

> [!IMPORTANT]
> **Esecuzione in ambiente safe.** Un'etichetta non censita nel DTI locale di chi riceve, un'etichetta esterna ammessa dalla policy o un'etichetta con sottotipo dinamico non ancora classificato va trattata ed eseguita **solo in un ambiente isolato e passivo** (sandbox), senza alcun privilegio di esecuzione diretta su comandi di sistema o attuatori fisici.

### 17.4 Esempi
**Esempio 1.** Il DTI locale contiene `use <K_verde> form:verde` e `label <K> verde`. L'utente scrive `ta dipingere au verde`: l'editor apre la tendina; se sceglie Etichetta, in trasmissione `verde` diventa `LVAAAAverde`; se sceglie Glifo, diventa la chiave del concetto «verde».

**Esempio 2.** Una macchina riceve `LVAXYZlotto42`: è un'etichetta di sottotipo `XYZ`, sconosciuto; il messaggio è accettato, `XYZ` viene registrato, e l'etichetta va gestita in sandbox.

---

## 18. Lo Stadio 3: forma espansa, forma indicizzata, viste

### 18.1 Il problema
Lo stesso periodo deve essere letto da una persona (che vuole vedere i valori risolti), trasmesso tra macchine (senza dipendere dalle lingue) e mostrato in vari formati.

### 18.2 La soluzione
Il motore lavora in tre stadi: **Stadio 1** sintassi, **Stadio 2** analisi (albero, anafore, variabili, ereditarietà), **Stadio 3** risoluzione nel DTI e rese.

### 18.3 La regola

| Resa | Che cosa contiene | A che cosa serve |
|---|---|---|
| Forma espansa | Niente `def` né ancore; `:` espliciti; variabili e `~` sostituiti dai valori (con il numero effettivo nelle coppie di caso); `~~` resta `~~` | Lettura umana dei valori risolti |
| Forma indicizzata | La forma **scritta**, con ogni glifo sostituito dalla sua chiave e ogni etichetta da `LVA…`; restano `def`, ancore, `~`, `~~` anaforici, tile e numerali | **Unica forma di scambio tra macchine** |
| Viste | Struttura scritta con forme leggibili (`HumanOriginal`), con forme primarie (`CanonicalLexical`) o indicizzata (`DtiIndexed`); su una riga o una clausola per riga | Consultazione |

- Risoluzione: si cerca prima il token intero tra le forme primarie, poi per segmenti `§`; un concetto non abilitato suggerisce `use <KEY>`.
- I nomi delle variabili e gli identificatori delle ancore diventano la chiave DTI se sono forme primarie abilitate, altrimenti una **chiave effimera** derivata in modo deterministico dal nome.
- Chi riceve verifica ogni chiave con la propria politica (abilitata, degradata al concetto più generale, sconosciuta, rifiutata).
- Un periodo ricevuto in forma indicizzata e ritrasmesso resta identico.

### 18.4 Esempio
`/^ ta vedere nu io au canis : ta abbaiare nu ~ ^/`
- Forma espansa: `/^ ta vedere nu io au canis : ta abbaiare nu canis ^/` (l'anafora è sostituita).
- Forma indicizzata: `/^ ta <K_vedere> nu <K_io> au <K_canis> : ta <K_abbaiare> nu ~ ^/`, dove ogni `<K_…>` è una chiave di 32 caratteri; `~` resta, perché chi riceve lo risolve allo stesso modo.

---

## 19. Il gestore delle policy

### 19.1 Il problema
Un apparato non deve eseguire tutto ciò che è grammaticalmente valido: certi comandi o certi ruoli vanno vietati, certe soglie rispettate.

### 19.2 La soluzione
Un file di regole scritto **sulle chiavi** (quindi indipendente dalla lingua) controlla ogni periodo dopo lo Stadio 3, con ricerche a costo garantito e deterministico.

### 19.3 La regola
```text
#awpolicy 1
#universal <versione snapshot>
#default allow|deny
signal <KEY> ta,ka,deep
role <KEY> n:<K>,<K> a:<K> d:… g:… ab:…
threshold <KEY> cha|chu|col min:<numero> max:<numero>
qualifier <KEY> <K>,<K>
require <KEY> zato|quo
label deny | label allow:<K>,<K>
```

| Filtro | Regola |
|---|---|
| Teste ammesse | Quali teste di segnale si possono usare con quel glifo |
| Matrice dei ruoli | Quali chiavi ammesse per ogni caso base |
| Soglie e qualificatori | Numerale dell'Accusativo entro [min, max]; qualificatori `§` ammessi |
| Subordinata obbligatoria | Un figlio con condizionale scritto o con `quo` (conferma) |
| Sicurezza rafforzata | Etichette vietate, o ammesse solo per i glifo di segnale elencati |

Con `#default deny` la policy è una lista bianca. Le regole valgono sul glifo del segnale; i valori raggiunti tramite variabili e anafore si controllano già risolti; un sema richiamato con `~~` vale come il glifo del suo segnale. Contano come etichette gli slot annotati Etichetta e i token `LVA…`.

### 19.4 Esempio
Con `signal <K_aprire> ta` e `#default deny`, `/^ ka aprire nu operatore au valvola ^/` (una domanda) viola le teste ammesse: per `aprire` è permessa solo l'affermazione `ta`.

---

## 20. Il Legacy Shield

### 20.1 Il problema
Un vecchio apparato non conosce GHLIP e non sa difendersi da comandi malformati o costruiti per sfruttare differenze di lettura.

### 20.2 La soluzione
Uno scudo posto davanti all'apparato applica Stadio 1, 2, 3 e la policy con la sicurezza rafforzata sempre attiva, e inoltra **solo la forma canonica ricostruita**, mai il testo originale.

### 20.3 La regola
- Nessun accesso alla rete: è un filtro puro.
- Un input bloccato non lascia traccia nella sessione dello scudo.
- Aggiornare la protezione significa sostituire i file del DTI e della policy.

### 20.4 Esempio
Un input con un'etichetta `LVAAAArm` destinata a un comando viene bloccato dalla sicurezza rafforzata; l'apparato non lo riceve.

---

## 21. Il Ghost Protocol

### 21.1 Il problema
Un messaggio cifrato in modo tradizionale è riconoscibile come tale e non attraversa i nodi che si aspettano GHLIP.

### 21.2 La soluzione
Il Ghost Protocol sostituisce ogni chiave e ogni cifra della forma indicizzata con un'altra della stessa classe: il risultato è **ancora un periodo GHLIP valido**, che lo Stadio 2 legge con la stessa struttura. La decifratura è esatta.

### 21.3 La regola

| Elemento | Regola |
|---|---|
| Algoritmi | SHAKE256 interno senza dipendenze; Ascon, Skein-512, BLAKE3 opzionali; derivazione della chiave con Argon2id |
| Che cosa cambia | Chiavi dello snapshot (tra loro), cifre dei numerali (stessa base e lunghezza), chiavi effimere (tra loro); opzionalmente alcune tile grammaticali dentro classi sicure |
| Che cosa resta | `~`, `~~`, `abse ~`, `end ~`, le tile `def`, i connettori |
| Coerenza | Atomi uguali visibili nello stesso contesto ricevono lo stesso cifrato, così variabili e anafore si risolvono identiche |
| Integrità | Busta con suite, versione, numero di sequenza, nonce e codice di autenticazione; confronto a tempo costante |
| Ordine | Si accetta solo il messaggio successivo: replay e messaggi fuori ordine sono rifiutati |
| Etichette | Rifiutate, per non esporre testo in chiaro |

### 21.4 Esempio
`/^ ta <K1> au <K2> ^/` diventa, per esempio, `/^ ta <K7> au <K4> ^/`: un periodo valido e indistinguibile da un messaggio normale, che solo chi ha la chiave riporta al testo originale.

---

## 22. L'editor assistito

### 22.1 Il problema
Scrivere testo canonico (un solo spazio, determinanti in ordine, forme primarie) è faticoso per una persona.

### 22.2 La soluzione
Un pre-processore accetta una scrittura libera e la converte in testo canonico, segnalando ciò che serve senza decidere al posto dell'utente.

### 22.3 La regola

| Funzione | Comportamento |
|---|---|
| Commenti | `//` all'inizio di un token o dopo un delimitatore di periodo |
| Spazi liberi | Tab e a capo diventano un solo spazio |
| Graffe `{ }` | Aprono i figli della clausola appena scritta e generano i `:` |
| Treno del segnale | `ta_zato_cha` diventa `tachazato` (riordino automatico) |
| Alias | `>` `<` `=` `!=` diventano `cha` `chu` `col` `cla` |
| Locuzioni | «verde smeraldo» diventa la forma primaria del concetto |
| Normalizzazione | Sinonimi e forme leggibili diventano forme primarie |
| Controllo DTI | Ogni glifo non censito (anche un delimitatore finito in uno slot pari) viene segnalato, con la proposta di censirlo come Glifo o Etichetta |
| Annotazioni | Una sola regola per ogni token: se un token non annotato ha più di un ruolo possibile (il ruolo dato dalla posizione più i ruoli censiti nel DTI), la tendina mostra i ruoli ammessi tra **Anafora**, **Glifo** ed **Etichetta**; le scelte accompagnano il testo all'ingestione |
| Suggerimenti | Tendina dei glifo e dei sema recenti che iniziano con le lettere digitate; automatica o con `Alt+Down` |
| Copia con ancore | Con una selezione: `Ctrl+C` copia le subordinate come blocco `anchor` (profondità assolute), `Alt+C` come blocco `anchorrel` (profondità relative). Senza selezione `Alt+C` apre l'inserimento dei comparativi |

```toml
[editor]
autocomplete = "auto"     # oppure "manual"

[shortcuts.actions]
suggest = "Alt+Down"
copy_absolute = "Ctrl+C"
copy_relative = "Alt+C"

[warnings]
ERR_SCOPE = "warning"     # a proprio rischio
```

### 22.4 Esempio
L'utente scrive su due righe:
```text
/^ ta_zato_> grandis n canis   // commento
   a 30 ^/
```
L'editor produce `/^ tachazato grandis n canis a 30 ^/`: il treno del segnale è riordinato, l'alias `>` diventa `cha`, il commento e gli spazi spariscono.

---

## 23. Il contratto con Apricot Logic

### 23.1 Il problema
Il Livello 1 deve poter calcolare (per esempio sommare valori) senza introdurre nella grammatica funzioni aritmetiche o eccezioni.

### 23.2 La soluzione
Un gancio transazionale: una funzione esterna riceve una clausola e restituisce **testo GHLIP**, che viene reingerito atomicamente con tutte le verifiche.

### 23.3 La regola
- L'interfaccia `FunctionEvaluator` e il metodo `Session::evaluate_and_ingest`.
- Gli operandi multipli si scrivono con `absi`, mai con `§`.
- Il protocollo non contiene funzioni aritmetiche; determinismo e terminazione sono obblighi della funzione esterna.

### 23.4 Esempio
`/^ ka somma nu sistema a 10 absi 20 ^/`: la funzione legge gli operandi `10` e `20` e risponde con un periodo come `/^ ta somma nu sistema a 30 ^/`, che entra nella sessione solo se valido.

---

## 24. Architettura del software

### 24.1 Il problema
Lo stesso protocollo deve girare su un server e su un microcontrollore con poche decine di kilobyte di memoria, senza comportamenti imprevedibili.

### 24.2 La soluzione
Due crate Rust: un nucleo senza sistema operativo e senza dipendenze, e un crate ospite con file, editor, esportazione e riga di comando.

### 24.3 La regola

| Crate | Ambiente | Contenuto |
|---|---|---|
| `apriwave-core` | `no_std` + allocatore, zero dipendenze; compila per Cortex-M0+ e Cortex-M4F | Lexer, parser, analisi, rese, DTI e formato binario su buffer, piano, policy, Legacy Shield |
| `apriwave` | libreria standard | Tutto il nucleo, più cache su file del DTI, editor, esportazione JSON, riga di comando, Ghost Protocol |

- Nessun codice `unsafe`, nessun arresto improvviso: ogni errore è un risultato strutturato.
- Mappe ordinate: iterazione deterministica e ricerche con costo garantito.
- Memoria: ogni testo è memorizzato una volta; una sessione chiusa con `^^/` si può compattare.
- Riga di comando: `apriwave-cli [--ast] [--json] [--warn=<CODICE>]… "<periodo>"`.

### 24.4 Esempio
Un sensore con Cortex-M0+ include solo `apriwave-core` e un allocatore: riceve un periodo indicizzato, lo valida, costruisce il piano di esecuzione e comanda l'attuatore secondo i passi del piano.

---

## 25. Glossario

| Termine | Definizione |
|---|---|
| Tile | Strumento grammaticale dell'insieme chiuso (segnale, caso, connettore, delimitatore…) |
| Glifo | Voce del dizionario DTI; contenuto mai interpretato dal protocollo |
| Sema | Blocco di significato completo: tile + glifo |
| Slot Locking | Alternanza rigida: posizione dispari = tile, posizione pari = glifo |
| DTI | Dizionario a due livelli: snapshot universale e overlay locale |
| Chiave | Identificatore di 32 caratteri di un concetto |
| Etichetta | Stringa inerte censita nel DTI locale, trasmessa come `LVA<SOTTOTIPO><forma>` |
| Annotazione dello slot | Scelta Anafora / Glifo / Etichetta per un token dello slot pari |
| Anafora | Rinvio a un glifo (`~`) o a un sema (`~~`) precedente |
| Stack di profondità | Struttura che colloca ogni blocco nell'albero in base ai `:` |
| Piano di esecuzione | Elenco ordinato dei passi da eseguire, con indici di iterazione e bersagli |
| Forma indicizzata | Forma di scambio tra macchine: struttura scritta con chiavi ed etichette `LVA…` |
| Policy | Regole di sicurezza scritte sulle chiavi |
| Legacy Shield | Filtro posto davanti a un apparato non GHLIP |
| Ghost Protocol | Cifratura che produce periodi GHLIP validi |
| Apricot Logic / LEMMA | Progetto e motore logico di Livello 1 |
