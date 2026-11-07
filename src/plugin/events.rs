//! Translate Fcitx presentation data into UIM's existing preedit/candidate API.

use crate::keymap::KeyEvent;
use crate::protocol::{Event, FormattedText};
use crate::scheme_ffi::{self as scm, Root};

const FCITX_UNDERLINE: i32 = 1 << 3;
const FCITX_HIGHLIGHT: i32 = 1 << 4;
const UIM_UNDERLINE: i64 = 1;
const UIM_REVERSE: i64 = 2;
const UIM_CURSOR: i64 = 4;
const CTRL: u32 = 1 << 2;
const ALT: u32 = 1 << 3;

pub fn error(message: &str) -> Root {
    scm::list([scm::symbol(c"error"), scm::text(message)])
}

fn segment(attributes: i64, text: &str) -> Root {
    scm::pair(&scm::integer(attributes), &scm::text(text))
}

fn preedit(text: FormattedText, cursor: i32) -> Root {
    let mut segments = Vec::new();
    let mut offset = 0;
    let mut cursor_added = false;
    for (text, flags) in text {
        let attributes = if flags & FCITX_UNDERLINE != 0 {
            UIM_UNDERLINE
        } else {
            0
        } | if flags & FCITX_HIGHLIGHT != 0 {
            UIM_REVERSE
        } else {
            0
        };
        let position = cursor as isize - offset as isize;
        if cursor >= 0
            && !cursor_added
            && position >= 0
            && position <= text.len() as isize
            && text.is_char_boundary(position as usize)
        {
            let (before, after) = text.split_at(position as usize);
            segments.push(segment(attributes, before));
            segments.push(segment(UIM_CURSOR, ""));
            segments.push(segment(attributes, after));
            cursor_added = true;
        } else {
            segments.push(segment(attributes, &text));
        }
        offset += text.len();
    }
    if cursor >= 0 && !cursor_added && cursor as usize == offset {
        segments.push(segment(UIM_CURSOR, ""));
    }
    scm::list(segments)
}

fn plain(text: FormattedText) -> String {
    text.into_iter().map(|(text, _)| text).collect()
}

fn forwarded_text(keysym: u32, state: u32) -> Option<String> {
    let mut codepoint = match keysym {
        0xff08 => 8,
        0xff09 => 9,
        0xff0a => 10,
        0xff0d => 13,
        0xff1b => 27,
        0xffff => 127,
        0x20..=0xff => keysym,
        0x01000100..=0x0110ffff => keysym & 0xffffff,
        _ => return None,
    };
    if state & !(CTRL | ALT | 1 | 2) != 0 {
        return None;
    }
    if state & CTRL != 0 {
        codepoint = match codepoint {
            0x40..=0x5f | 0x60..=0x7f => codepoint & 0x1f,
            0x3f => 127,
            _ => return None,
        };
    }
    // libuim's commit callback uses a NUL-terminated string.
    if codepoint == 0 {
        return None;
    }
    let mut text = String::new();
    if state & ALT != 0 {
        text.push('\x1b');
    }
    text.push(char::from_u32(codepoint)?);
    Some(text)
}

/// Returns presentation events and whether the current original key was forwarded.
pub fn convert(events: Vec<Event>, current: Option<(KeyEvent, bool)>) -> (Root, bool) {
    let mut forwarded = false;
    let events = events.into_iter().filter_map(|event| match event {
        Event::Error(message) => Some(error(&message)),
        Event::Commit(text) => Some(scm::list([scm::symbol(c"commit"), scm::text(&text)])),
        Event::Preedit { text, cursor } => Some(scm::list([
            scm::symbol(c"preedit"), preedit(text, cursor),
        ])),
        Event::ClientSideUI {
            preedit: text, cursor, aux_up, aux_down, candidates,
            candidate_index, has_prev, has_next, ..
        } => {
            let candidates = scm::list(candidates.into_iter().map(|(label, text)| {
                // Fcitx includes label punctuation; UIM adds its own colon.
                let label = label.trim_end();
                let label = label.strip_suffix('.').unwrap_or(label);
                scm::list([scm::text(&text), scm::text(label), scm::text("")])
            }));
            Some(scm::list([
                scm::symbol(c"panel"), preedit(text, cursor),
                scm::text(&plain(aux_up)), scm::text(&plain(aux_down)), candidates,
                scm::integer(candidate_index.into()),
                scm::boolean(has_prev), scm::boolean(has_next),
            ]))
        }
        Event::CurrentIM { name, unique_name, .. } => Some(scm::list([
            scm::symbol(c"mode"), scm::text(&name), scm::text(&unique_name),
        ])),
        Event::FocusOut => Some(scm::list([scm::symbol(c"focus-out")])),
        Event::ForwardKey { keysym, state, release } => {
            if current.is_some_and(|(key, current_release)| {
                key.keysym == keysym && key.state == state && current_release == release
            }) {
                forwarded = true;
                None
            } else if release {
                // A terminal carries bytes, so there is no key-release representation.
                None
            } else if let Some(text) = forwarded_text(keysym, state) {
                Some(scm::list([scm::symbol(c"commit"), scm::text(&text)]))
            } else {
                Some(scm::list([scm::symbol(c"notice"), scm::text(&format!(
                    "Fcitx5 forwarded unsupported terminal key {keysym:#x} with modifiers {state:#x}"
                ))]))
            }
        }
        Event::DeleteSurrounding { offset, nchars } => Some(scm::list([
            scm::symbol(c"notice"), scm::text(&format!(
                "Fcitx5 requested unavailable surrounding-text deletion ({offset}, {nchars})"
            )),
        ])),
    });
    (scm::list(events), forwarded)
}
