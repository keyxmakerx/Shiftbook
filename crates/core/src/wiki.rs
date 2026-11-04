//! The wiki (round 18, card r18-wiki): pages you write once and find again, beside your notes.
//!
//! A wiki page is a note of the Page kind; MOP templates show in the wiki too. Pages sit in
//! **spaces** (Network, Servers, your own), and a page can sit under another page, up to
//! [`MAX_DEPTH`] deep. A space is the thing that gets shared once the hosted version exists.
//!
//! `[[Page title]]` in any note, notepad or page links to that page (matched ignoring case);
//! each page lists what links to it. Saving a page keeps its text from before as an earlier
//! version, at most one every [`VERSION_GAP_MINUTES`] while you type, and the newest
//! [`MAX_VERSIONS`] of them. A space can be written out as Markdown files, one per page,
//! with pages under pages in folders.

use std::collections::{HashMap, HashSet};

use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::error::CoreError;
use crate::mop::bracket_spans;

/// Most spaces.
pub const MAX_SPACES: usize = 40;
/// Longest space name.
pub const MAX_SPACE_CHARS: usize = 60;
/// How deep pages go: a page, a page under it, … this many levels.
pub const MAX_DEPTH: usize = 4;
/// Earlier versions kept per page.
pub const MAX_VERSIONS: usize = 30;
/// While typing, at most one earlier version this often.
pub const VERSION_GAP_MINUTES: i64 = 10;
/// The longest file name (without `.md`) a page gets when a space is written out.
const FILE_NAME_CHARS: usize = 80;

/// A space name, trimmed: not empty, not too long, one line.
pub fn space_name(name: &str) -> Result<String, CoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::Required {
            field: "Space name",
        });
    }
    if name.chars().count() > MAX_SPACE_CHARS {
        return Err(CoreError::TooLong {
            field: "Space name",
            max: MAX_SPACE_CHARS,
        });
    }
    if name.contains(['\n', '\r']) {
        return Err(CoreError::SettingInvalid {
            value: name.chars().take(30).collect(),
        });
    }
    Ok(name.to_string())
}

/// The pages `text` links to with `[[Title]]`, in the order they first appear, each once
/// (ignoring case).
pub fn links(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (_, _, title) in bracket_spans(text, "[[", "]]") {
        if !out.iter().any(|t| t.eq_ignore_ascii_case(title)) {
            out.push(title.to_string());
        }
    }
    out
}

/// True when `text` links to the page called `title`.
pub fn links_to(text: &str, title: &str) -> bool {
    let title = title.trim();
    !title.is_empty()
        && bracket_spans(text, "[[", "]]")
            .iter()
            .any(|(_, _, t)| t.eq_ignore_ascii_case(title))
}

/// Where a page sits: its id, the page it's under (if any) and its place among its siblings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    /// The page (its note id).
    pub id: Uuid,
    /// The page it's under, if any.
    pub parent: Option<Uuid>,
    /// Its place among its siblings.
    pub position: i64,
}

/// One row of a space's tree, in the order shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeRow {
    /// The page.
    pub id: Uuid,
    /// 0 for a page at the top of its space.
    pub depth: usize,
}

/// A space's pages as a tree, depth first, siblings by position (then by `order` given, so
/// callers pass pages sorted by title). A page whose parent isn't in this space, or that
/// would loop, sits at the top.
pub fn tree(pages: &[Placed]) -> Vec<TreeRow> {
    let ids: HashSet<Uuid> = pages.iter().map(|p| p.id).collect();
    let parent_of = |p: &Placed| p.parent.filter(|q| ids.contains(q) && *q != p.id);
    let mut children: HashMap<Option<Uuid>, Vec<(i64, usize, Uuid)>> = HashMap::new();
    for (i, p) in pages.iter().enumerate() {
        children
            .entry(parent_of(p))
            .or_default()
            .push((p.position, i, p.id));
    }
    for list in children.values_mut() {
        list.sort();
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut stack: Vec<(Uuid, usize)> = children
        .get(&None)
        .map(|l| l.iter().rev().map(|(_, _, id)| (*id, 0)).collect())
        .unwrap_or_default();
    while let Some((id, depth)) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        out.push(TreeRow { id, depth });
        if let Some(kids) = children.get(&Some(id)) {
            stack.extend(kids.iter().rev().map(|(_, _, k)| (*k, depth + 1)));
        }
    }
    // Pages caught in a loop (a under b under a) never hang from the top: show them there.
    for p in pages {
        if seen.insert(p.id) {
            out.push(TreeRow { id: p.id, depth: 0 });
        }
    }
    out
}

/// Check moving `page` under `parent` (`None` for the top of its space): not under itself
/// or anything under it, and no deeper than [`MAX_DEPTH`] with everything under it.
pub fn check_move(pages: &[Placed], page: Uuid, parent: Option<Uuid>) -> Result<(), CoreError> {
    let Some(parent) = parent else {
        return Ok(());
    };
    let parent_of: HashMap<Uuid, Option<Uuid>> = pages.iter().map(|p| (p.id, p.parent)).collect();
    // How deep the new parent sits (1 at the top), refusing a loop back to `page`.
    let mut depth = 1;
    let mut at = Some(parent);
    while let Some(id) = at {
        if id == page {
            return Err(CoreError::NotAChoice {
                value: "a page under itself".into(),
            });
        }
        at = parent_of.get(&id).copied().flatten();
        depth += 1;
        if depth > pages.len() + 1 {
            break;
        }
    }
    // How many levels hang under `page` itself.
    let below = levels_below(pages, page);
    if depth + below > MAX_DEPTH {
        return Err(CoreError::TooManyItems {
            field: "levels of pages",
            max: MAX_DEPTH,
        });
    }
    Ok(())
}

fn levels_below(pages: &[Placed], page: Uuid) -> usize {
    let mut level = 0;
    let mut current = vec![page];
    let mut seen: HashSet<Uuid> = HashSet::from([page]);
    while !current.is_empty() {
        let next: Vec<Uuid> = pages
            .iter()
            .filter(|p| p.parent.is_some_and(|q| current.contains(&q)) && seen.insert(p.id))
            .map(|p| p.id)
            .collect();
        if next.is_empty() {
            break;
        }
        level += 1;
        current = next;
    }
    level
}

/// Whether saving should keep the text from before as an earlier version: when there's
/// none yet, or the newest is at least [`VERSION_GAP_MINUTES`] old, and the text changed.
pub fn version_due(newest: Option<OffsetDateTime>, now: OffsetDateTime, changed: bool) -> bool {
    changed && newest.is_none_or(|at| now - at >= Duration::minutes(VERSION_GAP_MINUTES))
}

/// A page title as a file name: characters no system allows become `-`, runs of spaces
/// or of dashes fold to one, and it never starts with a dot. Empty becomes "Page".
pub fn file_name(title: &str) -> String {
    let mut out = String::new();
    for c in title.trim().chars() {
        let c = if c.is_control() || r#"<>:"/\|?*"#.contains(c) {
            '-'
        } else {
            c
        };
        let last = out.chars().last();
        if (c == ' ' || c == '-') && last == Some(c) {
            continue;
        }
        out.push(c);
    }
    let out: String = out
        .trim_matches(|c: char| c == ' ' || c == '-' || c == '.')
        .chars()
        .take(FILE_NAME_CHARS)
        .collect();
    let out = out.trim_end().to_string();
    if out.is_empty() {
        "Page".into()
    } else {
        out
    }
}

/// Where each page goes when a space is written out: `Title.md` at the top, and a page with
/// pages under it as `Title.md` plus a folder `Title/` holding them. Names that would clash
/// in one folder get ` (2)`, ` (3)`…; Windows ignores case, so the check does too.
/// `titles` pairs each page id with its title; the result is in tree order.
pub fn export_paths(pages: &[Placed], titles: &HashMap<Uuid, String>) -> Vec<(Uuid, String)> {
    let rows = tree(pages);
    let parent_of: HashMap<Uuid, Option<Uuid>> = pages.iter().map(|p| (p.id, p.parent)).collect();
    let mut folder_of: HashMap<Uuid, String> = HashMap::new();
    let mut taken: HashMap<String, HashSet<String>> = HashMap::new();
    let mut out = Vec::new();
    for row in rows {
        let folder = if row.depth == 0 {
            String::new()
        } else {
            parent_of
                .get(&row.id)
                .copied()
                .flatten()
                .and_then(|p| folder_of.get(&p).cloned())
                .unwrap_or_default()
        };
        let base = file_name(titles.get(&row.id).map(String::as_str).unwrap_or(""));
        let used = taken.entry(folder.clone()).or_default();
        let mut name = base.clone();
        let mut n = 2;
        while !used.insert(name.to_lowercase()) {
            name = format!("{base} ({n})");
            n += 1;
        }
        let path = if folder.is_empty() {
            name.clone()
        } else {
            format!("{folder}/{name}")
        };
        folder_of.insert(row.id, path.clone());
        out.push((row.id, format!("{path}.md")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn placed(id: Uuid, parent: Option<Uuid>, position: i64) -> Placed {
        Placed {
            id,
            parent,
            position,
        }
    }

    #[test]
    fn space_names_are_checked() {
        assert_eq!(space_name("  Network ").unwrap(), "Network");
        assert!(space_name("  ").is_err());
        assert!(space_name("a\nb").is_err());
        assert!(space_name(&"x".repeat(MAX_SPACE_CHARS + 1)).is_err());
    }

    #[test]
    fn links_are_found_once_each() {
        let text = "See [[VPN: common fixes]] and [[ vpn: COMMON fixes ]].\n\
                    Fill {{site}} first, then [[Printer queues]]; [[]] and [x] aren't links.";
        assert_eq!(links(text), ["VPN: common fixes", "Printer queues"]);
        assert!(links_to(text, "printer QUEUES"));
        assert!(!links_to(text, "Printer"));
        assert!(!links_to(text, " "));
    }

    #[test]
    fn the_tree_is_depth_first_by_position() {
        let (a, b, c, d, e) = (
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        let gone = Uuid::now_v7();
        let pages = [
            placed(b, None, 2),
            placed(a, None, 1),
            placed(c, Some(a), 0),
            placed(d, Some(c), 0),
            placed(e, Some(gone), 0),
        ];
        let rows = tree(&pages);
        let shown: Vec<(Uuid, usize)> = rows.iter().map(|r| (r.id, r.depth)).collect();
        assert_eq!(shown, [(e, 0), (a, 0), (c, 1), (d, 2), (b, 0)]);
        // A loop still shows every page.
        let (x, y) = (Uuid::now_v7(), Uuid::now_v7());
        assert_eq!(
            tree(&[placed(x, Some(y), 0), placed(y, Some(x), 0)]).len(),
            2
        );
    }

    #[test]
    fn moves_never_loop_or_go_too_deep() {
        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::now_v7()).collect();
        // A chain 0 > 1 > 2 > 3, and 4 at the top.
        let pages = [
            placed(ids[0], None, 0),
            placed(ids[1], Some(ids[0]), 0),
            placed(ids[2], Some(ids[1]), 0),
            placed(ids[3], Some(ids[2]), 0),
            placed(ids[4], None, 1),
        ];
        assert!(
            check_move(&pages, ids[0], Some(ids[2])).is_err(),
            "under itself"
        );
        assert!(check_move(&pages, ids[1], Some(ids[1])).is_err());
        assert!(
            check_move(&pages, ids[4], Some(ids[3])).is_err(),
            "a fifth level"
        );
        assert!(check_move(&pages, ids[4], Some(ids[2])).is_ok());
        assert!(
            check_move(&pages, ids[2], Some(ids[4])).is_ok(),
            "2 and 3 under 4"
        );
        assert!(
            check_move(&pages, ids[1], Some(ids[4])).is_ok(),
            "1, 2, 3 under 4: four levels"
        );
        assert!(
            check_move(&pages, ids[0], Some(ids[4])).is_err(),
            "0 to 3 under 4: five"
        );
        assert!(check_move(&pages, ids[3], None).is_ok());
    }

    #[test]
    fn versions_wait_between_saves() {
        let now = datetime!(2026-11-04 10:00 UTC);
        assert!(version_due(None, now, true));
        assert!(!version_due(None, now, false));
        assert!(!version_due(Some(now - Duration::minutes(3)), now, true));
        assert!(version_due(Some(now - Duration::minutes(10)), now, true));
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("VPN: common fixes"), "VPN- common fixes");
        assert_eq!(file_name("  ..hidden / page?  "), "hidden - page");
        assert_eq!(file_name("***"), "Page");
        assert_eq!(file_name(&"x".repeat(200)).chars().count(), FILE_NAME_CHARS);
    }

    #[test]
    fn export_paths_nest_and_never_clash() {
        let (a, b, c, d) = (
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        let pages = [
            placed(a, None, 0),
            placed(b, Some(a), 0),
            placed(c, Some(a), 1),
            placed(d, None, 1),
        ];
        let titles = HashMap::from([
            (a, "Switches".to_string()),
            (b, "Stack swap".to_string()),
            (c, "stack SWAP".to_string()),
            (d, "switches".to_string()),
        ]);
        let paths: Vec<String> = export_paths(&pages, &titles)
            .into_iter()
            .map(|(_, p)| p)
            .collect();
        assert_eq!(
            paths,
            [
                "Switches.md",
                "Switches/Stack swap.md",
                "Switches/stack SWAP (2).md",
                "switches (2).md"
            ]
        );
    }
}
