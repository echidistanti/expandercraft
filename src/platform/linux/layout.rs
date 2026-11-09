//! Detection of the keyboard layout of a Wayland session.
//!
//! The compositor does not tell other programs which layout it uses, so ExpanderCraft reads it from the
//! settings of the desktop: KDE Plasma (`kxkbrc`), GNOME (`gsettings`), Hyprland, then the system defaults
//! (`localectl`). The preferences can override it. Only the first layout of a list is used.

use super::xkb::LayoutNames;

/// The detected layout and where it was found.
pub fn detect() -> (LayoutNames, &'static str) {
    if let Some(l) = from_env() {
        return (l, "XKB_DEFAULT_LAYOUT");
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_ascii_uppercase();
    if desktop.contains("KDE")
        && let Some(l) = kde()
    {
        return (l, "KDE");
    }
    if (desktop.contains("GNOME") || desktop.contains("UNITY") || desktop.contains("CINNAMON") || desktop.contains("BUDGIE"))
        && let Some(l) = gnome()
    {
        return (l, "GNOME");
    }
    if let Some(l) = super::compositor::hyprland_layout().and_then(|s| first_of(&s, "")) {
        return (l, "Hyprland");
    }
    if let Some(l) = system() {
        return (l, "localectl");
    }
    (LayoutNames::layout("us", ""), "default")
}

fn from_env() -> Option<LayoutNames> {
    let layout = std::env::var("XKB_DEFAULT_LAYOUT").ok().filter(|s| !s.is_empty())?;
    let mut l = first_of(&layout, &std::env::var("XKB_DEFAULT_VARIANT").unwrap_or_default())?;
    l.model = std::env::var("XKB_DEFAULT_MODEL").unwrap_or_default();
    l.options = std::env::var("XKB_DEFAULT_OPTIONS").unwrap_or_default();
    Some(l)
}

/// The first layout of comma-separated lists (`it,us` + `,intl`).
fn first_of(layouts: &str, variants: &str) -> Option<LayoutNames> {
    let layout = layouts.split(',').next()?.trim();
    if layout.is_empty() {
        return None;
    }
    let variant = variants.split(',').next().unwrap_or("").trim();
    Some(LayoutNames::layout(layout, variant))
}

fn kde() -> Option<LayoutNames> {
    let path = directories::BaseDirs::new()?.config_dir().join("kxkbrc");
    parse_kxkbrc(&std::fs::read_to_string(path).ok()?)
}

/// `~/.config/kxkbrc`: the `[Layout]` group, used only when `Use=true`.
pub fn parse_kxkbrc(text: &str) -> Option<LayoutNames> {
    let mut in_layout = false;
    let (mut use_it, mut layouts, mut variants, mut model, mut options) =
        (false, String::new(), String::new(), String::new(), String::new());
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_layout = line == "[Layout]";
            continue;
        }
        if !in_layout {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        match k.trim() {
            "Use" => use_it = v.trim() == "true",
            "LayoutList" => layouts = v.trim().into(),
            "VariantList" => variants = v.trim().into(),
            "Model" => model = v.trim().into(),
            "Options" => options = v.trim().into(),
            _ => {}
        }
    }
    if !use_it {
        return None;
    }
    let mut l = first_of(&layouts, &variants)?;
    l.model = model;
    l.options = options;
    Some(l)
}

fn gnome() -> Option<LayoutNames> {
    let get = |key: &str| {
        let out = std::process::Command::new("gsettings").args(["get", "org.gnome.desktop.input-sources", key]).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let mut l = parse_gnome_sources(&get("sources")?)?;
    if let Some(opts) = get("xkb-options") {
        l.options = parse_gvariant_strings(&opts).join(",");
    }
    Some(l)
}

/// `[('xkb', 'it'), ('xkb', 'us+intl')]`: the first xkb source.
pub fn parse_gnome_sources(text: &str) -> Option<LayoutNames> {
    let strings = parse_gvariant_strings(text);
    let mut it = strings.chunks(2);
    let spec = it.find(|pair| pair.len() == 2 && pair[0] == "xkb").map(|pair| pair[1].clone())?;
    let (layout, variant) = spec.split_once('+').unwrap_or((&spec, ""));
    Some(LayoutNames::layout(layout, variant))
}

/// The quoted strings of a GVariant text, in order.
fn parse_gvariant_strings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\'' || c == '"' {
            let s: String = chars.by_ref().take_while(|x| *x != c).collect();
            out.push(s);
        }
    }
    out
}

fn system() -> Option<LayoutNames> {
    let files = ["/etc/X11/xorg.conf.d/00-keyboard.conf", "/etc/vconsole.conf", "/etc/default/keyboard"];
    files.iter().find_map(|f| parse_system_file(&std::fs::read_to_string(f).ok()?))
}

/// `Option "XkbLayout" "it"` (xorg.conf.d) or `XKBLAYOUT=it` (vconsole.conf, /etc/default/keyboard).
pub fn parse_system_file(text: &str) -> Option<LayoutNames> {
    let (mut layout, mut variant, mut model, mut options) = (String::new(), String::new(), String::new(), String::new());
    for line in text.lines().map(str::trim).filter(|l| !l.starts_with('#')) {
        let (key, value) = if let Some(rest) = line.strip_prefix("Option") {
            let q = parse_gvariant_strings(rest);
            if q.len() < 2 {
                continue;
            }
            (q[0].clone(), q[1].clone())
        } else if let Some((k, v)) = line.split_once('=') {
            (k.trim().to_string(), v.trim().trim_matches('"').to_string())
        } else {
            continue;
        };
        match key.to_ascii_lowercase().as_str() {
            "xkblayout" => layout = value,
            "xkbvariant" => variant = value,
            "xkbmodel" => model = value,
            "xkboptions" => options = value,
            _ => {}
        }
    }
    let mut l = first_of(&layout, &variant)?;
    l.model = model;
    l.options = options;
    Some(l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kde_settings() {
        let text = "[$Version]\nupdate_info=kxkb.upd\n\n[Layout]\nDisplayNames=,\nLayoutList=it,us\nModel=pc105\nOptions=compose:ralt\nUse=true\nVariantList=,intl\n";
        let l = parse_kxkbrc(text).unwrap();
        assert_eq!((l.layout.as_str(), l.variant.as_str(), l.model.as_str(), l.options.as_str()), ("it", "", "pc105", "compose:ralt"));
        assert!(parse_kxkbrc("[Layout]\nLayoutList=it\nUse=false\n").is_none());
    }

    #[test]
    fn gnome_settings() {
        assert_eq!(parse_gnome_sources("[('xkb', 'fr+oss'), ('xkb', 'us')]\n").unwrap(), LayoutNames::layout("fr", "oss"));
        assert_eq!(parse_gnome_sources("[('ibus', 'mozc-jp'), ('xkb', 'es')]").unwrap(), LayoutNames::layout("es", ""));
        assert!(parse_gnome_sources("@a(ss) []").is_none());
    }

    #[test]
    fn system_files() {
        let xorg = "Section \"InputClass\"\n        Identifier \"system-keyboard\"\n        MatchIsKeyboard \"on\"\n        Option \"XkbLayout\" \"it\"\nEndSection\n";
        assert_eq!(parse_system_file(xorg).unwrap().layout, "it");
        let vconsole = "KEYMAP=it\nXKBLAYOUT=es,us\nXKBVARIANT=cat,\n";
        assert_eq!(parse_system_file(vconsole).unwrap(), LayoutNames::layout("es", "cat"));
        assert!(parse_system_file("KEYMAP=it\n").is_none());
    }
}
