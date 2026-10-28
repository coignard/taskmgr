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

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{NSBeep, NSSound};
use objc2_foundation::NSData;

pub(crate) const CHORD: &[u8] = include_bytes!("../assets/chord.wav");
pub(crate) const DING: &[u8] = include_bytes!("../assets/ding.wav");

pub(crate) struct Sound(Option<Retained<NSSound>>);

impl Sound {
    pub(crate) fn new(wave: &[u8]) -> Self {
        let data = NSData::with_bytes(wave);
        Self(NSSound::initWithData(NSSound::alloc(), &data))
    }

    pub(crate) fn play(&self) {
        match &self.0 {
            Some(sound) => {
                sound.stop();
                sound.play();
            }
            None => NSBeep(),
        }
    }
}
