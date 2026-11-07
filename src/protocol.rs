//! Events delivered by Fcitx5 to the UIM frontend, in D-Bus wire order.

pub type FormattedText = Vec<(String, i32)>;

#[derive(Debug)]
pub enum Event {
    Error(String),
    Commit(String),
    Preedit {
        text: FormattedText,
        cursor: i32,
    },
    ClientSideUI {
        preedit: FormattedText,
        cursor: i32,
        aux_up: FormattedText,
        aux_down: FormattedText,
        candidates: Vec<(String, String)>,
        candidate_index: i32,
        has_prev: bool,
        has_next: bool,
    },
    ForwardKey {
        keysym: u32,
        state: u32,
        release: bool,
    },
    DeleteSurrounding {
        offset: i32,
        nchars: u32,
    },
    CurrentIM {
        name: String,
        unique_name: String,
    },
    FocusOut,
}

#[derive(Debug)]
pub struct KeyResult {
    pub handled: bool,
    pub events: Vec<Event>,
}
