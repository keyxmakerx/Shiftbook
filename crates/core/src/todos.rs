//! To-dos inside notepads (round 21, idea approved 26 October): a line that starts with a
//! tick box is a to-do. Typing `[]` at the start of a line makes the box (`☐`), clicking it
//! ticks it (`☑`); the brackets themselves (`[]`, `[ ]`, `[x]`) count too, so text pasted
//! in or typed where the box isn't made still works. Open ones are counted on cards and
//! listed in the bell.

/// An open to-do's box.
pub const OPEN_BOX: char = '☐';
/// A ticked to-do's box.
pub const DONE_BOX: char = '☑';

/// One to-do line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Todo {
    /// What it says, without the box.
    pub text: String,
    /// Ticked.
    pub done: bool,
}

/// The to-do on `line`, if it is one: a box (or brackets) first, after any indent, then the
/// words. A box with nothing after it isn't a to-do yet.
pub fn todo_on(line: &str) -> Option<Todo> {
    let rest = line.trim_start();
    let open = [OPEN_BOX.to_string(), "[]".into(), "[ ]".into()];
    let ticked = [DONE_BOX.to_string(), "[x]".into(), "[X]".into()];
    let after = |boxes: &[String]| boxes.iter().find_map(|b| rest.strip_prefix(b.as_str()));
    let (done, words) = match after(&open) {
        Some(w) => (false, w),
        None => (true, after(&ticked)?),
    };
    let text = words.trim();
    (!text.is_empty()).then(|| Todo {
        text: text.to_string(),
        done,
    })
}

/// Every to-do in `text`, in order.
pub fn todos(text: &str) -> Vec<Todo> {
    text.lines().filter_map(todo_on).collect()
}

/// The to-dos in `text` that aren't ticked, in order.
pub fn open_todos(text: &str) -> Vec<String> {
    todos(text)
        .into_iter()
        .filter(|t| !t.done)
        .map(|t| t.text)
        .collect()
}

/// How many to-dos in `text` aren't ticked.
pub fn open_count(text: &str) -> u32 {
    u32::try_from(open_todos(text).len()).unwrap_or(u32::MAX)
}

/// The bell's words for a card's open to-dos: "Update the port map" for one, and for more
/// "Update the port map, and 2 more".
pub fn open_words(open: &[String]) -> String {
    match open {
        [] => String::new(),
        [one] => one.clone(),
        [first, rest @ ..] => format!("{first}, and {} more", rest.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_that_starts_with_a_box_is_a_to_do() {
        let pad = "Old switch out, new one racked.\n☑ Patch panel labels\n☐ Update the port map\n  ☐ Tell the floor 2 team it's done";
        let all = todos(pad);
        assert_eq!(all.len(), 3);
        assert!(all[0].done && all[0].text == "Patch panel labels");
        assert_eq!(
            open_todos(pad),
            ["Update the port map", "Tell the floor 2 team it's done"]
        );
        assert_eq!(open_count(pad), 2);
    }

    #[test]
    fn brackets_count_as_boxes_too() {
        assert_eq!(
            open_todos("[] call the vendor\n[ ] order toner"),
            ["call the vendor", "order toner"]
        );
        assert_eq!(open_count("[x] done already\n[X] and this"), 0);
        assert_eq!(todos("[x] done already").len(), 1);
    }

    #[test]
    fn other_lines_are_left_alone() {
        // A box in the middle of a line, an empty box, and a list with brackets later on.
        assert_eq!(
            open_count("see ☐ in the doc\n☐\n[]\n- item [] here\ninterface ge-0/0/1 [up]"),
            0
        );
        assert_eq!(todo_on("[link] to the page"), None);
        assert_eq!(open_count(""), 0);
    }

    #[test]
    fn the_bell_names_the_first_and_counts_the_rest() {
        assert_eq!(open_words(&[]), "");
        assert_eq!(open_words(&["Order toner".into()]), "Order toner");
        assert_eq!(
            open_words(&["A".into(), "B".into(), "C".into()]),
            "A, and 2 more"
        );
    }
}
