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

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl Size {
    pub const fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn sides(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self::new(left, top, right - left, bottom - top)
    }

    pub const fn size(self) -> Size {
        Size::new(self.width, self.height)
    }

    pub const fn right(self) -> i32 {
        self.x + self.width
    }

    pub const fn bottom(self) -> i32 {
        self.y + self.height
    }

    pub const fn contains(self, point: Point) -> bool {
        point.x >= self.x && point.x < self.right() && point.y >= self.y && point.y < self.bottom()
    }

    #[must_use]
    pub const fn inset(self, amount: i32) -> Self {
        self.shrink(amount, amount, amount, amount)
    }

    #[must_use]
    pub const fn shrink(self, left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self::sides(
            self.x + left,
            self.y + top,
            self.right() - right,
            self.bottom() - bottom,
        )
    }

    #[must_use]
    pub fn intersect(self, other: Self) -> Self {
        Self::sides(
            self.x.max(other.x),
            self.y.max(other.y),
            self.right().min(other.right()),
            self.bottom().min(other.bottom()),
        )
    }

    #[must_use]
    pub const fn offset(self, x: i32, y: i32) -> Self {
        Self::new(self.x + x, self.y + y, self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_its_origin_but_not_its_far_edges() {
        let rect = Rect::new(10, 20, 30, 40);
        assert!(rect.contains(Point::new(10, 20)));
        assert!(rect.contains(Point::new(39, 59)));
        assert!(!rect.contains(Point::new(40, 30)));
        assert!(!rect.contains(Point::new(20, 60)));
    }

    #[test]
    fn shrinking_moves_each_side_inwards() {
        let rect = Rect::new(0, 0, 100, 50).shrink(1, 2, 3, 4);
        assert_eq!(rect, Rect::sides(1, 2, 97, 46));
    }
}
