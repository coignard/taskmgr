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

use crate::assets::BALLS;
use crate::graphics::Image;

const COLORS: [&str; 7] = ["red", "orange", "yellow", "green", "cyan", "blue", "violet"];

pub(crate) fn split(name: &str) -> (String, Option<&'static Image>) {
    let mut shown = String::with_capacity(name.len());
    let mut ball = None;
    for piece in name.split_inclusive(char::is_whitespace) {
        let Some((index, rest)) = color(piece) else {
            shown.push_str(piece);
            continue;
        };
        ball.get_or_insert(&BALLS[index]);
        if !rest.trim().is_empty() {
            shown.truncate(shown.trim_end().len());
            shown.push_str(rest);
        }
    }
    (shown.trim().to_owned(), ball)
}

fn color(piece: &str) -> Option<(usize, &str)> {
    let word = piece.strip_prefix('@')?;
    let end = word
        .find(|letter: char| !letter.is_alphanumeric())
        .unwrap_or(word.len());
    let tag = word[..end].to_lowercase();
    let tag = if tag == "purple" { "violet" } else { &tag };
    let index = COLORS.iter().position(|&color| color == tag)?;
    Some((index, &word[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(name: &str) -> (String, Option<usize>) {
        let (shown, image) = split(name);
        let index = image.and_then(|image| BALLS.iter().position(|ball| std::ptr::eq(ball, image)));
        (shown, index)
    }

    #[test]
    fn a_color_tag_anywhere_turns_into_its_ball() {
        assert_eq!(
            shown("Fix the tap @blue"),
            ("Fix the tap".to_owned(), Some(5))
        );
        assert_eq!(
            shown("@Red Renew  passport"),
            ("Renew  passport".to_owned(), Some(0))
        );
        assert_eq!(
            shown("Fix @green the @red tap"),
            ("Fix the tap".to_owned(), Some(3))
        );
        assert_eq!(
            shown("Call Anna @purple!"),
            ("Call Anna!".to_owned(), Some(6))
        );
        assert_eq!(
            shown("ask@red.com about @redesign"),
            ("ask@red.com about @redesign".to_owned(), None)
        );
        assert_eq!(shown("Buy milk"), ("Buy milk".to_owned(), None));
    }
}
