//! Printable cheat sheet: an A4 PDF with one section per group and, for each combo, its name, keyword and
//! snippet. The text uses the fonts bundled with egui (Ubuntu Light, Hack, Noto Emoji), subsetted and embedded,
//! so the file looks the same on every computer and printer.

use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow};
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use subsetter::GlyphRemapper;

/// A group and its combos, in the order of the main window.
pub struct Section {
    pub name: String,
    pub enabled: bool,
    pub combos: Vec<Entry>,
}

pub struct Entry {
    pub name: String,
    pub keyword: String,
    pub snippet: String,
    pub enabled: bool,
}

/// The translated texts of the document.
pub struct Labels {
    pub title: String,
    pub subtitle: String,
    pub name: String,
    pub keyword: String,
    pub snippet: String,
    /// Shown under the title when some combos are disabled.
    pub disabled_note: String,
    /// "Page 1 of 3".
    pub page: Box<dyn Fn(usize, usize) -> String>,
}

// ---- Page geometry (points, origin at the top left; converted when writing) ------------------------------------

const PAGE_W: f32 = 595.28;
const PAGE_H: f32 = 841.89;
const MARGIN_X: f32 = 42.0;
const MARGIN_TOP: f32 = 44.0;
const BOTTOM: f32 = PAGE_H - 54.0;
const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN_X;
const GAP: f32 = 12.0;
const NAME_W: f32 = CONTENT_W * 0.25;
const KEYWORD_W: f32 = CONTENT_W * 0.19;
const SNIPPET_W: f32 = CONTENT_W - NAME_W - KEYWORD_W - 2.0 * GAP;
const BODY: f32 = 9.0;
const LEADING: f32 = 12.2;
const ROW_PAD: f32 = 4.5;

type Rgb = [f32; 3];
const TEXT: Rgb = [0.12, 0.13, 0.15];
const MUTED: Rgb = [0.42, 0.45, 0.49];
const DISABLED: Rgb = [0.63, 0.65, 0.68];
const ACCENT: Rgb = [0x0F as f32 / 255.0, 0x76 as f32 / 255.0, 0x6E as f32 / 255.0];
const RULE: Rgb = [0.84, 0.86, 0.88];

// ---- Fonts ---------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Style {
    Sans,
    Mono,
}

const UBUNTU: usize = 0;
const HACK: usize = 1;
const NOTO_EMOJI: usize = 2;
const EMOJI_ICONS: usize = 3;

struct Font {
    data: &'static [u8],
    face: ttf_parser::Face<'static>,
    base_name: &'static str,
    units: f32,
}

#[derive(Clone, Copy)]
struct Glyph {
    font: usize,
    id: u16,
    /// Advance in ems.
    advance: f32,
    ch: char,
}

struct Fonts(Vec<Font>);

impl Fonts {
    fn load() -> Result<Self> {
        let sources: [(&'static [u8], &'static str); 4] = [
            (epaint_default_fonts::UBUNTU_LIGHT, "Ubuntu-Light"),
            (epaint_default_fonts::HACK_REGULAR, "Hack-Regular"),
            (epaint_default_fonts::NOTO_EMOJI_REGULAR, "NotoEmoji-Regular"),
            (epaint_default_fonts::EMOJI_ICON, "EmojiIconFont"),
        ];
        let mut fonts = Vec::new();
        for (data, base_name) in sources {
            let face = ttf_parser::Face::parse(data, 0).map_err(|e| anyhow!("{base_name}: {e}"))?;
            let units = f32::from(face.units_per_em());
            fonts.push(Font { data, face, base_name, units });
        }
        Ok(Self(fonts))
    }

    fn chain(style: Style) -> [usize; 4] {
        match style {
            Style::Sans => [UBUNTU, NOTO_EMOJI, EMOJI_ICONS, HACK],
            Style::Mono => [HACK, UBUNTU, NOTO_EMOJI, EMOJI_ICONS],
        }
    }

    fn glyph(&self, ch: char, style: Style) -> Option<Glyph> {
        Self::chain(style).into_iter().find_map(|font| {
            let f = &self.0[font];
            let id = f.face.glyph_index(ch)?;
            let advance = f32::from(f.face.glyph_hor_advance(id).unwrap_or(0)) / f.units;
            Some(Glyph { font, id: id.0, advance, ch })
        })
    }

    /// The glyphs of a single line of text. Characters that no font has become '?'.
    fn shape(&self, text: &str, style: Style) -> Vec<Glyph> {
        let mut out = Vec::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                // emoji presentation selectors and joiners have no glyph of their own
                '\u{FE0E}' | '\u{FE0F}' | '\u{200D}' | '\u{200B}' => continue,
                '\t' => {
                    for _ in 0..4 {
                        out.extend(self.glyph(' ', style));
                    }
                    continue;
                }
                c if c.is_control() => continue,
                _ => {}
            }
            if let Some(g) = self.glyph(ch, style).or_else(|| self.glyph('?', style)) {
                out.push(g);
            }
        }
        out
    }

    fn width(glyphs: &[Glyph], size: f32) -> f32 {
        glyphs.iter().map(|g| g.advance).sum::<f32>() * size
    }

    /// Word-wrap a text (with line breaks) to a width. Words longer than the width are cut.
    fn wrap(&self, text: &str, style: Style, size: f32, width: f32) -> Vec<Vec<Glyph>> {
        let mut lines = Vec::new();
        for paragraph in text.trim_end().replace("\r\n", "\n").replace('\r', "\n").split('\n') {
            let glyphs = self.shape(paragraph, style);
            if glyphs.is_empty() {
                lines.push(Vec::new());
                continue;
            }
            // split into words, each with the spaces that precede it
            let mut words: Vec<Vec<Glyph>> = Vec::new();
            let mut current: Vec<Glyph> = Vec::new();
            for g in glyphs {
                if g.ch == ' ' && current.iter().any(|c| c.ch != ' ') {
                    words.push(std::mem::take(&mut current));
                }
                current.push(g);
            }
            words.push(current);

            let mut line: Vec<Glyph> = Vec::new();
            let mut wrapped = false;
            for word in words {
                let fits = Fonts::width(&line, size) + Fonts::width(&word, size) <= width;
                if !line.is_empty() && !fits {
                    lines.push(std::mem::take(&mut line));
                    wrapped = true;
                }
                // a wrapped line does not start with the spaces between words (indentation is kept)
                let word: Vec<Glyph> =
                    if line.is_empty() && wrapped { word.into_iter().skip_while(|g| g.ch == ' ').collect() } else { word };
                for g in word {
                    if !line.is_empty() && Fonts::width(&line, size) + g.advance * size > width {
                        lines.push(std::mem::take(&mut line));
                        wrapped = true;
                    }
                    line.push(g);
                }
            }
            lines.push(line);
        }
        if lines.is_empty() {
            lines.push(Vec::new());
        }
        lines
    }
}

// ---- Layout --------------------------------------------------------------------------------------------------------

enum Op {
    Text { x: f32, baseline: f32, size: f32, color: Rgb, glyphs: Vec<Glyph> },
    Rule { x1: f32, x2: f32, y: f32, color: Rgb, width: f32 },
}

struct Layout<'a> {
    fonts: &'a Fonts,
    labels: &'a Labels,
    pages: Vec<Vec<Op>>,
    y: f32,
}

impl Layout<'_> {
    fn ops(&mut self) -> &mut Vec<Op> {
        self.pages.last_mut().expect("at least one page")
    }

    fn text(&mut self, x: f32, top: f32, size: f32, color: Rgb, glyphs: Vec<Glyph>) {
        if !glyphs.is_empty() {
            self.ops().push(Op::Text { x, baseline: top + size * 0.95, size, color, glyphs });
        }
    }

    fn line(&mut self, text: &str, style: Style, x: f32, size: f32, color: Rgb) {
        let glyphs = self.fonts.shape(text, style);
        let y = self.y;
        self.text(x, y, size, color, glyphs);
    }

    fn rule(&mut self, y: f32, color: Rgb, width: f32) {
        self.ops().push(Op::Rule { x1: MARGIN_X, x2: PAGE_W - MARGIN_X, y, color, width });
    }

    fn new_page(&mut self) {
        self.pages.push(Vec::new());
        self.y = MARGIN_TOP;
    }

    fn section_heading(&mut self, section: &Section) {
        let color = if section.enabled { ACCENT } else { DISABLED };
        self.line(&section.name, Style::Sans, MARGIN_X, 12.5, color);
        self.y += 18.0;
        self.rule(self.y, color, 0.8);
        self.y += 5.0;
        let l = self.labels;
        let labels = [(&l.name, 0.0), (&l.keyword, NAME_W + GAP), (&l.snippet, NAME_W + KEYWORD_W + 2.0 * GAP)];
        for (label, dx) in labels {
            let upper = label.to_uppercase();
            self.line(&upper, Style::Sans, MARGIN_X + dx, 7.2, MUTED);
        }
        self.y += 11.0;
        self.rule(self.y, RULE, 0.5);
    }

    fn title(&mut self, any_disabled: bool) {
        let l = self.labels;
        self.line(&l.title, Style::Sans, MARGIN_X, 19.0, ACCENT);
        self.y += 26.0;
        self.line(&l.subtitle, Style::Sans, MARGIN_X, 9.0, MUTED);
        self.y += 13.0;
        if any_disabled {
            self.line(&l.disabled_note, Style::Sans, MARGIN_X, 8.0, DISABLED);
            self.y += 12.0;
        }
        self.y += 12.0;
    }

    fn combo(&mut self, section: &Section, entry: &Entry) {
        let color = if entry.enabled && section.enabled { TEXT } else { DISABLED };
        let kw_color = if entry.enabled && section.enabled { ACCENT } else { DISABLED };
        let name = self.fonts.wrap(&entry.name, Style::Sans, BODY, NAME_W);
        let keyword = if entry.keyword.is_empty() { "—" } else { entry.keyword.as_str() };
        let keyword = self.fonts.wrap(keyword, Style::Mono, BODY, KEYWORD_W);
        let snippet = self.fonts.wrap(&entry.snippet, Style::Sans, BODY, SNIPPET_W);
        let rows = name.len().max(keyword.len()).max(snippet.len());

        // keep short combos on a single page; long ones continue on the next page
        let height = rows as f32 * LEADING + 2.0 * ROW_PAD;
        let page_room = BOTTOM - MARGIN_TOP - 60.0;
        if self.y + height.min(page_room) > BOTTOM {
            self.new_page();
            self.section_heading(section);
        }
        self.y += ROW_PAD;
        let columns = [
            (name, MARGIN_X, color),
            (keyword, MARGIN_X + NAME_W + GAP, kw_color),
            (snippet, MARGIN_X + NAME_W + KEYWORD_W + 2.0 * GAP, color),
        ];
        let mut columns: Vec<_> = columns.into_iter().map(|(lines, x, c)| (lines.into_iter(), x, c)).collect();
        for _ in 0..rows {
            if self.y + LEADING > BOTTOM {
                self.new_page();
                self.section_heading(section);
                self.y += ROW_PAD;
            }
            for (lines, x, c) in &mut columns {
                if let Some(glyphs) = lines.next() {
                    let (x, c, y) = (*x, *c, self.y);
                    self.text(x, y, BODY, c, glyphs);
                }
            }
            self.y += LEADING;
        }
        self.y += ROW_PAD;
        self.rule(self.y, RULE, 0.4);
    }

    fn footer(&mut self) {
        let total = self.pages.len();
        for i in 0..total {
            let label = (self.labels.page)(i + 1, total);
            let glyphs = self.fonts.shape(&label, Style::Sans);
            let x = PAGE_W - MARGIN_X - Fonts::width(&glyphs, 7.5);
            let brand = self.fonts.shape("ExpanderCraft", Style::Sans);
            let top = PAGE_H - 36.0;
            self.pages[i].push(Op::Text { x, baseline: top + 7.5, size: 7.5, color: MUTED, glyphs });
            self.pages[i].push(Op::Text { x: MARGIN_X, baseline: top + 7.5, size: 7.5, color: MUTED, glyphs: brand });
        }
    }
}

fn layout(fonts: &Fonts, sections: &[Section], labels: &Labels) -> Vec<Vec<Op>> {
    let mut l = Layout { fonts, labels, pages: Vec::new(), y: MARGIN_TOP };
    l.new_page();
    let any_disabled = sections.iter().any(|s| !s.combos.is_empty() && (!s.enabled || s.combos.iter().any(|c| !c.enabled)));
    l.title(any_disabled);
    for section in sections.iter().filter(|s| !s.combos.is_empty()) {
        // a heading needs at least one combo under it
        if l.y + 40.0 + LEADING + 2.0 * ROW_PAD > BOTTOM {
            l.new_page();
        }
        l.section_heading(section);
        for entry in &section.combos {
            l.combo(section, entry);
        }
        l.y += 16.0;
    }
    l.footer();
    l.pages
}

// ---- PDF writing ---------------------------------------------------------------------------------------------------

/// Generate the cheat sheet as a PDF document.
pub fn pdf(sections: &[Section], labels: &Labels) -> Result<Vec<u8>> {
    let fonts = Fonts::load()?;
    let pages = layout(&fonts, sections, labels);

    // glyphs used by each font, with the character they show (for copy and paste)
    let mut used: Vec<BTreeMap<u16, char>> = vec![BTreeMap::new(); fonts.0.len()];
    for op in pages.iter().flatten() {
        if let Op::Text { glyphs, .. } = op {
            for g in glyphs {
                used[g.font].entry(g.id).or_insert(g.ch);
            }
        }
    }
    let remappers: Vec<GlyphRemapper> =
        used.iter().map(|u| GlyphRemapper::new_from_glyphs_sorted(&u.keys().copied().collect::<Vec<_>>())).collect();

    let mut pdf = Pdf::new();
    let mut next = Ref::new(1);
    let catalog_id = next.bump();
    let tree_id = next.bump();
    let info_id = next.bump();
    let font_refs: Vec<Option<Ref>> = used.iter().map(|u| (!u.is_empty()).then(|| next.bump())).collect();
    let font_names: Vec<String> = (0..fonts.0.len()).map(|i| format!("F{i}")).collect();

    // fonts: Type0 / CIDFontType2 with Identity-H encoding, the subset glyph ids being the CIDs
    for (i, font) in fonts.0.iter().enumerate() {
        let Some(type0_id) = font_refs[i] else { continue };
        let (cid_id, descriptor_id, cmap_id, file_id) = (next.bump(), next.bump(), next.bump(), next.bump());
        let remapper = &remappers[i];
        let subset = subsetter::subset(font.data, 0, remapper).map_err(|e| anyhow!("{}: {e:?}", font.base_name))?;
        let base_name = format!("ECSUB{}+{}", (b'A' + i as u8) as char, font.base_name);
        let scale = 1000.0 / font.units;

        pdf.type0_font(type0_id)
            .base_font(Name(base_name.as_bytes()))
            .encoding_predefined(Name(b"Identity-H"))
            .descendant_font(cid_id)
            .to_unicode(cmap_id);

        let mut widths = vec![0.0f32; usize::from(remapper.num_gids())];
        for &old in used[i].keys() {
            if let Some(new) = remapper.get(old) {
                widths[usize::from(new)] = f32::from(font.face.glyph_hor_advance(ttf_parser::GlyphId(old)).unwrap_or(0)) * scale;
            }
        }
        let mut cid = pdf.cid_font(cid_id);
        cid.subtype(CidFontType::Type2)
            .base_font(Name(base_name.as_bytes()))
            .system_info(SystemInfo { registry: Str(b"Adobe"), ordering: Str(b"Identity"), supplement: 0 })
            .font_descriptor(descriptor_id)
            .default_width(0.0)
            .cid_to_gid_map_predefined(Name(b"Identity"));
        cid.widths().consecutive(0, widths);
        cid.finish();

        let bbox = font.face.global_bounding_box();
        let mut flags = FontFlags::SYMBOLIC;
        if font.face.is_monospaced() {
            flags |= FontFlags::FIXED_PITCH;
        }
        let ascent = f32::from(font.face.ascender()) * scale;
        pdf.font_descriptor(descriptor_id)
            .name(Name(base_name.as_bytes()))
            .flags(flags)
            .bbox(Rect::new(
                f32::from(bbox.x_min) * scale,
                f32::from(bbox.y_min) * scale,
                f32::from(bbox.x_max) * scale,
                f32::from(bbox.y_max) * scale,
            ))
            .italic_angle(0.0)
            .ascent(ascent)
            .descent(f32::from(font.face.descender()) * scale)
            .cap_height(font.face.capital_height().map(|h| f32::from(h) * scale).unwrap_or(ascent))
            .stem_v(80.0)
            .font_file2(file_id);

        let mut cmap = UnicodeCmap::new(Name(b"Custom"), SystemInfo { registry: Str(b"Adobe"), ordering: Str(b"UCS"), supplement: 0 });
        for (&old, &ch) in &used[i] {
            if let Some(new) = remapper.get(old) {
                cmap.pair(new, ch);
            }
        }
        pdf.cmap(cmap_id, &cmap.finish());
        pdf.stream(file_id, &subset).pair(Name(b"Length1"), subset.len() as i32);
    }

    // pages
    let page_ids: Vec<(Ref, Ref)> = pages.iter().map(|_| (next.bump(), next.bump())).collect();
    pdf.catalog(catalog_id).pages(tree_id);
    pdf.pages(tree_id).kids(page_ids.iter().map(|(p, _)| *p)).count(page_ids.len() as i32);
    for (ops, (page_id, content_id)) in pages.iter().zip(&page_ids) {
        let mut page = pdf.page(*page_id);
        page.parent(tree_id).media_box(Rect::new(0.0, 0.0, PAGE_W, PAGE_H)).contents(*content_id);
        let mut resources = page.resources();
        let mut font_dict = resources.fonts();
        for (i, r) in font_refs.iter().enumerate() {
            if let Some(r) = r {
                font_dict.pair(Name(font_names[i].as_bytes()), *r);
            }
        }
        font_dict.finish();
        resources.finish();
        page.finish();

        let content = content_stream(ops, &remappers, &font_names);
        pdf.stream(*content_id, &content);
    }
    pdf.document_info(info_id).title(TextStr(&labels.title)).creator(TextStr("ExpanderCraft"));
    Ok(pdf.finish())
}

fn content_stream(ops: &[Op], remappers: &[GlyphRemapper], font_names: &[String]) -> Vec<u8> {
    let mut c = Content::new();
    for op in ops {
        match op {
            Op::Rule { x1, x2, y, color, width } => {
                c.set_stroke_rgb(color[0], color[1], color[2]);
                c.set_line_width(*width);
                c.move_to(*x1, PAGE_H - y);
                c.line_to(*x2, PAGE_H - y);
                c.stroke();
            }
            Op::Text { x, baseline, size, color, glyphs } => {
                c.set_fill_rgb(color[0], color[1], color[2]);
                c.begin_text();
                let mut pen = *x;
                // one run per consecutive glyphs of the same font
                for run in glyphs.chunk_by(|a, b| a.font == b.font) {
                    let font = run[0].font;
                    let mut bytes = Vec::with_capacity(run.len() * 2);
                    for g in run {
                        let id = remappers[font].get(g.id).unwrap_or(0);
                        bytes.extend_from_slice(&id.to_be_bytes());
                    }
                    c.set_font(Name(font_names[font].as_bytes()), *size);
                    c.set_text_matrix([1.0, 0.0, 0.0, 1.0, pen, PAGE_H - baseline]);
                    c.show(Str(&bytes));
                    pen += Fonts::width(run, *size);
                }
                c.end_text();
            }
        }
    }
    c.finish().to_vec()
}

/// Write the cheat sheet to a PDF file.
pub fn write_pdf(path: &std::path::Path, sections: &[Section], labels: &Labels) -> Result<()> {
    let data = pdf(sections, labels)?;
    std::fs::write(path, data).with_context(|| tr!("error-write-file", path = path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> Labels {
        Labels {
            title: "ExpanderCraft — Cheat sheet".into(),
            subtitle: "3 combos · 2 groups".into(),
            name: "Name".into(),
            keyword: "Keyword".into(),
            snippet: "Snippet".into(),
            disabled_note: "Disabled combos are shown in grey.".into(),
            page: Box::new(|n, total| format!("Page {n} of {total}")),
        }
    }

    fn entry(name: &str, keyword: &str, snippet: &str) -> Entry {
        Entry { name: name.into(), keyword: keyword.into(), snippet: snippet.into(), enabled: true }
    }

    #[test]
    fn wraps_words_and_keeps_line_breaks() {
        let fonts = Fonts::load().unwrap();
        let text = |l: &Vec<Glyph>| l.iter().map(|g| g.ch).collect::<String>();
        let lines = fonts.wrap("uno due tre quattro cinque sei sette otto", Style::Sans, 10.0, 60.0);
        assert!(lines.len() > 2);
        assert!(lines.iter().all(|l| Fonts::width(l, 10.0) <= 60.0 + 0.01));
        assert!(lines.iter().skip(1).all(|l| !text(l).starts_with(' ')));
        let lines = fonts.wrap("Gentile collega,\n\n  indentato", Style::Sans, 10.0, 400.0);
        assert_eq!(lines.iter().map(text).collect::<Vec<_>>(), ["Gentile collega,", "", "  indentato"]);
        // a word wider than the column is cut
        assert!(fonts.wrap(&"x".repeat(200), Style::Mono, 9.0, 50.0).len() > 3);
    }

    #[test]
    fn every_character_gets_a_glyph() {
        let fonts = Fonts::load().unwrap();
        for (text, style) in [("Città è più → ✔ ★ € 😄 «ok»", Style::Sans), ("#{dateTime:dd/MM}", Style::Mono)] {
            let glyphs = fonts.shape(text, style);
            assert_eq!(glyphs.iter().map(|g| g.ch).collect::<String>(), text);
            assert!(glyphs.iter().all(|g| g.id != 0), "{text}");
        }
        // characters missing from every font are replaced, not dropped
        assert_eq!(fonts.shape("a✓b", Style::Sans).iter().map(|g| g.ch).collect::<String>(), "a?b");
        // the emoji comes from Noto Emoji
        assert_eq!(fonts.shape("😄", Style::Sans)[0].font, NOTO_EMOJI);
    }

    /// `EC_PDF_SAMPLE=path cargo test sample` writes an example document, to look at it.
    #[test]
    fn sample() {
        let Ok(path) = std::env::var("EC_PDF_SAMPLE") else { return };
        let mut disabled = entry("Vecchia firma", "vfirma", "Saluti,\nMario Rossi");
        disabled.enabled = false;
        let sections = vec![
            Section {
                name: "Email".into(),
                enabled: true,
                combos: vec![
                    entry("Data di oggi", "ddata", "#{dateTime:dddd d MMMM yyyy}"),
                    entry(
                        "Firma email",
                        "ffirma",
                        "Cordiali saluti,\nMario Rossi\nAzienda Esempio S.r.l.",
                    ),
                    disabled,
                ],
            },
            Section {
                name: "Università".into(),
                enabled: true,
                combos: vec![
                    entry("Indirizzo ufficio", "lluff", "Studio Esempio\nVia Roma 1 — 00100 Roma"),
                    entry(
                        "Risposta ricevimento",
                        "rric",
                        "Gentile #{input:Nome studente},\n\nil ricevimento si tiene il martedì alle 11, nella stanza 3 del Centro. Per esigenze particolari può scrivermi indicando due o tre date possibili: le risponderò appena possibile.\n\nCordiali saluti",
                    ),
                    entry("Solo dal selettore", "", "Testo inserito solo dal selettore delle combo 😄"),
                ],
            },
            Section { name: "Simboli".into(), enabled: false, combos: vec![entry("Freccia", "==>", "→")] },
        ];
        let mut l = labels();
        l.title = "ExpanderCraft — Tabella riassuntiva".into();
        l.subtitle = "7 combo · 3 gruppi · 10 novembre 2026".into();
        l.name = "Nome".into();
        l.keyword = "Parola chiave".into();
        l.snippet = "Testo".into();
        l.disabled_note = "Le combo e i gruppi disattivati sono in grigio.".into();
        l.page = Box::new(|n, total| format!("Pagina {n} di {total}"));
        std::fs::write(path, pdf(&sections, &l).unwrap()).unwrap();
    }

    #[test]
    fn long_lists_span_several_pages() {
        let combos: Vec<Entry> = (0..120).map(|i| entry(&format!("Combo {i}"), &format!("kw{i}"), "Testo breve\nsu due righe")).collect();
        let sections = vec![
            Section { name: "Email".into(), enabled: true, combos },
            Section { name: "Vuoto".into(), enabled: true, combos: Vec::new() },
            Section { name: "Long".into(), enabled: false, combos: vec![entry("Long", "", &"riga\n".repeat(150))] },
        ];
        let fonts = Fonts::load().unwrap();
        let pages = layout(&fonts, &sections, &labels());
        assert!(pages.len() >= 5, "{} pages", pages.len());
        // nothing is drawn below the bottom margin, except the footer
        for page in &pages {
            for op in page {
                if let Op::Text { baseline, size, .. } = op
                    && *size != 7.5
                {
                    assert!(*baseline <= BOTTOM + 1.0);
                }
            }
        }
        let pdf = pdf(&sections, &labels()).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        // the fonts are subsetted: far smaller than the original files
        assert!(pdf.len() < 200_000, "{} bytes", pdf.len());
    }
}
