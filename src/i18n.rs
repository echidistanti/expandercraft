//! User interface translations. English strings are the keys; the Italian table is in `italian()`.

use std::sync::atomic::{AtomicU8, Ordering};

use crate::prefs::Language;

static LANG: AtomicU8 = AtomicU8::new(0); // 0 = English, 1 = Italian

pub fn set_language(lang: Language) {
    let it = match lang {
        Language::English => false,
        Language::Italian => true,
        Language::Auto => sys_locale::get_locale().map(|l| l.to_lowercase().starts_with("it")).unwrap_or(false),
    };
    LANG.store(u8::from(it), Ordering::Relaxed);
}

pub fn is_italian() -> bool {
    LANG.load(Ordering::Relaxed) == 1
}

/// Translate a string of the user interface.
pub fn tr(s: &'static str) -> &'static str {
    if is_italian() { italian(s).unwrap_or(s) } else { s }
}

fn italian(s: &str) -> Option<&'static str> {
    Some(match s {
        "A Beeftext combo list was found on this computer. ExpanderCraft can import it with all its groups." => {
            "Su questo computer c'è una lista di combo di Beeftext. ExpanderCraft può importarla con tutti i suoi gruppi."
        }
        "A backup of the combo list is made every time it is saved." => {
            "Ogni salvataggio della lista dei combo crea una copia di sicurezza."
        }
        "A combo associates a short keyword to a snippet. Type the keyword in any application and it is replaced by the snippet." => {
            "Un combo associa una parola chiave breve a un testo. Digita la parola chiave in qualsiasi applicazione e viene sostituita dal testo."
        }
        "A copy of the file was saved as:" => "Una copia del file è stata salvata come:",
        "A synchronized folder (OneDrive, Dropbox...) lets you share your combos between computers." => {
            "Con una cartella sincronizzata (OneDrive, Dropbox...) puoi condividere i combo tra più computer."
        }
        "A text expander written in Rust. Type a keyword, get a snippet." => {
            "Un espansore di testo scritto in Rust. Digiti una parola chiave, ottieni un testo."
        }
        "About ExpanderCraft" => "Informazioni su ExpanderCraft",
        "Active" => "Attivo",
        "Advanced" => "Avanzate",
        "All combos" => "Tutti i combo",
        "All files" => "Tutti i file",
        "All the existing combos and groups will be replaced by the content of the backup file. Continue?" => {
            "Tutti i combo e i gruppi attuali verranno sostituiti dal contenuto del file di backup. Continuare?"
        }
        "Appearance" => "Aspetto",
        "Applications excluded from emoji substitution" => "Applicazioni escluse dalla sostituzione delle emoji",
        "Asks the user for a value. The same description is only asked once." => {
            "Chiede un valore all'utente. La stessa descrizione viene chiesta una volta sola."
        }
        "Automatic: combos are triggered as soon as the keyword is typed" => "Automatica: il combo scatta appena digiti la parola chiave",
        "Back up combo list..." => "Salva una copia della lista...",
        "Back up the combo list automatically" => "Salva automaticamente una copia della lista dei combo",
        "Backup" => "Backup",
        "Beeftext library found" => "Trovata una libreria di Beeftext",
        "Behavior" => "Comportamento",
        "Browse..." => "Sfoglia...",
        "CSV (TextExpander compatible)" => "CSV (compatibile con TextExpander)",
        "Cancel" => "Annulla",
        "Case insensitive" => "Maiuscole indifferenti",
        "Case sensitive" => "Distingue maiuscole",
        "Case sensitivity" => "Maiuscole/minuscole",
        "Change folder..." => "Cambia cartella...",
        "Clear" => "Cancella",
        "Click to enable or disable ExpanderCraft" => "Clicca per attivare o disattivare ExpanderCraft",
        "Click, then press the new shortcut" => "Clicca, poi premi la nuova scorciatoia",
        "Clipboard content" => "Contenuto degli appunti",
        "Combo" => "Combo",
        "Combo files" => "File di combo",
        "Combo in lowercase" => "Combo in minuscolo",
        "Combo in uppercase" => "Combo in maiuscolo",
        "Combo list file" => "File della lista dei combo",
        "Combo picker" => "Selettore dei combo",
        "Combo, trimmed" => "Combo senza spazi ai bordi",
        "Combos" => "Combo",
        "Confirmation" => "Conferma",
        "Copy snippet to clipboard" => "Copia il testo negli appunti",
        "Create your first combo" => "Crea il primo combo",
        "Current date and/or time, in the format of the system." => "Data e/o ora correnti, nel formato del sistema.",
        "Current date/time with a custom format (d dd ddd dddd M MM MMM MMMM yy yyyy h hh H HH m mm s ss zzz AP w ww)." => {
            "Data/ora correnti con un formato personalizzato (d dd ddd dddd M MM MMM MMMM yy yyyy h hh H HH m mm s ss zzz AP w ww)."
        }
        "Cursor position" => "Posizione del cursore",
        "Custom date/time" => "Data/ora personalizzata",
        "Custom executable" => "Eseguibile personalizzato",
        "Custom location" => "Percorso personalizzato",
        "Custom sound" => "Suono personalizzato",
        "Dark" => "Scuro",
        "Data folder" => "Cartella dei dati",
        "Date" => "Data",
        "Date and time" => "Data e ora",
        "Default" => "Predefinito",
        "Default Group" => "Gruppo predefinito",
        "Defaults for new combos" => "Valori predefiniti dei nuovi combo",
        "Delay (ms)" => "Pausa (ms)",
        "Delay between keystrokes when typing (ms)" => "Pausa tra i tasti durante la digitazione (ms)",
        "Delete group..." => "Elimina gruppo...",
        "Delete the group '{name}' and the {n} combos it contains?" => "Eliminare il gruppo '{name}' e i {n} combo che contiene?",
        "Delete the group '{name}'?" => "Eliminare il gruppo '{name}'?",
        "Delete the selected combo?" => "Eliminare il combo selezionato?",
        "Delete the {} selected combos?" => "Eliminare i {} combo selezionati?",
        "Delete..." => "Elimina...",
        "Description" => "Descrizione",
        "Diagnostics" => "Diagnostica",
        "Disable" => "Disattiva",
        "Disable group" => "Disattiva gruppo",
        "Duplicate" => "Duplica",
        "Edit combo" => "Modifica combo",
        "Edit group" => "Modifica gruppo",
        "Edit group..." => "Modifica gruppo...",
        "Edit..." => "Modifica...",
        "Emoji shortcodes" => "Codici delle emoji",
        "Emoji shortcodes are not replaced in these applications." => "In queste applicazioni i codici delle emoji non vengono sostituiti.",
        "Emoji shortcodes from gemoji (MIT License)." => "Codici delle emoji da gemoji (licenza MIT).",
        "Emojis" => "Emoji",
        "Enable" => "Attiva",
        "Enable / disable group" => "Attiva / disattiva gruppo",
        "Enable / disable shortcut" => "Scorciatoia per attivare / disattivare",
        "Enable group" => "Attiva gruppo",
        "Enable the combo picker" => "Attiva il selettore dei combo",
        "Enabled" => "Attivo",
        "Environment variable" => "Variabile d'ambiente",
        "Error" => "Errore",
        "Example of a process name:" => "Esempio di nome di processo:",
        "Example:" => "Esempio:",
        "Excluded applications" => "Applicazioni escluse",
        "Excluded applications..." => "Applicazioni escluse...",
        "ExpanderCraft - Combo picker" => "ExpanderCraft - Selettore dei combo",
        "ExpanderCraft does not perform any substitution in these applications." => {
            "ExpanderCraft non sostituisce nulla in queste applicazioni."
        }
        "ExpanderCraft is enabled" => "ExpanderCraft è attivo",
        "Export all combos..." => "Esporta tutti i combo...",
        "Export selected combos..." => "Esporta i combo selezionati...",
        "File" => "File",
        "Follow the system" => "Come il sistema",
        "For instance pwsh.exe (PowerShell 7) instead of Windows PowerShell." => {
            "Per esempio pwsh.exe (PowerShell 7) al posto di Windows PowerShell."
        }
        "General" => "Generale",
        "Generate cheat sheet..." => "Genera un promemoria...",
        "Grant ExpanderCraft the Accessibility and Input Monitoring permissions in System Settings › Privacy & Security, then restart it." => {
            "Concedi a ExpanderCraft i permessi Accessibilità e Monitoraggio input in Impostazioni di Sistema › Privacy e sicurezza, poi riavvialo."
        }
        "Group" => "Gruppo",
        "Groups" => "Gruppi",
        "Help" => "Aiuto",
        "Hide window" => "Nascondi finestra",
        "Import" => "Importa",
        "Import combos" => "Importa combo",
        "Import combos..." => "Importa combo...",
        "Import into group" => "Importa nel gruppo",
        "Information" => "Informazione",
        "Insert variable" => "Inserisci variabile",
        "Inspired by Beeftext by Xavier Michelon (MIT License). Combo files are compatible with Beeftext." => {
            "Ispirato a Beeftext di Xavier Michelon (licenza MIT). I file dei combo sono compatibili con Beeftext."
        }
        "Keep the space after the snippet" => "Mantieni lo spazio dopo il testo",
        "Key" => "Tasto",
        "Keyboard monitoring is not available: an X11 display is required." => {
            "Il monitoraggio della tastiera non è disponibile: serve un display X11."
        }
        "Keyword" => "Parola chiave",
        "Language" => "Lingua",
        "Last used" => "Ultimo uso",
        "Launch ExpanderCraft at login" => "Avvia ExpanderCraft all'accesso",
        "Left delimiter" => "Delimitatore sinistro",
        "Light" => "Chiaro",
        "Loose" => "Libera",
        "Manual: type the keyword, then press the trigger shortcut" => "Manuale: digita la parola chiave, poi premi la scorciatoia",
        "Matching" => "Corrispondenza",
        "Matching mode" => "Corrispondenza",
        "Move down" => "Sposta giù",
        "Move to group" => "Sposta nel gruppo",
        "Move up" => "Sposta su",
        "Name" => "Nome",
        "New combo" => "Nuovo combo",
        "New combo..." => "Nuovo combo...",
        "New combos: {}" => "Nuovi combo: {}",
        "New group" => "Nuovo gruppo",
        "New group..." => "Nuovo gruppo...",
        "No backup available." => "Nessun backup disponibile.",
        "No combo matches the search." => "Nessun combo corrisponde alla ricerca.",
        "No combo yet" => "Ancora nessun combo",
        "No matching combo." => "Nessun combo corrispondente.",
        "Not available in the portable edition." => "Non disponibile nell'edizione portatile.",
        "One process name per line (e.g. notepad.exe). Wildcards * and ? are accepted." => {
            "Un nome di processo per riga (per esempio notepad.exe). Sono ammessi i caratteri jolly * e ?."
        }
        "Open ExpanderCraft" => "Apri ExpanderCraft",
        "Open data folder" => "Apri la cartella dei dati",
        "Open log file" => "Apri il file di log",
        "Optional" => "Facoltativo",
        "Overwrite only the older existing combos ({})" => "Sovrascrivi solo i combo esistenti più vecchi ({})",
        "Overwrite the existing combos" => "Sovrascrivi i combo esistenti",
        "Paste with Shift+Insert instead of Ctrl+V" => "Incolla con Maiusc+Ins invece di Ctrl+V",
        "Paused" => "In pausa",
        "Pauses the substitution." => "Mette in pausa la sostituzione.",
        "Play a sound when a combo is triggered" => "Riproduci un suono quando scatta un combo",
        "Portable edition" => "Edizione portatile",
        "Position of the text cursor after the substitution." => "Dove si trova il cursore dopo la sostituzione.",
        "PowerShell script" => "Script PowerShell",
        "Preferences" => "Preferenze",
        "Preferences..." => "Preferenze...",
        "Press a shortcut... (Esc to cancel)" => "Premi una scorciatoia... (Esc per annullare)",
        "Press the shortcut to insert (Esc to cancel)..." => "Premi la scorciatoia da inserire (Esc per annullare)...",
        "Presses a key (tab, enter, up, f5...), optionally several times." => "Preme un tasto (tab, enter, up, f5...), anche più volte.",
        "Presses a keyboard shortcut." => "Preme una scorciatoia da tastiera.",
        "Quit" => "Esci",
        "Remove all backups..." => "Elimina tutti i backup...",
        "Remove all the automatic backups?" => "Eliminare tutti i backup automatici?",
        "Replace emoji shortcodes" => "Sostituisci i codici delle emoji",
        "Reset" => "Ripristina",
        "Reset all preferences to their default values?" => "Riportare tutte le preferenze ai valori predefiniti?",
        "Reset preferences..." => "Ripristina le preferenze...",
        "Restore" => "Ripristina",
        "Restore an automatic backup" => "Ripristina un backup automatico",
        "Restore an automatic backup..." => "Ripristina un backup automatico...",
        "Restore combo list..." => "Ripristina la lista dei combo...",
        "Restore the clipboard after a substitution" => "Ripristina gli appunti dopo una sostituzione",
        "Right delimiter" => "Delimitatore destro",
        "Search a combo..." => "Cerca un combo...",
        "Search combos..." => "Cerca nei combo...",
        "Sensitive applications" => "Applicazioni sensibili",
        "Sensitive applications..." => "Applicazioni sensibili...",
        "Shifted date/time" => "Data/ora spostata",
        "Shifted date/time: y M w d h m s z units, the sign is mandatory. The format can be empty, the last colon is mandatory." => {
            "Data/ora spostata: unità y M w d h m s z, il segno è obbligatorio. Il formato può restare vuoto, ma i due punti finali sono obbligatori."
        }
        "Short keywords may be triggered by mistake." => "Le parole chiave brevi possono scattare per errore.",
        "Shortcut" => "Scorciatoia",
        "Shortcut... (record)" => "Scorciatoia... (registra)",
        "Show emojis in the combo picker (after 3 characters)" => "Mostra le emoji nel selettore (dopo 3 caratteri)",
        "Skip the conflicting combos" => "Salta i combo in conflitto",
        "Snippet" => "Testo",
        "Strict" => "Esatta",
        "Strict: the typed text must be exactly the keyword. Loose: the combo can be triggered in the middle of a word." => {
            "Esatta: il testo digitato deve coincidere con la parola chiave. Libera: il combo può scattare anche in mezzo a una parola."
        }
        "Substitution" => "Sostituzione",
        "System language" => "Lingua del sistema",
        "Text insertion" => "Inserimento del testo",
        "The automatic startup could not be changed." => "Impossibile modificare l'avvio automatico.",
        "The backup was saved." => "Backup salvato.",
        "The cheat sheet was generated." => "Promemoria generato.",
        "The combo list could not be loaded." => "Impossibile caricare la lista dei combo.",
        "The combo list could not be saved." => "Impossibile salvare la lista dei combo.",
        "The combo list is now saved in:" => "Ora la lista dei combo viene salvata in:",
        "The combos could not be exported." => "Impossibile esportare i combo.",
        "The combos were exported." => "Combo esportati.",
        "The file could not be imported." => "Impossibile importare il file.",
        "The file could not be loaded." => "Impossibile caricare il file.",
        "The keyboard hook could not be installed: substitutions are disabled." => {
            "Impossibile intercettare la tastiera: le sostituzioni sono disattivate."
        }
        "The last group cannot be deleted." => "L'ultimo gruppo non si può eliminare.",
        "The output of a PowerShell script (10 s timeout by default, 0 means no timeout)." => {
            "L'output di uno script PowerShell (timeout predefinito di 10 s, 0 lo disattiva)."
        }
        "The snippet cannot be empty." => "Il testo non può essere vuoto.",
        "The snippet of another combo (also #{upper:}, #{lower:} and #{trim:})." => {
            "Il testo di un altro combo (anche #{upper:}, #{lower:} e #{trim:})."
        }
        "The text content of the clipboard." => "Il testo contenuto negli appunti.",
        "The text that replaces the keyword" => "Il testo che sostituisce la parola chiave",
        "The tray menu is updated at the next launch." => "Il menu dell'area di notifica si aggiorna al prossimo avvio.",
        "The value of an environment variable." => "Il valore di una variabile d'ambiente.",
        "Theme" => "Tema",
        "These applications do not support pasting with the keyboard: snippets are typed character by character instead." => {
            "Queste applicazioni non accettano l'incolla da tastiera: il testo viene digitato carattere per carattere."
        }
        "This keyword is also used by '{}': one of them will be chosen at random." => {
            "Anche '{}' usa questa parola chiave: ne verrà scelto uno a caso."
        }
        "Time" => "Ora",
        "Tools" => "Strumenti",
        "Trigger combos with a space typed after the keyword" => "Fai scattare i combo con uno spazio dopo la parola chiave",
        "Trigger shortcut" => "Scorciatoia di attivazione",
        "Use a shortcut to enable or disable ExpanderCraft" => "Usa una scorciatoia per attivare o disattivare ExpanderCraft",
        "Use the default folder" => "Usa la cartella predefinita",
        "User input" => "Input dell'utente",
        "Variables are evaluated each time a combo is triggered. Use \\} and \\\\ to write } and \\ inside a variable." => {
            "Le variabili vengono valutate ogni volta che scatta un combo. Usa \\} e \\\\ per scrivere } e \\ dentro una variabile."
        }
        "Variables reference" => "Guida alle variabili",
        "Version" => "Versione",
        "Warn about empty keywords" => "Avvisa se la parola chiave è vuota",
        "Warn about keywords shorter than 3 characters" => "Avvisa se la parola chiave ha meno di 3 caratteri",
        "Warning" => "Attenzione",
        "Warnings" => "Avvisi",
        "Wayland session: ExpanderCraft only works in X11/XWayland applications for now." => {
            "Sessione Wayland: per ora ExpanderCraft funziona solo nelle applicazioni X11/XWayland."
        }
        "Without a keyword, the combo can only be inserted with the combo picker." => {
            "Senza parola chiave, il combo si può inserire solo dal selettore."
        }
        "Write a log file" => "Scrivi un file di log",
        "Yes" => "Sì",
        "combos" => "combo",
        "e.g. btsig" => "es. ffirma",
        "groups" => "gruppi",
        "paused" => "in pausa",
        "{} backup files were removed." => "{} file di backup eliminati.",
        "{} combos have the same keyword as an existing combo." => "{} combo hanno la stessa parola chiave di un combo esistente.",
        "{} combos were imported." => "{} combo importati.",
        "{} combos were restored." => "{} combo ripristinati.",
        "{} invalid combo(s) were ignored while loading the combo list." => {
            "Durante il caricamento sono stati ignorati {} combo non validi."
        }
        "↑↓ to select · Enter to insert · Esc to close" => "↑↓ per scegliere · Invio per inserire · Esc per chiudere",
        _ => return None,
    })
}
