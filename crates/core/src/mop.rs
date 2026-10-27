//! The MOP notebook (round 16, card r15-mopbook): `[[blanks]]` in templates, ticking a MOP
//! off as a checklist, MOP files brought in from disk, and where the notebook window is.
//!
//! A template is a MOP with blanks such as `[[site]]` or `[[ticket]]`. "New MOP from this
//! template" asks for each blank once and fills it in everywhere; a blank left empty stays
//! as `[[name]]` so it is easy to spot later.
//!
//! In Checklist mode any line with something on it can be ticked. Ticking a line crosses it
//! out along with the section under it: everything after it that sits deeper, until the
//! next line at the same depth or shallower. Headings (`#`, `##`…) are shallowest, then
//! numbered steps and bullets (deeper when indented, `1.2` deeper than `1`), then plain text.
//! Ticks are stored by the line's text (and which repeat of it), not by line number, so
//! adding a line above doesn't move them.

use serde::Serialize;
use uuid::Uuid;

use crate::error::{preview, CoreError};
use crate::notes::{validate_body, MAX_TITLE_CHARS};
use crate::windows::{WindowMode, MAX_OFFSET, MAX_WINDOW, MIN_WINDOW};

/// Longest blank name inside `[[ ]]`.
pub const MAX_BLANK_CHARS: usize = 60;
/// Most different blanks in one template.
pub const MAX_BLANKS: usize = 50;
/// Longest value typed into one blank.
pub const MAX_VALUE_CHARS: usize = 500;
/// Most ticks kept on one MOP.
pub const MAX_TICKS: usize = 2000;
/// Longest piece of line text a tick is stored by.
const TICK_TEXT_CHARS: usize = 200;
/// Title for a dropped file whose name has nothing usable in it.
pub const IMPORTED_TITLE: &str = "Imported MOP";
/// Most MOPs open as tabs at once.
pub const MAX_TABS: usize = 20;

/// The `[[blanks]]` in a template, in the order they first appear. Names are trimmed and
/// matched ignoring case (the first spelling wins); text in `[[ ]]` that is empty, too
/// long or spans lines isn't a blank.
pub fn blanks(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (_, _, name) in blank_spans(text) {
        if out.len() < MAX_BLANKS && !out.iter().any(|b| b.eq_ignore_ascii_case(name)) {
            out.push(name.to_string());
        }
    }
    out
}

/// Each blank in `text`: its byte start and end (covering the brackets) and trimmed name.
fn blank_spans(text: &str) -> Vec<(usize, usize, &str)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(open) = text[from..].find("[[").map(|i| from + i) {
        let inner_start = open + 2;
        let Some(close) = text[inner_start..].find("]]").map(|i| inner_start + i) else {
            break;
        };
        let inner = &text[inner_start..close];
        // `[[a [[b]]`: the blank is the innermost `[[b]]`.
        if let Some(again) = inner.rfind("[[") {
            from = inner_start + again;
            continue;
        }
        let name = inner.trim();
        if !name.is_empty()
            && !inner.contains('\n')
            && !inner.contains('[')
            && name.chars().count() <= MAX_BLANK_CHARS
        {
            out.push((open, close + 2, name));
        }
        from = close + 2;
    }
    out
}

/// Check what was typed into the blanks: one line each, not too long, at most
/// [`MAX_BLANKS`]. Returns them trimmed.
pub fn clean_values(values: &[(String, String)]) -> Result<Vec<(String, String)>, CoreError> {
    if values.len() > MAX_BLANKS {
        return Err(CoreError::TooManyItems {
            field: "blanks",
            max: MAX_BLANKS,
        });
    }
    values
        .iter()
        .map(|(name, value)| {
            let value = value.trim();
            if value.chars().count() > MAX_VALUE_CHARS {
                return Err(CoreError::TooLong {
                    field: "What's typed in a blank",
                    max: MAX_VALUE_CHARS,
                });
            }
            if value.contains(['\n', '\r']) {
                return Err(CoreError::SettingInvalid {
                    value: preview(value, 30),
                });
            }
            Ok((name.trim().to_string(), value.to_string()))
        })
        .collect()
}

/// Fill the blanks: every `[[name]]` with a value (matched ignoring case) is replaced by it,
/// everywhere. Blanks without a value, or with an empty one, stay as they are.
pub fn fill_blanks(text: &str, values: &[(String, String)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (start, end, name) in blank_spans(text) {
        let value = values
            .iter()
            .find(|(n, _)| n.trim().eq_ignore_ascii_case(name))
            .map(|(_, v)| v.trim())
            .filter(|v| !v.is_empty());
        if let Some(value) = value {
            out.push_str(&text[at..start]);
            out.push_str(value);
            at = end;
        }
    }
    out.push_str(&text[at..]);
    out
}

/// The title for a MOP made from a template: the typed one, else the template's own title
/// with its blanks filled in.
pub fn filled_title(typed: &str, template_title: &str, values: &[(String, String)]) -> String {
    let typed = typed.trim();
    let title = if typed.is_empty() {
        fill_blanks(template_title, values)
    } else {
        typed.to_string()
    };
    title.chars().take(MAX_TITLE_CHARS).collect()
}

/// How deep a line sits, for Checklist mode. Smaller is shallower; `None` for a blank line.
fn depth(line: &str) -> Option<u32> {
    let trimmed = line.trim_start();
    if trimmed.trim().is_empty() {
        return None;
    }
    let indent: u32 = line[..line.len() - trimmed.len()]
        .chars()
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum();
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with([' ', '\t']) {
        return Some(u32::try_from(hashes).unwrap_or(6));
    }
    if let Some(parts) = step_number(trimmed) {
        return Some(10 + indent * 2 + (parts - 1) * 2);
    }
    if is_bullet(trimmed) {
        return Some(10 + indent * 2 + 1);
    }
    Some(200 + indent)
}

/// `1.`, `1)`, `2.3`, `2.3.` or `Step 4` at the start of a line, followed by a space: how
/// many parts the number has.
fn step_number(text: &str) -> Option<u32> {
    let text = match text.get(..5) {
        Some(word) if word.eq_ignore_ascii_case("step ") => {
            let rest = text[5..].trim_start();
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            return (digits > 0).then_some(1);
        }
        _ => text,
    };
    let mut parts = 0;
    let mut rest = text;
    loop {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 || digits > 4 {
            return None;
        }
        parts += 1;
        rest = &rest[digits..];
        match rest.chars().next() {
            Some('.') if rest[1..].starts_with(|c: char| c.is_ascii_digit()) => rest = &rest[1..],
            Some('.' | ')' | ':') if rest[1..].is_empty() || rest[1..].starts_with([' ', '\t']) => {
                return Some(parts)
            }
            Some(' ' | '\t') if parts > 1 => return Some(parts),
            _ => return None,
        }
    }
}

fn is_bullet(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some('-' | '*' | '•' | '+')) && matches!(chars.next(), Some(' ' | '\t'))
}

/// The text a tick is stored by: spaces squeezed, ignoring case, shortened.
fn tick_text(line: &str) -> String {
    line.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
        .chars()
        .take(TICK_TEXT_CHARS)
        .collect()
}

/// One line of a MOP in Checklist mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckLine {
    /// What its tick is stored by; empty for a blank line (which can't be ticked).
    pub key: String,
    /// Ticked itself.
    pub ticked: bool,
    /// Crossed out: ticked, or under a ticked line.
    pub crossed: bool,
}

/// The tick keys of each line, in order (`""` for blank lines). A line's key is its text
/// and which repeat of that text it is, so two identical "Verify" lines tick separately.
pub fn tick_keys(text: &str) -> Vec<String> {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    text.split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                return String::new();
            }
            let t = tick_text(line);
            let n = seen.entry(t.clone()).or_insert(0);
            *n += 1;
            format!("{}|{t}", *n)
        })
        .collect()
}

/// Every line of `text` (split on `\n`) with its tick and whether it is crossed out.
pub fn checklist(text: &str, ticks: &[String]) -> Vec<CheckLine> {
    let keys = tick_keys(text);
    // The depth of the shallowest ticked line still covering what follows.
    let mut covering: Option<u32> = None;
    text.split('\n')
        .zip(keys)
        .map(|(line, key)| {
            let Some(d) = depth(line) else {
                return CheckLine {
                    key,
                    ticked: false,
                    crossed: false,
                };
            };
            if covering.is_some_and(|c| d <= c) {
                covering = None;
            }
            let ticked = ticks.contains(&key);
            if ticked && covering.is_none() {
                covering = Some(d);
            }
            CheckLine {
                crossed: ticked || covering.is_some(),
                ticked,
                key,
            }
        })
        .collect()
}

/// The ticks worth keeping for `text`: ones that still match a line, without repeats, at
/// most [`MAX_TICKS`], in line order.
pub fn clean_ticks(text: &str, ticks: &[String]) -> Vec<String> {
    tick_keys(text)
        .into_iter()
        .filter(|k| !k.is_empty() && ticks.contains(k))
        .take(MAX_TICKS)
        .collect()
}

/// Ticks as stored in `notes.ticks`: one per line (keys never hold a line break).
pub fn join_ticks(ticks: &[String]) -> String {
    ticks.join("\n")
}

/// Ticks read back from `notes.ticks`.
pub fn parse_ticks(text: &str) -> Vec<String> {
    text.split('\n')
        .filter(|t| !t.is_empty())
        .take(MAX_TICKS)
        .map(str::to_string)
        .collect()
}

/// The title for a MOP brought in from a file: the file name without its folder or
/// extension, else "Imported MOP".
pub fn import_title(file_name: &str) -> String {
    let name = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let stem = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.trim().is_empty() && ext.len() <= 5 => stem,
        _ => name,
    };
    let title = stem.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        IMPORTED_TITLE.to_string()
    } else {
        title.chars().take(MAX_TITLE_CHARS).collect()
    }
}

/// A safe file name for saving a MOP as a text file: its title with characters that file
/// systems refuse swapped for `-`, plus `.txt`.
pub fn file_name_for(title: &str) -> String {
    let name: String = title
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '-'
            } else {
                c
            }
        })
        .take(80)
        .collect();
    let name = name.trim().trim_matches('.').trim();
    if name.is_empty() {
        "MOP.txt".to_string()
    } else {
        format!("{name}.txt")
    }
}

/// Why a dropped file can't come in.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportProblem {
    /// A Word file (or another zip): it isn't plain text.
    #[error(
        "Word files can't be read yet. In Word, use Save As, Plain Text (.txt), then drop that in."
    )]
    Word,
    /// Not text at all (a picture, a program…).
    #[error("This doesn't look like a text file.")]
    NotText,
    /// Too long for one note.
    #[error("{0}")]
    Invalid(#[from] CoreError),
}

/// The text of a dropped file, ready to keep as a MOP: a leading byte-order mark dropped and
/// Windows or old Mac line endings turned into plain ones.
pub fn import_text(text: &str) -> Result<String, ImportProblem> {
    if text.starts_with("PK\u{3}\u{4}") {
        return Err(ImportProblem::Word);
    }
    if text.contains('\0') {
        return Err(ImportProblem::NotText);
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    validate_body(&text)?;
    Ok(text)
}

/// How the notebook shows a MOP.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MopView {
    /// Like Notepad++: line numbers, colours, counts.
    #[default]
    Numbered,
    /// Just the text, nothing around it.
    Minimal,
    /// Tick lines off; a ticked line crosses out its section.
    Checklist,
}

impl MopView {
    /// The stored word.
    pub const fn as_str(self) -> &'static str {
        match self {
            MopView::Numbered => "numbered",
            MopView::Minimal => "minimal",
            MopView::Checklist => "checklist",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "numbered" => Some(MopView::Numbered),
            "minimal" => Some(MopView::Minimal),
            "checklist" => Some(MopView::Checklist),
            _ => None,
        }
    }
}

/// The notebook window: whether it's showing, where, which MOPs are open as tabs, and how
/// it shows them. Kept as one preference ([`crate::prefs::KEY_MOPBOOK`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MopBook {
    /// Showing (in any mode); false once closed.
    pub shown: bool,
    /// Floating, rolled up, minimised or full screen.
    pub mode: WindowMode,
    /// Left.
    pub x: i32,
    /// Top.
    pub y: i32,
    /// Width when floating.
    pub w: i32,
    /// Height when floating.
    pub h: i32,
    /// Numbered, Minimal or Checklist.
    pub view: MopView,
    /// Line numbers showing.
    pub numbers: bool,
    /// The MOPs open as tabs, in order.
    pub tabs: Vec<Uuid>,
    /// The tab in front.
    pub active: Option<Uuid>,
}

impl Default for MopBook {
    fn default() -> Self {
        Self {
            shown: false,
            mode: WindowMode::Open,
            x: 120,
            y: 60,
            w: 640,
            h: 460,
            view: MopView::Numbered,
            numbers: true,
            tabs: Vec::new(),
            active: None,
        }
    }
}

impl MopBook {
    /// Read the stored form `shown mode x y w h view numbers active tabs` (tabs comma
    /// separated, `-` for none). Anything broken gives the default.
    pub fn parse(text: &str) -> Self {
        Self::parse_parts(text).unwrap_or_default()
    }

    fn parse_parts(text: &str) -> Option<Self> {
        let mut bits = text.split_whitespace();
        let shown = bits.next()? == "1";
        let mode = WindowMode::parse(bits.next()?)?;
        let mut n = || bits.next()?.parse::<i32>().ok();
        let (x, y, w, h) = (n()?, n()?, n()?, n()?);
        let view = MopView::parse(bits.next()?)?;
        let numbers = bits.next()? == "1";
        let active = Uuid::parse_str(bits.next()?).ok();
        let mut tabs: Vec<Uuid> = Vec::new();
        for id in bits.next().unwrap_or("-").split(',') {
            if let Ok(id) = Uuid::parse_str(id) {
                if tabs.len() < MAX_TABS && !tabs.contains(&id) {
                    tabs.push(id);
                }
            }
        }
        let active = active
            .filter(|a| tabs.contains(a))
            .or(tabs.first().copied());
        Some(Self {
            shown,
            mode,
            x: x.clamp(-MAX_OFFSET, MAX_OFFSET),
            y: y.clamp(0, MAX_OFFSET),
            w: w.clamp(MIN_WINDOW.0, MAX_WINDOW.0),
            h: h.clamp(MIN_WINDOW.1, MAX_WINDOW.1),
            view,
            numbers,
            tabs,
            active,
        })
    }

    /// The stored form.
    pub fn format(&self) -> String {
        let tabs: Vec<String> = self
            .tabs
            .iter()
            .take(MAX_TABS)
            .map(Uuid::to_string)
            .collect();
        format!(
            "{} {} {} {} {} {} {} {} {} {}",
            u8::from(self.shown),
            self.mode.as_str(),
            self.x,
            self.y,
            self.w,
            self.h,
            self.view.as_str(),
            u8::from(self.numbers),
            self.active
                .map_or_else(|| "-".to_string(), |a| a.to_string()),
            if tabs.is_empty() {
                "-".to_string()
            } else {
                tabs.join(",")
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
            .collect()
    }

    #[test]
    fn blanks_are_found_once_in_order() {
        let text = "Change on [[ Site ]] for [[ticket]]\nReboot [[site]] then [[]] and [[a\nb]] \
                    and [[outer [[inner]]";
        assert_eq!(blanks(text), ["Site", "ticket", "inner"]);
        assert!(blanks("no blanks [here] or [[ ]]").is_empty());
        let long = format!("[[{}]]", "x".repeat(MAX_BLANK_CHARS + 1));
        assert!(blanks(&long).is_empty());
    }

    #[test]
    fn filling_replaces_every_copy_and_leaves_empty_ones() {
        let text = "# [[Site]] change\n1. Log in to [[site]]\n2. Note [[ticket]]\n";
        let out = fill_blanks(text, &v(&[("SITE", " North hall "), ("ticket", "  ")]));
        assert_eq!(
            out,
            "# North hall change\n1. Log in to North hall\n2. Note [[ticket]]\n"
        );
        assert_eq!(fill_blanks("plain", &v(&[("a", "b")])), "plain");
        assert_eq!(
            filled_title("", "MOP [[site]]", &v(&[("site", "A")])),
            "MOP A"
        );
        assert_eq!(filled_title(" Mine ", "MOP [[site]]", &[]), "Mine");
    }

    #[test]
    fn values_are_checked() {
        assert_eq!(
            clean_values(&v(&[(" a ", " b ")])).unwrap(),
            v(&[("a", "b")])
        );
        assert!(clean_values(&v(&[("a", "two\nlines")])).is_err());
        let long = "x".repeat(MAX_VALUE_CHARS + 1);
        assert!(clean_values(&[("a".into(), long)]).is_err());
    }

    #[test]
    fn depths_follow_headings_steps_and_indents() {
        assert_eq!(depth("# Prep"), Some(1));
        assert_eq!(depth("## Backout"), Some(2));
        assert_eq!(depth("#hashtag"), Some(200));
        assert_eq!(depth("1. Do it"), Some(10));
        assert_eq!(depth("Step 3 do it"), Some(10));
        assert_eq!(depth("2.1 Sub step"), Some(12));
        assert_eq!(depth("  - detail"), Some(15));
        assert_eq!(depth("- detail"), Some(11));
        assert_eq!(depth("10) Go"), Some(10));
        assert_eq!(depth("2026 was a year"), Some(200));
        assert_eq!(depth("plain"), Some(200));
        assert_eq!(depth("   "), None);
    }

    #[test]
    fn ticking_a_heading_crosses_out_its_section() {
        let text = "# Prep\n1. Back up\n   Verify: done\n\n# Change\n1. Apply";
        let keys = tick_keys(text);
        assert_eq!(keys[0], "1|# prep");
        assert_eq!(keys[3], "");
        // The second "1." step has a different text, so its own key.
        let lines = checklist(text, &[keys[0].clone()]);
        let crossed: Vec<bool> = lines.iter().map(|l| l.crossed).collect();
        assert_eq!(crossed, [true, true, true, false, false, false]);
        assert!(lines[0].ticked && !lines[1].ticked);
    }

    #[test]
    fn ticking_a_step_crosses_out_what_is_under_it_only() {
        let text = "1. Stop service\n   check it stopped\n   - log it\n2. Start\n2.1 Wait\n3. Done";
        let keys = tick_keys(text);
        let lines = checklist(text, &[keys[0].clone(), keys[3].clone()]);
        let crossed: Vec<bool> = lines.iter().map(|l| l.crossed).collect();
        assert_eq!(crossed, [true, true, true, true, true, false]);
        // Plain text ticks only itself (and anything indented under it).
        let lines = checklist("note a\n  more\nnote b", &tick_keys("note a")[..1]);
        let crossed: Vec<bool> = lines.iter().map(|l| l.crossed).collect();
        assert_eq!(crossed, [true, true, false]);
    }

    #[test]
    fn repeated_lines_tick_separately_and_ticks_survive_edits() {
        let text = "Verify\nVerify";
        let keys = tick_keys(text);
        assert_eq!(keys, ["1|verify", "2|verify"]);
        let lines = checklist(text, &[keys[1].clone()]);
        assert!(!lines[0].crossed && lines[1].crossed);
        // A line added above keeps the tick on the same text.
        let ticks = vec!["1|# prep".to_string()];
        assert!(checklist("new line\n#  Prep", &ticks)[1].ticked);
    }

    #[test]
    fn ticks_are_cleaned_and_round_trip() {
        let text = "a\nb";
        let ticks = vec!["1|b".into(), "1|gone".into(), "1|a".into(), "1|a".into()];
        let clean = clean_ticks(text, &ticks);
        assert_eq!(clean, ["1|a", "1|b"]);
        assert_eq!(parse_ticks(&join_ticks(&clean)), clean);
        assert!(parse_ticks("").is_empty());
    }

    #[test]
    fn import_titles_and_text() {
        assert_eq!(
            import_title("C:\\MOPs\\Core switch  swap.txt"),
            "Core switch swap"
        );
        assert_eq!(
            import_title("/home/a/firewall.change.md"),
            "firewall.change"
        );
        assert_eq!(import_title(".txt"), ".txt");
        assert_eq!(import_title("  "), IMPORTED_TITLE);
        assert_eq!(import_text("\u{feff}a\r\nb\rc").unwrap(), "a\nb\nc");
        assert_eq!(file_name_for("Swap: core/edge?"), "Swap- core-edge-.txt");
        assert_eq!(file_name_for(" .. "), "MOP.txt");
        assert_eq!(import_text("PK\u{3}\u{4}rest"), Err(ImportProblem::Word));
        assert_eq!(import_text("a\0b"), Err(ImportProblem::NotText));
        assert!(import_text(&"x".repeat(crate::notes::MAX_BODY_CHARS + 1)).is_err());
    }

    #[test]
    fn notebook_round_trips_and_stays_in_bounds() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let book = MopBook {
            shown: true,
            mode: WindowMode::Minimised,
            x: 10,
            y: 20,
            w: 700,
            h: 500,
            view: MopView::Checklist,
            numbers: false,
            tabs: vec![a, b],
            active: Some(b),
        };
        assert_eq!(MopBook::parse(&book.format()), book);
        assert_eq!(MopBook::parse(""), MopBook::default());
        assert_eq!(MopBook::parse("junk"), MopBook::default());
        let odd = MopBook::parse(&format!("0 full -99999 -4 1 1 minimal 1 {a} {b},{b},bad"));
        assert_eq!(odd.tabs, [b]);
        assert_eq!(odd.active, Some(b));
        assert_eq!((odd.x, odd.y, odd.w), (-MAX_OFFSET, 0, MIN_WINDOW.0));
        let none = MopBook::default().format();
        assert_eq!(MopBook::parse(&none), MopBook::default());
    }
}
