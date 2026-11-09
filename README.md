# ExpanderCraft

Espansore di testo scritto in Rust, clone di [Beeftext](https://github.com/xmichelo/Beeftext) (C++/Qt, licenza MIT).
Digiti una parola chiave in qualsiasi applicazione e ExpanderCraft la sostituisce con il testo associato (il *combo*).

Primo obiettivo: Windows 10/11. macOS e Linux hanno già un backend dedicato, dietro la stessa interfaccia di piattaforma.

## Funzionalità

Equivalenti a Beeftext 15:

- **Combo**: parola chiave → testo, con nome, descrizione, gruppo, stato attivo/disattivo.
- **Corrispondenza** *esatta* (il testo digitato coincide con la parola chiave) o *libera* (il combo scatta anche in mezzo a una parola), per combo o come valore predefinito.
- **Maiuscole/minuscole**: distinte oppure indifferenti, per combo o come valore predefinito.
- **Gruppi** attivabili e disattivabili, riordinabili.
- **Variabili** nel testo (vedi sotto), comprese quelle annidate (`#{combo:}`), con protezione dalla ricorsione infinita.
- **Selettore dei combo** (picker): finestra di ricerca aperta con una scorciatoia globale (predefinita `Ctrl+Alt+Shift+Invio`), ordinata per ultimo uso.
- **Sostituzione automatica**, oppure **manuale** con scorciatoia (`Ctrl+Alt+Shift+B`), con eventuale attivazione tramite **spazio** dopo la parola chiave.
- **Scorciatoia per attivare/disattivare** l'applicazione (`Ctrl+Alt+Shift+V`, disattivata di default).
- **Emoji** con codici brevi tra delimitatori (`|smile|` → 😄), anche nel picker.
- **Applicazioni escluse** (nessuna sostituzione), **applicazioni sensibili** (testo digitato carattere per carattere al posto dell'incolla), esclusioni specifiche per le emoji. Sono ammessi i caratteri jolly `*` e `?`.
- **Appunti**: il testo viene incollato tramite gli appunti, poi il contenuto precedente viene ripristinato (su Windows tutti i formati, immagini e file compresi).
- **Import/export** JSON (formato Beeftext) e CSV (formato TextExpander), con gestione dei conflitti: salta, sovrascrivi, sovrascrivi solo i più vecchi.
- **Backup automatico** a ogni salvataggio (ultimi 50), ripristino da backup, backup e ripristino manuali.
- **Promemoria** (cheat sheet) in CSV.
- **Cartella della lista personalizzabile**, per esempio una cartella OneDrive o Dropbox per condividere i combo tra computer.
- **Edizione portatile**: con un file `Portable.bin` accanto all'eseguibile, tutti i dati vanno nella sottocartella `Data`.
- **Avvio all'accesso**, icona nell'area di notifica, istanza singola (un secondo avvio mostra la finestra già aperta).
- **Suono** di conferma (integrato, oppure un file WAV a scelta).
- Interfaccia in **inglese** (predefinita), **italiano**, **francese** e **spagnolo**, con cambio di lingua immediato; tema chiaro, scuro o di sistema.

### Compatibilità con Beeftext

ExpanderCraft legge e scrive il formato `comboList.json` di Beeftext (versione 10) e aggiorna automaticamente i file delle versioni 1–9. Al primo avvio, se trova una libreria di Beeftext (`%LOCALAPPDATA%\beeftext.org\Beeftext\comboList.json`), propone di importarla con tutti i gruppi. Un backup di Beeftext si può aprire con *Strumenti → Ripristina la lista dei combo*.

## Variabili

| Variabile | Effetto |
|---|---|
| `#{clipboard}` | testo contenuto negli appunti |
| `#{cursor}` | posizione del cursore dopo la sostituzione |
| `#{date}` `#{time}` `#{dateTime}` | data/ora nel formato del sistema (su Windows: formato lungo delle impostazioni internazionali) |
| `#{dateTime:yyyy-MM-dd HH:mm}` | formato personalizzato in stile Qt (`d dd ddd dddd M MM MMM MMMM yy yyyy h hh H HH m mm s ss z zzz AP ap`, testo tra apici, `w`/`ww` per la settimana ISO) |
| `#{dateTime:+1d-2h:formato}` | data/ora spostata (`y M w d h m s z`, segno obbligatorio; formato vuoto ammesso, i due punti finali sono obbligatori) |
| `#{combo:kw}` `#{upper:kw}` `#{lower:kw}` `#{trim:kw}` | testo di un altro combo, eventualmente trasformato |
| `#{input:Descrizione}` | chiede un valore all'utente; la stessa descrizione viene chiesta una volta sola |
| `#{envVar:NOME}` | variabile d'ambiente |
| `#{powershell:percorso.ps1[:timeoutMs]}` | output di uno script PowerShell (10 s di default, `0` = nessun limite; su macOS/Linux usa `pwsh`) |
| `#{key:nome[:ripetizioni]}` | preme un tasto (`tab`, `enter`, `up`, `f5`, `volumeUp`…) |
| `#{shortcut:Ctrl+Shift+J}` | preme una scorciatoia |
| `#{delay:ms}` | pausa |

Dentro una variabile, `\}` scrive `}` e `\\` scrive `\`. Le variabili sconosciute restano nel testo così come sono.

## Compilazione

Serve Rust stabile ≥ 1.88 (edizione 2024): <https://rustup.rs>.

### Windows

```powershell
cargo build --release
# eseguibile: target\release\expandercraft.exe
```

L'eseguibile non è firmato: al primo avvio SmartScreen può mostrare un avviso (*Ulteriori informazioni → Esegui comunque*).

Cross-compilazione da Linux:

```bash
sudo apt install gcc-mingw-w64-x86-64
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

### macOS

```bash
cargo build --release
```

Al primo avvio vanno concessi i permessi *Accessibilità* e *Monitoraggio input* (Impostazioni di Sistema › Privacy e sicurezza). I permessi sono legati al binario: dopo ogni ricompilazione può servire rimuoverli e concederli di nuovo.

### Linux

Dipendenze (Debian/Ubuntu):

```bash
sudo apt install libgtk-3-dev libxdo-dev libayatana-appindicator3-dev libxtst-dev libx11-dev libxkbcommon-dev
```

Su Fedora, e su sistemi immutabili come Bazzite dentro un `toolbox`:

```bash
toolbox create && toolbox enter
sudo dnf install gcc gtk3-devel libxdo-devel libappindicator-gtk3-devel libXtst-devel libxkbcommon-devel
cargo build --release
```

### Wayland

Wayland non permette a un programma di vedere i tasti premuti nelle altre finestre. In una sessione Wayland ExpanderCraft legge quindi direttamente tastiere e mouse da `/dev/input` (evdev, come Espanso) e scrive con una tastiera virtuale creata con uinput. Il layout della tastiera viene letto dalle impostazioni del desktop (KDE Plasma, GNOME, Hyprland, `localectl`) e si può forzare in Preferenze › Avanzate › Layout della tastiera (per esempio `it` o `fr(oss)`). L'applicazione attiva, usata per le esclusioni, viene chiesta a KWin con un piccolo script, a Sway e a Hyprland tramite i loro socket. Le finestre di ExpanderCraft girano su XWayland, perché Wayland non permette di nasconderle nella tray né di dar loro il focus.

Per leggere `/dev/input` serve un permesso. Ci sono due modi:

1. **Capability sull'eseguibile** (consigliato). Basta un comando, da ripetere dopo ogni aggiornamento dell'eseguibile:

   ```bash
   sudo setcap cap_dac_override+p ~/.local/bin/expandercraft
   ```

   ExpanderCraft attiva il permesso solo nel momento in cui apre una tastiera o la tastiera virtuale, e lo spegne subito dopo. All'avvio, se il permesso manca, una finestra mostra il comando esatto con il percorso dell'eseguibile.

2. **Gruppo `input`**. Su Bazzite si fa con `ujust add-user-to-input-group` (oppure da Bazzite Portal), poi serve una regola per la tastiera virtuale e un nuovo accesso:

   ```bash
   echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/60-expandercraft-uinput.rules
   sudo udevadm control --reload && sudo udevadm trigger
   ```

   In questo modo tutti i programmi dell'utente possono leggere la tastiera, non solo ExpanderCraft.

Senza permesso ExpanderCraft funziona come prima, solo nelle applicazioni X11.

## Stato delle piattaforme

| | Windows | macOS | Linux |
|---|---|---|---|
| Intercettazione tastiera e mouse | hook `WH_KEYBOARD_LL`/`WH_MOUSE_LL` | `CGEventTap` | X11: `rdev` (XRecord); Wayland: evdev (`/dev/input`) |
| Testo da tasto | `ToUnicodeEx` con il layout della finestra attiva, tasti morti preservati | `CGEventKeyboardGetUnicodeString` | X11: `rdev`; Wayland: libxkbcommon con il layout del desktop, tasti morti compresi |
| Sintesi dei tasti | `SendInput` (eventi marcati e ignorati dall'hook) | `CGEventPost` (eventi marcati) | X11: `enigo` (XTest); Wayland: tastiera virtuale uinput |
| Scorciatoie globali intercettate | sì | sì | rilevate, non intercettate |
| Appunti con backup | tutti i formati in memoria; esclusi da cronologia Win+V e cloud | testo e immagini (`arboard`), esclusi dalla cronologia | testo e immagini (`arboard`, anche Wayland data-control), esclusi da Klipper |
| Applicazione attiva | nome dell'eseguibile | nome dell'app (`NSWorkspace`) | X11: `_NET_ACTIVE_WINDOW`; Wayland: KWin, Sway, Hyprland (GNOME: solo finestre X11) |
| Avvio all'accesso | `HKCU\…\Run` | LaunchAgent | `~/.config/autostart` |
| Verifiche fatte | compilazione, clippy, test end-to-end sotto Wine (Notepad) | compilazione e clippy per `aarch64-apple-darwin` | X11: test end-to-end (xterm); Wayland: eventi simulati, sway headless, KWin simulato su D-Bus |

**Limiti noti**

- *Linux/Wayland*: serve il permesso di leggere `/dev/input` (vedi sotto). Le scorciatoie arrivano anche all'applicazione attiva; si usa solo il primo layout della lista; su GNOME le esclusioni per applicazione valgono solo per le applicazioni X11. Il backend evdev/uinput va ancora collaudato su una sessione reale.
- *macOS*: il backend compila; manca ancora il collaudo su una macchina reale e la creazione di un bundle `.app` firmato.
- *Windows*: come ogni espansore basato su `SendInput`, non scrive nelle finestre eseguite come amministratore se ExpanderCraft non è a sua volta elevato (UIPI).

## Architettura

```
src/
├── main.rs            avvio: percorsi, preferenze, log, istanza singola, thread
├── library.rs         Combo, Group, Library: formato JSON Beeftext v1–10, CSV, import con conflitti
├── variables.rs       valutazione delle variabili (trait VariableHost, testabile)
├── fragments.rs       suddivisione in testo/#{key}/#{shortcut}/#{delay}, calcolo di #{cursor}
├── qtdate.rs          formati data/ora in stile Qt, spostamenti, nomi localizzati
├── engine.rs          thread del motore: buffer digitato, corrispondenze, emoji, rendering
├── app_state.rs       stato condiviso (RwLock) tra hook, motore e interfaccia
├── i18n.rs            traduzioni Fluent: tr("id"), tr!("id", nome = valore), plurali CLDR
├── prefs.rs  shortcut.rs  process_list.rs  backup.rs  emoji.rs  sound.rs  paths.rs  logger.rs  icon.rs
├── platform/
│   ├── mod.rs         trait Keyboard, Clipboard, InputSink e tipi comuni
│   ├── windows.rs     backend Windows
│   ├── macos.rs       backend macOS
│   ├── linux.rs       backend Linux (X11)
│   └── unix_common.rs appunti arboard, istanza singola via socket Unix, suoni
└── ui/                interfaccia egui/eframe: finestra principale, dialoghi, preferenze, picker, tray
locales/               en.ftl (riferimento), it.ftl, fr.ftl, es.ftl
```

Flusso: l'hook di piattaforma traduce i tasti in `InputEvent` (carattere, backspace, interruzione, scorciatoia) e li invia al thread del motore. Il motore aggiorna il buffer, cerca i combo, valuta le variabili (il dialogo `#{input:}` passa per l'interfaccia tramite un canale), cancella la parola chiave e incolla o digita il risultato. L'interfaccia gira nel thread principale e resta attiva anche con la finestra nascosta nella tray: il picker e il dialogo di input sono finestre indipendenti.

Per aggiungere una piattaforma basta un modulo in `platform/` che esponga le stesse funzioni (`start_input_monitor`, `new_keyboard`, `new_clipboard`, `active_app_name`, …).

## Traduzioni

I testi dell'interfaccia stanno in `locales/*.ftl`, nel formato [Fluent](https://projectfluent.org), e vengono inclusi nell'eseguibile. L'inglese (`en.ftl`) è il riferimento: un messaggio che manca in un'altra lingua viene mostrato in inglese. Nelle preferenze, alla voce Aspetto, si sceglie tra le quattro lingue o "Lingua del sistema", che usa la lingua del sistema operativo se è tradotta e l'inglese altrimenti.

Nel codice un messaggio semplice si scrive `tr("menu-quit")`, uno con parametri `tr!("msg-combos-imported", count = n)`. I plurali seguono le regole CLDR di ogni lingua (in francese 0 e 1 sono singolari):

```ftl
msg-combos-imported =
    { $count ->
        [one] { $count } combo a été importé.
       *[other] { $count } combos ont été importés.
    }
```

Per aggiungere una lingua:

1. copia `locales/en.ftl` in `locales/xx.ftl` e traduci i valori, lasciando invariati gli id e le variabili `{ $nome }`;
2. in `src/i18n.rs` aggiungi la variante a `Lang` (codice, nome nella lingua stessa, `include_str!`);
3. in `src/prefs.rs` aggiungi la voce a `Language` e alla lista del menu in `ui/prefs_window.rs`;
4. esegui `cargo test`: i test controllano che il file abbia tutti i messaggi di `en.ftl`, con le stesse variabili, e che ogni id usato nel codice esista.

I nomi dei tasti nelle scorciatoie mostrate nelle preferenze sono tradotti (Maiusc, Maj, Mayús…), ma nei file e nelle variabili `#{shortcut:}` restano in inglese, così i combo funzionano con qualsiasi lingua.

## Differenze rispetto a Beeftext

- `#{key:insert}` preme Ins (in Beeftext premeva Home); `#{key:backspace}` è disponibile.
- `#{shortcut:}` si ferma alla prima `}`, quindi più scorciatoie nello stesso testo funzionano.
- Le scorciatoie configurate vengono intercettate e non arrivano all'applicazione attiva.
- Dopo una sostituzione, i tasti modificatori vengono ripremuti solo se l'utente li tiene ancora premuti, così Maiusc o Ctrl non restano bloccati.
- Il testo incollato non entra nella cronologia degli appunti di Windows.
- Un combo non può richiamare se stesso con `#{combo:}`.
- Con lo spazio come attivatore e lo spazio finale mantenuto, `#{cursor}` tiene conto dello spazio aggiunto.
- Non c'è il controllo degli aggiornamenti. Le traduzioni sono quattro (inglese, italiano, francese, spagnolo) contro le undici di Beeftext.

## Test

```bash
cargo test
```

62 test unitari coprono le traduzioni, il formato dei file, i layout di tastiera e il backend Wayland (eventi evdev simulati), le variabili, i formati di data, i frammenti, il cursore, le corrispondenze e il motore di sostituzione (con tastiera e appunti simulati).

## Licenza

MIT. ExpanderCraft si ispira a Beeftext di Xavier Michelon (MIT) e ne riprende il formato dei file. I codici brevi delle emoji provengono da gemoji (MIT) tramite il crate `emojis`.
