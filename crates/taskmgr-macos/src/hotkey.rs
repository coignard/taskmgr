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

use std::ffi::c_void;
use std::ptr;

use taskmgr::Keystroke;

use crate::keyboard;

const KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
const PRESSED: u32 = 5;
const SIGNATURE: u32 = u32::from_be_bytes(*b"tmgr");
const COMMAND: u32 = 1 << 8;
const SHIFT: u32 = 1 << 9;
const OPTION: u32 = 1 << 11;
const CONTROL: u32 = 1 << 12;

#[repr(C)]
struct HotKeyId {
    signature: u32,
    id: u32,
}

#[repr(C)]
struct EventType {
    class: u32,
    kind: u32,
}

type Handler = extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> *mut c_void;
    fn InstallEventHandler(
        target: *mut c_void,
        handler: Handler,
        count: u32,
        types: *const EventType,
        data: *mut c_void,
        installed: *mut *mut c_void,
    ) -> i32;
    fn RegisterEventHotKey(
        code: u32,
        modifiers: u32,
        id: HotKeyId,
        target: *mut c_void,
        options: u32,
        registered: *mut *mut c_void,
    ) -> i32;
}

pub(crate) fn register(keystroke: Keystroke, handler: Handler, data: *mut c_void) -> bool {
    let Some(code) = keyboard::code(keystroke.key) else {
        return false;
    };
    let held = keystroke.held;
    let modifiers = [
        (held.command, COMMAND),
        (held.shift, SHIFT),
        (held.alt, OPTION),
        (held.control, CONTROL),
    ]
    .iter()
    .filter(|&&(on, _)| on)
    .map(|&(_, flag)| flag)
    .sum();
    let pressed = EventType {
        class: KEYBOARD,
        kind: PRESSED,
    };
    let id = HotKeyId {
        signature: SIGNATURE,
        id: 1,
    };
    unsafe {
        let target = GetApplicationEventTarget();
        let mut installed = ptr::null_mut();
        let mut registered = ptr::null_mut();
        InstallEventHandler(
            target,
            handler,
            1,
            &raw const pressed,
            data,
            &raw mut installed,
        ) == 0
            && RegisterEventHotKey(code, modifiers, id, target, 0, &raw mut registered) == 0
    }
}
