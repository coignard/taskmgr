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

#![forbid(unsafe_code)]

mod app;
mod assets;
mod chrome;
mod command;
mod controls;
mod dialog;
mod field;
pub mod geometry;
pub mod graphics;
mod keys;
mod list;
mod menu;
mod metric;
mod pages;
mod performance;
mod scroll;
mod state;
mod storage;
mod system;
mod tag;
mod ui;

pub use app::{App, Cursor, Effect, Event, Host, Scene};
pub use assets::icons;
pub use keys::{Key, Keystroke, Modifiers};
pub use state::Placement;
pub use storage::Storage;
pub use ui::Edge;
