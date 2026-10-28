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

mod canvas;
mod font;
mod image;

pub use canvas::Canvas;
pub(crate) use font::Font;
pub(crate) use image::Image;

pub(crate) type Color = [u8; 4];

pub(crate) const TRANSPARENT: Color = [0, 0, 0, 0];
pub(crate) const BLACK: Color = [0, 0, 0, 255];
pub(crate) const WHITE: Color = [255, 255, 255, 255];
pub(crate) const FACE: Color = [0xD4, 0xD0, 0xC8, 255];
pub(crate) const SHADOW: Color = [0x80, 0x80, 0x80, 255];
pub(crate) const DARK: Color = [0x40, 0x40, 0x40, 255];
pub(crate) const HIGHLIGHT: Color = [0x0A, 0x24, 0x6A, 255];
pub(crate) const GRADIENT: Color = [0xA6, 0xCA, 0xF1, 255];
pub(crate) const INACTIVE_GRADIENT: Color = [0xC0, 0xC0, 0xC0, 255];
pub(crate) const GREEN: Color = [3, 0xFF, 0, 255];
pub(crate) const GRID: Color = [0, 0x80, 0x40, 255];
pub(crate) const YELLOW: Color = [0xFF, 0xFF, 0, 255];
pub(crate) const RED: Color = [0xFF, 0, 0, 255];
