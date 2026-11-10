# Novità

## Prossima versione

- Di default l'interfaccia usa la lingua del sistema, e l'inglese se la lingua del sistema non è tradotta.
- Il gruppo creato al primo avvio, se ha ancora il nome predefinito ("Default Group"), viene rinominato nella lingua scelta.
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
