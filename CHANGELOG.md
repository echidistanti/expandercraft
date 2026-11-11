# Novità

## Non ancora pubblicato

- Tolta la tabella riassuntiva (*Strumenti → Crea una tabella riassuntiva*), sia in PDF sia in CSV. L'esportazione delle combo in JSON e CSV resta com'era.

## 0.2.2 — 10 novembre 2026

- **Sicurezza**: il testo inserito da `#{clipboard}`, `#{input:}`, `#{envVar:}` e `#{powershell:}` non viene più interpretato. Prima un testo copiato con dentro `#{shortcut:Win+R}…#{key:enter}` poteva premere tasti ed eseguire comandi.
- La lista delle combo che non si riesce a leggere all'avvio non viene più sostituita da una lista vuota, e non viene sovrascritta finché non la ripristini. Il messaggio parla della copia di sicurezza solo se la copia è riuscita davvero.
- Lista in una cartella condivisa (OneDrive, Dropbox…): le modifiche fatte su un altro computer vengono ricaricate entro pochi secondi. Se il file è cambiato mentre modificavi una combo, ExpanderCraft lo ricarica e ti avvisa, invece di sovrascrivere le modifiche dell'altro computer.
- Un errore durante una sostituzione non spegne più le espansioni fino al riavvio. Uno spostamento di data enorme (`#{dateTime:+100000000d:}`) viene ignorato.
- L'importazione di un file JSON non valido (per esempio di una versione più recente) mostra l'errore, invece di importare frammenti di JSON come se fosse un CSV.
- Premendo due volte la scorciatoia del selettore, la combo scelta arriva comunque nell'applicazione di partenza.
- Un valore non valido nelle preferenze azzera solo quel valore, e non più tutte le preferenze (cartella della lista compresa). Il file originale viene conservato come `settings.broken.json`.
- Interfaccia più leggera: la finestra di importazione da Beeftext non rilegge più il file a ogni ridisegno, e il selettore e la tabella ordinano le combo con meno lavoro.
- Pulizia del codice: tolti la dipendenza `rand` e il codice rimasto inutilizzato dal backend Linux.

## 0.2.1 — 10 novembre 2026

- Di default l'interfaccia usa la lingua del sistema, e l'inglese se la lingua del sistema non è tradotta.
- Il gruppo creato al primo avvio, se ha ancora il nome predefinito ("Default Group"), viene rinominato nella lingua scelta.
- Con lo spazio come attivatore, lo spazio che completa una parola chiave non arriva più all'applicazione: ExpanderCraft lo trattiene e cancella solo la parola chiave. Nel Blocco note di Windows 11 lo spazio avviava controllo ortografico e correzione automatica, e la combo usciva a volte come una semplice "v".
- La sostituzione aspetta che tu abbia rilasciato Ctrl, Alt, AltGr, Maiusc e Win prima di inviare i tasti: con le parole chiave che finiscono con un carattere fatto con AltGr (`@@`) il Blocco note di Windows 11 poteva entrare nella modalità dei tasti di scelta rapida e perdere l'incolla.
- Windows: nel Blocco note di Windows 11, e in altre app recenti, alcune combo comparivano come una semplice "v" al posto del testo. Ora la scorciatoia di incolla (e quelle di `#{shortcut:}`) tiene premuto Ctrl abbastanza a lungo anche quando l'app è in ritardo, per esempio dopo lo spazio che fa scattare la combo.
- Windows: nel Blocco note di Windows 11 le combo che iniziano con un segno di punteggiatura (`.cf`) lasciavano a volte il primo carattere. Il Blocco note perdeva un backspace quando arrivavano tutti insieme: ora ExpanderCraft li manda a 15 ms di distanza.
- Con la corrispondenza non rigorosa vince la parola chiave più lunga: scrivendo `@@@` scatta la combo `@@@` e non `@@`.
- Italiano: "shortcode" al posto di "codici delle emoji"; la casella delle preferenze è diventata "Usa gli shortcode delle emoji".

## 0.2.0 — 10 novembre 2026

- **Interfaccia tradotta** in inglese (lingua predefinita), italiano, francese e spagnolo, con i plurali giusti per ogni lingua. La lingua si sceglie in Preferenze › Aspetto, anche "Lingua del sistema", e cambia subito, menu della tray compreso.
- **Tabella riassuntiva in PDF**: nome, parola chiave e testo di ogni combo, gruppo per gruppo, in un A4 pronto da stampare. Resta disponibile il CSV.
- **Elenco delle emoji** dentro il programma (*Aiuto → Elenco delle emoji* o Preferenze › Emoji): ricerca per shortcode, nome o categoria; un clic copia lo shortcode.
- Italiano rivisto: "combo" è femminile ovunque, più qualche formulazione più chiara.
- Messaggi di errore sul file delle combo e sull'avvio automatico tradotti.
- macOS: il testo messo negli appunti durante una sostituzione resta fuori dalla cronologia.
- Linux è temporaneamente fuori dal ramo principale; il lavoro su X11 e Wayland continua nel branch `linux-wayland`.

## 0.1.0 — 9 novembre 2026

- Prima versione: clone di Beeftext in Rust per Windows, con il backend macOS già compilabile.
