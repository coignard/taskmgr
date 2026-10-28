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

use std::ops::Range;

use crate::assets::FONT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Motion {
    Back,
    Forth,
    WordBack,
    WordForth,
    Start,
    End,
}

#[derive(Debug, Default)]
pub(crate) struct Field {
    text: String,
    anchor: usize,
    caret: usize,
    pub(crate) shift: i32,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    typing: bool,
}

#[derive(Debug)]
struct Snapshot {
    text: String,
    anchor: usize,
    caret: usize,
}

impl Field {
    pub(crate) fn new(text: String) -> Self {
        let caret = text.len();
        Self {
            text,
            caret,
            ..Self::default()
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn caret(&self) -> usize {
        self.caret
    }

    pub(crate) fn selection(&self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }

    pub(crate) fn selected(&self) -> Option<&str> {
        let range = self.selection();
        (!range.is_empty()).then(|| &self.text[range])
    }

    pub(crate) fn type_in(&mut self, text: &str) {
        self.record(true);
        self.insert(text);
    }

    pub(crate) fn paste(&mut self, text: &str) {
        self.record(false);
        self.insert(text);
    }

    pub(crate) fn cut(&mut self) -> Option<String> {
        let cut = self.selected()?.to_owned();
        self.record(false);
        self.insert("");
        Some(cut)
    }

    pub(crate) fn replace(&mut self, text: String) {
        self.record(false);
        self.text = text;
        self.select_all();
    }

    pub(crate) fn erase(&mut self, motion: Motion) {
        let coalesce = self.selection().is_empty();
        if coalesce {
            self.anchor = self.target(motion);
        }
        self.record(coalesce);
        self.insert("");
    }

    pub(crate) fn undo(&mut self) -> bool {
        self.travel(true)
    }

    pub(crate) fn redo(&mut self) -> bool {
        self.travel(false)
    }

    pub(crate) fn go(&mut self, motion: Motion, extend: bool) {
        let selection = self.selection();
        let caret = match motion {
            Motion::Back if !extend && !selection.is_empty() => selection.start,
            Motion::Forth if !extend && !selection.is_empty() => selection.end,
            _ => self.target(motion),
        };
        self.place(caret, extend);
    }

    pub(crate) fn select_all(&mut self) {
        self.typing = false;
        self.anchor = 0;
        self.caret = self.text.len();
    }

    pub(crate) fn select_word(&mut self, index: usize) {
        let word = |character: char| character.is_alphanumeric();
        let (before, after) = self.text.split_at(index.min(self.text.len()));
        let inside = after.chars().next().is_some_and(word);
        let start = if inside {
            before.trim_end_matches(word).len()
        } else {
            index
        };
        let end = self.text.len() - after.trim_start_matches(word).len();
        self.place(start, false);
        self.place(end.max(start), true);
    }

    pub(crate) fn place(&mut self, index: usize, extend: bool) {
        self.typing = false;
        self.caret = index;
        if !extend {
            self.anchor = index;
        }
    }

    pub(crate) fn index_at(&self, x: i32) -> usize {
        let mut left = 0;
        for (index, character) in self.text.char_indices() {
            let advance = FONT.advance(character);
            if x < left + advance / 2 {
                return index;
            }
            left += advance;
        }
        self.text.len()
    }

    fn insert(&mut self, text: &str) {
        let line: String = text
            .chars()
            .map(|character| {
                if character.is_whitespace() {
                    ' '
                } else {
                    character
                }
            })
            .filter(|character| !character.is_control())
            .collect();
        let range = self.selection();
        self.text.replace_range(range.clone(), &line);
        self.caret = range.start + line.len();
        self.anchor = self.caret;
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            anchor: self.anchor,
            caret: self.caret,
        }
    }

    fn record(&mut self, coalesce: bool) {
        if !(coalesce && self.typing) {
            self.undo.push(self.snapshot());
        }
        self.redo.clear();
        self.typing = coalesce;
    }

    fn travel(&mut self, back: bool) -> bool {
        let (from, to) = if back {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(snapshot) = from.pop() else {
            return false;
        };
        to.push(Snapshot {
            text: std::mem::replace(&mut self.text, snapshot.text),
            anchor: self.anchor,
            caret: self.caret,
        });
        self.anchor = snapshot.anchor;
        self.caret = snapshot.caret;
        self.typing = false;
        true
    }

    fn target(&self, motion: Motion) -> usize {
        let (before, after) = self.text.split_at(self.caret);
        let word = |character: char| character.is_alphanumeric();
        match motion {
            Motion::Back => before
                .char_indices()
                .next_back()
                .map_or(0, |(index, _)| index),
            Motion::Forth => after
                .chars()
                .next()
                .map_or(self.caret, |next| self.caret + next.len_utf8()),
            Motion::WordBack => {
                let trimmed = before.trim_end_matches(|character: char| !word(character));
                trimmed.trim_end_matches(word).len()
            }
            Motion::WordForth => {
                let rest = after.trim_start_matches(|character: char| !word(character));
                let rest = rest.trim_start_matches(word);
                self.text.len() - rest.len()
            }
            Motion::Start => 0,
            Motion::End => self.text.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(text: &str) -> Field {
        let mut field = Field::new(text.to_owned());
        field.go(Motion::End, false);
        field
    }

    #[test]
    fn opens_with_everything_selected_so_typing_replaces_it() {
        let mut field = Field::new("taskmgr".to_owned());
        assert_eq!(field.selected(), Some("taskmgr"));
        field.type_in("Ship\tit");
        assert_eq!((field.text(), field.caret()), ("Ship it", 7));
    }

    #[test]
    fn edits_cyrillic_by_characters() {
        let mut field = field("ёжик");
        field.go(Motion::Back, false);
        field.erase(Motion::Back);
        field.type_in("щ");
        assert_eq!(field.text(), "ёжщк");
        field.erase(Motion::Forth);
        assert_eq!(field.text(), "ёжщ");
    }

    #[test]
    fn jumps_and_selects_by_words() {
        let mut field = field("Call the bank");
        field.go(Motion::WordBack, true);
        assert_eq!(field.selected(), Some("bank"));
        field.go(Motion::WordBack, false);
        assert_eq!(&field.text()[field.caret()..], "the bank");
        field.go(Motion::WordForth, false);
        assert_eq!(&field.text()[field.caret()..], " bank");
        field.erase(Motion::WordBack);
        assert_eq!(field.text(), "Call  bank");
    }

    #[test]
    fn typing_undoes_as_one_step_and_paste_as_another() {
        let mut field = field("");
        for letter in ["C", "a", "l", "l"] {
            field.type_in(letter);
        }
        field.paste(" Anna");
        field.erase(Motion::Back);
        assert_eq!(field.text(), "Call Ann");
        assert!(field.undo());
        assert_eq!(field.text(), "Call Anna");
        assert!(field.undo());
        assert_eq!(field.text(), "Call");
        assert!(field.undo());
        assert_eq!(field.text(), "");
        assert!(!field.undo());
        assert!(field.redo() && field.redo());
        assert_eq!(field.text(), "Call Anna");
        field.type_in("!");
        assert!(!field.redo());
    }

    #[test]
    fn double_clicks_pick_the_word_under_the_pointer() {
        let mut field = field("Call the bank");
        field.select_word(6);
        assert_eq!(field.selected(), Some("the"));
        field.select_word(13);
        assert_eq!(field.selected(), None);
        assert_eq!(field.cut(), None);
        field.select_word(0);
        assert_eq!(field.cut().as_deref(), Some("Call"));
        assert_eq!(field.text(), " the bank");
    }

    #[test]
    fn clicks_land_between_characters() {
        let field = field("ab");
        let a = FONT.advance('a');
        assert_eq!(field.index_at(0), 0);
        assert_eq!(field.index_at(a - 1), 1);
        assert_eq!(field.index_at(500), 2);
    }
}
