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

use objc2_app_kit::{NSEvent, NSEventModifierFlags};
use taskmgr::{Key, Keystroke, Modifiers};

const OPTION_KEYS: [u16; 2] = [58, 61];
const ANSI: &str = "asdfhgzxcv\0bqweryt123465=97-80]ou[ip\0lj'k;\\,/nm.";
const FUNCTION_KEYS: std::ops::RangeInclusive<char> = '\u{F700}'..='\u{F8FF}';

pub(crate) fn key(event: &NSEvent) -> Option<Key> {
    match event.keyCode() {
        53 => Some(Key::Escape),
        36 | 76 => Some(Key::Enter),
        49 => Some(Key::Space),
        48 => Some(Key::Tab),
        51 => Some(Key::Backspace),
        117 => Some(Key::Delete),
        123 => Some(Key::Left),
        124 => Some(Key::Right),
        125 => Some(Key::Down),
        126 => Some(Key::Up),
        115 => Some(Key::Home),
        119 => Some(Key::End),
        116 => Some(Key::PageUp),
        121 => Some(Key::PageDown),
        120 => Some(Key::F2),
        118 => Some(Key::F4),
        109 => Some(Key::F10),
        code => {
            let typed = event
                .charactersIgnoringModifiers()?
                .to_string()
                .chars()
                .next()?;
            if typed.is_control() || FUNCTION_KEYS.contains(&typed) {
                return None;
            }
            let latin = ANSI
                .chars()
                .nth(usize::from(code))
                .filter(|&character| character != '\0');
            Some(Key::Character(
                latin.filter(|_| !typed.is_ascii()).unwrap_or(typed),
            ))
        }
    }
}

pub(crate) fn code(key: Key) -> Option<u32> {
    let named = match key {
        Key::Enter => 0x24,
        Key::Tab => 0x30,
        Key::Space => 0x31,
        Key::Backspace => 0x33,
        Key::Escape => 0x35,
        Key::F10 => 0x6D,
        Key::Home => 0x73,
        Key::PageUp => 0x74,
        Key::Delete => 0x75,
        Key::F2 => 0x78,
        Key::F4 => 0x76,
        Key::End => 0x77,
        Key::PageDown => 0x79,
        Key::Left => 0x7B,
        Key::Right => 0x7C,
        Key::Down => 0x7D,
        Key::Up => 0x7E,
        Key::Character('`') => 0x32,
        Key::Character(character) => {
            let index = ANSI.chars().position(|letter| letter == character)?;
            return u32::try_from(index).ok();
        }
        Key::Alt => return None,
    };
    Some(named)
}

pub(crate) fn keystroke(event: &NSEvent) -> Option<Keystroke> {
    Some(Keystroke::new(key(event)?, held(event)))
}

pub(crate) fn held(event: &NSEvent) -> Modifiers {
    let flags = event.modifierFlags();
    Modifiers {
        command: flags.contains(NSEventModifierFlags::Command),
        control: flags.contains(NSEventModifierFlags::Control),
        alt: flags.contains(NSEventModifierFlags::Option),
        shift: flags.contains(NSEventModifierFlags::Shift),
    }
}

pub(crate) fn text(event: &NSEvent) -> Option<String> {
    let text: String = event
        .characters()?
        .to_string()
        .chars()
        .filter(|character| !character.is_control() && !FUNCTION_KEYS.contains(character))
        .collect();
    (!text.is_empty()).then_some(text)
}

pub(crate) fn option(event: &NSEvent) -> Option<bool> {
    OPTION_KEYS
        .contains(&event.keyCode())
        .then(|| event.modifierFlags().contains(NSEventModifierFlags::Option))
}
