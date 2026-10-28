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

mod delegate;
mod hotkey;
mod keyboard;
mod pasteboard;
mod sound;
mod surface;
mod view;
mod window;

use std::path::PathBuf;
use std::process;

use jiff::{Timestamp, Zoned};
use objc2::rc::Retained;
use objc2::runtime::{ProtocolObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
use objc2_foundation::{
    NSFileManager, NSFullUserName, NSProcessInfo, NSSearchPathDirectory, NSSearchPathDomainMask,
    NSString,
};
use taskmgr::{App, Host, Storage};

use crate::delegate::Delegate;

fn main() {
    let mtm = MainThreadMarker::new().expect("Task Manager should start on the main thread");
    let application = NSApplication::sharedApplication(mtm);
    application.setMainMenu(Some(&menu(mtm)));
    let host = Host {
        user: licensee(),
        pid: process::id(),
        seed: Timestamp::now().as_second().unsigned_abs(),
    };
    let app = App::new(Storage::new(data_directory()), host, Zoned::now());
    delegate::dock(mtm, app.docked());
    let delegate = Delegate::new(mtm, app);
    application.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    application.run();
}

fn licensee() -> String {
    let name = NSFullUserName().to_string();
    if name.trim().is_empty() {
        NSProcessInfo::processInfo().hostName().to_string()
    } else {
        name
    }
}

fn data_directory() -> PathBuf {
    NSFileManager::defaultManager()
        .URLsForDirectory_inDomains(
            NSSearchPathDirectory::ApplicationSupportDirectory,
            NSSearchPathDomainMask::UserDomainMask,
        )
        .firstObject()
        .and_then(|url| url.to_file_path())
        .expect("macOS should provide an Application Support directory")
        .join("Task Manager")
}

fn menu(mtm: MainThreadMarker) -> Retained<NSMenu> {
    let bar = NSMenu::new(mtm);
    let application = NSMenu::new(mtm);
    for (title, action, key) in [
        ("Hide Task Manager", sel!(hide:), "h"),
        ("Quit Task Manager", sel!(terminate:), "q"),
    ] {
        application.addItem(&item(mtm, title, action, key));
    }
    let root = NSMenuItem::new(mtm);
    root.setSubmenu(Some(&application));
    bar.addItem(&root);
    bar
}

fn item(mtm: MainThreadMarker, title: &str, action: Sel, key: &str) -> Retained<NSMenuItem> {
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(action),
            &NSString::from_str(key),
        )
    }
}
