// This file is part of Task Manager.
//
// Copyright (c) 2026  René Coignard <contact@renecoignard.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    Alt,
    F2,
    F4,
    F10,
    Escape,
    Enter,
    Space,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Character(char),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[expect(clippy::struct_excessive_bools)]
pub struct Modifiers {
    pub command: bool,
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Keystroke {
    pub key: Key,
    pub held: Modifiers,
}

const NAMES: [(Key, &str); 18] = [
    (Key::Alt, "Alt"),
    (Key::F2, "F2"),
    (Key::F4, "F4"),
    (Key::F10, "F10"),
    (Key::Escape, "Esc"),
    (Key::Enter, "Enter"),
    (Key::Space, "Space"),
    (Key::Tab, "Tab"),
    (Key::Backspace, "Backspace"),
    (Key::Delete, "Delete"),
    (Key::Left, "Left"),
    (Key::Right, "Right"),
    (Key::Up, "Up"),
    (Key::Down, "Down"),
    (Key::Home, "Home"),
    (Key::End, "End"),
    (Key::PageUp, "PageUp"),
    (Key::PageDown, "PageDown"),
];

impl Keystroke {
    pub const fn new(key: Key, held: Modifiers) -> Self {
        Self { key, held }
    }
}

impl fmt::Display for Keystroke {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Modifiers {
            command,
            control,
            alt,
            shift,
        } = self.held;
        for (held, name) in [
            (command, "Cmd"),
            (control, "Ctrl"),
            (alt, "Alt"),
            (shift, "Shift"),
        ] {
            if held {
                write!(formatter, "{name}+")?;
            }
        }
        match self.key {
            Key::Character(character) => write!(formatter, "{}", character.to_uppercase()),
            key => {
                let name = NAMES.iter().find(|&&(named, _)| named == key);
                write!(formatter, "{}", name.map_or("", |&(_, name)| name))
            }
        }
    }
}

impl FromStr for Keystroke {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let last = parts.pop().unwrap_or_default();
        let mut held = Modifiers::default();
        for part in parts {
            let flag = match part.to_lowercase().as_str() {
                "cmd" | "command" => &mut held.command,
                "ctrl" | "control" => &mut held.control,
                "alt" | "option" => &mut held.alt,
                "shift" => &mut held.shift,
                _ => return Err(format!("unknown modifier `{part}` in `{text}`")),
            };
            *flag = true;
        }
        let mut characters = last.chars();
        let key = match (characters.next(), characters.next()) {
            (Some(character), None) => Key::Character(character.to_ascii_lowercase()),
            _ => NAMES
                .iter()
                .find(|(_, name)| name.eq_ignore_ascii_case(last))
                .map(|&(key, _)| key)
                .ok_or_else(|| format!("unknown key `{last}` in `{text}`"))?,
        };
        Ok(Self { key, held })
    }
}

impl TryFrom<String> for Keystroke {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        text.parse()
    }
}

impl From<Keystroke> for String {
    fn from(keystroke: Keystroke) -> Self {
        keystroke.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_read_and_write_like_people_type_them() {
        let keystroke: Keystroke = "cmd+Shift+n".parse().unwrap();
        assert_eq!(keystroke.key, Key::Character('n'));
        assert!(keystroke.held.command && keystroke.held.shift && !keystroke.held.alt);
        assert_eq!(keystroke.to_string(), "Cmd+Shift+N");
        assert_eq!(
            "Option+F10".parse::<Keystroke>().unwrap().to_string(),
            "Alt+F10"
        );
    }

    #[test]
    fn mistakes_name_the_culprit() {
        assert!(
            "Hyper+K"
                .parse::<Keystroke>()
                .unwrap_err()
                .contains("`Hyper`")
        );
        assert!(
            "Cmd+Nope"
                .parse::<Keystroke>()
                .unwrap_err()
                .contains("`Nope`")
        );
    }
}
