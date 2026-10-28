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

use std::sync::LazyLock;

use crate::graphics::{Canvas, Font, Image, TRANSPARENT};

const FONT_SIZE: u16 = 11;

pub(crate) static FONT: LazyLock<Font> =
    LazyLock::new(|| font(include_bytes!("../assets/fonts/tahoma.otb")));

pub(crate) static BOLD: LazyLock<Font> =
    LazyLock::new(|| font(include_bytes!("../assets/fonts/tahomabd.otb")));

pub(crate) static SANS: LazyLock<Font> =
    LazyLock::new(|| font(include_bytes!("../assets/fonts/micross.otb")).cell(10, 13));

pub(crate) static ICON: LazyLock<Image> =
    LazyLock::new(|| image(include_bytes!("../assets/icons/taskmgr.png")));

pub(crate) static BIG: LazyLock<Image> =
    LazyLock::new(|| image(include_bytes!("../assets/icons/taskmgr32.png")));

pub(crate) static WARNING: LazyLock<Image> =
    LazyLock::new(|| image(include_bytes!("../assets/icons/warning.png")));
pub(crate) static ERROR: LazyLock<Image> =
    LazyLock::new(|| image(include_bytes!("../assets/icons/error.png")));
pub(crate) static BALLS: LazyLock<[Image; 7]> = LazyLock::new(|| {
    [
        include_bytes!("../assets/icons/balls/red.png").as_slice(),
        include_bytes!("../assets/icons/balls/orange.png"),
        include_bytes!("../assets/icons/balls/yellow.png"),
        include_bytes!("../assets/icons/balls/green.png"),
        include_bytes!("../assets/icons/balls/cyan.png"),
        include_bytes!("../assets/icons/balls/blue.png"),
        include_bytes!("../assets/icons/balls/violet.png"),
    ]
    .map(image)
});

fn image(data: &[u8]) -> Image {
    Image::decode(data).expect("embedded icons should be 8-bit RGBA PNGs")
}

fn font(data: &'static [u8]) -> Font {
    Font::parse(data, FONT_SIZE).expect("embedded fonts should have an 11 px bitmap strike")
}

pub fn icons() -> [Canvas; 2] {
    [&*ICON, &*BIG].map(|image| {
        let mut canvas = Canvas::new(image.size(), TRANSPARENT);
        canvas.draw(image, 0, 0);
        canvas
    })
}
