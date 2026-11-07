//! Translate UIM Scheme keys into the XKB keysyms and Fcitx modifier mask.
//!
//! Characters and named keys stay distinct: UIM's C special-key enum starts
//! at 256, which overlaps valid Unicode scalars. Scheme already distinguishes
//! them as integers and symbols (uim 1.9.8, uim/uim-key.c).

#[derive(Clone, Copy, Debug)]
pub enum Key<'a> {
    Character(u32),
    Named(&'a str),
}

#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub keysym: u32,
    pub state: u32,
}

pub const F13: u32 = 0xffca;

pub fn translate(key: Key<'_>, state: u32) -> Option<KeyEvent> {
    let (keysym, extra_state) = match key {
        Key::Character(codepoint) => character(codepoint)?,
        Key::Named(name) => (named(name)?, 0),
    };
    Some(KeyEvent {
        keysym,
        state: modifiers(state) | extra_state,
    })
}

// UIM's UKeyModifier and Fcitx's KeyState use different bit assignments.
// Pseudo0/Pseudo1 are UIM predicate flags, not XKB lock modifiers.
fn modifiers(state: u32) -> u32 {
    const MODIFIERS: [(u32, u32); 6] = [
        (1, 1),       // Shift -> Shift
        (2, 4),       // Control -> Ctrl
        (4, 8),       // Alt -> Mod1
        (8, 1 << 28), // Meta -> Meta
        (64, 64),     // Super -> Mod4
        (128, 32),    // Hyper -> Mod3
    ];
    MODIFIERS.into_iter().fold(0, |mapped, (uim, fcitx)| {
        mapped | if state & uim != 0 { fcitx } else { 0 }
    })
}

fn character(codepoint: u32) -> Option<(u32, u32)> {
    char::from_u32(codepoint)?;
    let event = match codepoint {
        0x08 => (0xff08, 0), // BackSpace
        0x09 => (0xff09, 0), // Tab
        0x0a => (0xff0a, 0), // Linefeed
        0x0d => (0xff0d, 0), // Return
        0x1b => (0xff1b, 0), // Escape
        0x7f => (0xffff, 0), // Delete
        // FEP normally expands these before the Scheme handler. Preserve
        // control semantics if a UIM frontend supplies the raw character.
        0 => (u32::from(b' '), 4),
        1..=26 => (codepoint + u32::from(b'a') - 1, 4),
        28..=31 => (codepoint + u32::from(b'A') - 1, 4),
        0x20..=0x7e | 0xa0..=0xff => (codepoint, 0),
        // C1 controls have no printable XKB character mapping.
        0x80..=0x9f => return None,
        _ => (0x0100_0000 | codepoint, 0),
    };
    Some(event)
}

// Exact Scheme spellings from uim 1.9.8 uim/uim-key.c and keysym values
// from fcitx-utils/keysymgen.h. UIM private keys have no XKB counterpart.
fn named(name: &str) -> Option<u32> {
    if let Some(number) = name.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
        if (1..=35).contains(&number) {
            return Some(0xffbe + number - 1);
        }
    }
    Some(match name {
        "yen" => 0x00a5,
        "backspace" => 0xff08,
        "delete" => 0xffff,
        "escape" => 0xff1b,
        "return" => 0xff0d,
        "tab" => 0xff09,
        "left" => 0xff51,
        "up" => 0xff52,
        "right" => 0xff53,
        "down" => 0xff54,
        "prior" => 0xff55,
        "next" => 0xff56,
        "home" => 0xff50,
        "end" => 0xff57,
        "insert" => 0xff63,
        "Multi_key" => 0xff20,
        "codeinput" => 0xff37,
        "single-candidate" => 0xff3c,
        "multiple-candidate" => 0xff3d,
        "previous-candidate" => 0xff3e,
        "Mode_switch" => 0xff7e,
        "Kanji" => 0xff21,
        "Muhenkan" => 0xff22,
        "Henkan_Mode" => 0xff23,
        "romaji" => 0xff24,
        "hiragana" => 0xff25,
        "katakana" => 0xff26,
        "hiragana-katakana" => 0xff27,
        "zenkaku" => 0xff28,
        "hankaku" => 0xff29,
        "zenkaku-hankaku" => 0xff2a,
        "touroku" => 0xff2b,
        "massyo" => 0xff2c,
        "kana-lock" => 0xff2d,
        "kana-shift" => 0xff2e,
        "eisu-shift" => 0xff2f,
        "eisu-toggle" => 0xff30,
        "hangul" => 0xff31,
        "hangul-start" => 0xff32,
        "hangul-end" => 0xff33,
        "hangul-hanja" => 0xff34,
        "hangul-jamo" => 0xff35,
        "hangul-romaja" => 0xff36,
        "hangul-codeinput" => 0xff37,
        "hangul-jeonja" => 0xff38,
        "hangul-banja" => 0xff39,
        "hangul-prehanja" => 0xff3a,
        "hangul-posthanja" => 0xff3b,
        "hangul-single-candidate" => 0xff3c,
        "hangul-multiple-candidate" => 0xff3d,
        "hangul-previous-candidate" => 0xff3e,
        "hangul-special" => 0xff3f,
        "dead-grave" => 0xfe50,
        "dead-acute" => 0xfe51,
        "dead-circumflex" => 0xfe52,
        "dead-tilde" => 0xfe53,
        "dead-macron" => 0xfe54,
        "dead-breve" => 0xfe55,
        "dead-abovedot" => 0xfe56,
        "dead-diaeresis" => 0xfe57,
        "dead-abovering" => 0xfe58,
        "dead-doubleacute" => 0xfe59,
        "dead-caron" => 0xfe5a,
        "dead-cedilla" => 0xfe5b,
        "dead-ogonek" => 0xfe5c,
        "dead-iota" => 0xfe5d,
        "dead-voiced-sound" => 0xfe5e,
        "dead-semivoiced-sound" => 0xfe5f,
        "dead-belowdot" => 0xfe60,
        "dead-hook" => 0xfe61,
        "dead-horn" => 0xfe62,
        "kana-fullstop" => 0x04a1,
        "kana-opening-bracket" => 0x04a2,
        "kana-closing-bracket" => 0x04a3,
        "kana-comma" => 0x04a4,
        "kana-conjunctive" => 0x04a5,
        "kana-WO" => 0x04a6,
        "kana-a" => 0x04a7,
        "kana-i" => 0x04a8,
        "kana-u" => 0x04a9,
        "kana-e" => 0x04aa,
        "kana-o" => 0x04ab,
        "kana-ya" => 0x04ac,
        "kana-yu" => 0x04ad,
        "kana-yo" => 0x04ae,
        "kana-tsu" => 0x04af,
        "kana-prolonged-sound" => 0x04b0,
        "kana-A" => 0x04b1,
        "kana-I" => 0x04b2,
        "kana-U" => 0x04b3,
        "kana-E" => 0x04b4,
        "kana-O" => 0x04b5,
        "kana-KA" => 0x04b6,
        "kana-KI" => 0x04b7,
        "kana-KU" => 0x04b8,
        "kana-KE" => 0x04b9,
        "kana-KO" => 0x04ba,
        "kana-SA" => 0x04bb,
        "kana-SHI" => 0x04bc,
        "kana-SU" => 0x04bd,
        "kana-SE" => 0x04be,
        "kana-SO" => 0x04bf,
        "kana-TA" => 0x04c0,
        "kana-CHI" => 0x04c1,
        "kana-TSU" => 0x04c2,
        "kana-TE" => 0x04c3,
        "kana-TO" => 0x04c4,
        "kana-NA" => 0x04c5,
        "kana-NI" => 0x04c6,
        "kana-NU" => 0x04c7,
        "kana-NE" => 0x04c8,
        "kana-NO" => 0x04c9,
        "kana-HA" => 0x04ca,
        "kana-HI" => 0x04cb,
        "kana-FU" => 0x04cc,
        "kana-HE" => 0x04cd,
        "kana-HO" => 0x04ce,
        "kana-MA" => 0x04cf,
        "kana-MI" => 0x04d0,
        "kana-MU" => 0x04d1,
        "kana-ME" => 0x04d2,
        "kana-MO" => 0x04d3,
        "kana-YA" => 0x04d4,
        "kana-YU" => 0x04d5,
        "kana-YO" => 0x04d6,
        "kana-RA" => 0x04d7,
        "kana-RI" => 0x04d8,
        "kana-RU" => 0x04d9,
        "kana-RE" => 0x04da,
        "kana-RO" => 0x04db,
        "kana-WA" => 0x04dc,
        "kana-N" => 0x04dd,
        "kana-voiced-sound" => 0x04de,
        "kana-semivoiced-sound" => 0x04df,
        "Shift_key" => 0xffe1,
        "Alt_key" => 0xffe9,
        "Control_key" => 0xffe3,
        "Meta_key" => 0xffe7,
        "Super_key" => 0xffeb,
        "Hyper_key" => 0xffed,
        "caps-lock" => 0xffe5,
        "num-lock" => 0xff7f,
        "scroll-lock" => 0xff14,
        _ => return None,
    })
}
