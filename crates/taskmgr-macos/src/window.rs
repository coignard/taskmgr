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

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, available, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSCursor, NSCursorFrameResizeDirections,
    NSCursorFrameResizePosition, NSFloatingWindowLevel, NSNormalWindowLevel, NSPanel,
    NSPopUpMenuWindowLevel, NSResponder, NSWindow, NSWindowCollectionBehavior, NSWindowLevel,
    NSWindowStyleMask,
};
use objc2_foundation::{NSObject, NSPoint, NSRect, NSSize};
use taskmgr::geometry::{Point, Rect, Size};
use taskmgr::{Cursor, Edge};

define_class!(
    #[unsafe(super(NSWindow, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    struct Frame;

    impl Frame {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            true
        }
    }
);

pub(crate) fn frame(mtm: MainThreadMarker, size: Size) -> Retained<NSWindow> {
    let this = Frame::alloc(mtm).set_ivars(());
    let content = NSRect::new(
        NSPoint::ZERO,
        NSSize::new(points(size.width), points(size.height)),
    );
    let window: Retained<Frame> = unsafe {
        msg_send![
            super(this),
            initWithContentRect: content,
            styleMask: NSWindowStyleMask::Borderless | NSWindowStyleMask::Miniaturizable,
            backing: NSBackingStoreType::Buffered,
            defer: false,
        ]
    };
    let window = Retained::into_super(window);
    window.setHasShadow(false);
    window.setAcceptsMouseMovedEvents(true);
    window.setCollectionBehavior(NSWindowCollectionBehavior::MoveToActiveSpace);
    unsafe { window.setReleasedWhenClosed(false) };
    window
}

pub(crate) fn popup(mtm: MainThreadMarker) -> Retained<NSPanel> {
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        NSRect::ZERO,
        NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
        NSBackingStoreType::Buffered,
        false,
    );
    panel.setHasShadow(false);
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setLevel(NSPopUpMenuWindowLevel);
    unsafe { panel.setReleasedWhenClosed(false) };
    panel
}

pub(crate) fn level(floats: bool) -> NSWindowLevel {
    if floats {
        NSFloatingWindowLevel
    } else {
        NSNormalWindowLevel
    }
}

pub(crate) fn locate(frame: NSRect, screen: NSPoint) -> Point {
    let top = frame.origin.y + frame.size.height;
    Point::new(pixels(screen.x - frame.origin.x), pixels(top - screen.y))
}

pub(crate) fn place(frame: NSRect, rect: Rect) -> NSRect {
    let top = frame.origin.y + frame.size.height;
    NSRect::new(
        NSPoint::new(frame.origin.x + points(rect.x), top - points(rect.bottom())),
        NSSize::new(points(rect.width), points(rect.height)),
    )
}

pub(crate) fn world(frame: NSRect) -> Rect {
    Rect::new(
        pixels(frame.origin.x),
        pixels(frame.origin.y),
        pixels(frame.size.width),
        pixels(frame.size.height),
    )
}

pub(crate) fn screen_rect(rect: Rect) -> NSRect {
    NSRect::new(
        NSPoint::new(points(rect.x), points(rect.y)),
        NSSize::new(points(rect.width), points(rect.height)),
    )
}

pub(crate) fn size(frame: NSRect) -> Size {
    Size::new(pixels(frame.size.width), pixels(frame.size.height))
}

#[expect(deprecated)]
pub(crate) fn cursor(cursor: Cursor) -> Retained<NSCursor> {
    match cursor {
        Cursor::Arrow => NSCursor::arrowCursor(),
        Cursor::No => NSCursor::operationNotAllowedCursor(),
        Cursor::Split if available!(macos = 15.0) => NSCursor::columnResizeCursor(),
        Cursor::Resize(edge) if available!(macos = 15.0) => {
            NSCursor::frameResizeCursorFromPosition_inDirections(
                position(edge),
                NSCursorFrameResizeDirections::All,
            )
        }
        Cursor::Resize(Edge { x: 0, .. }) => NSCursor::resizeUpDownCursor(),
        Cursor::Split | Cursor::Resize(_) => NSCursor::resizeLeftRightCursor(),
    }
}

fn position(edge: Edge) -> NSCursorFrameResizePosition {
    match (edge.x, edge.y) {
        (-1, -1) => NSCursorFrameResizePosition::TopLeft,
        (1, -1) => NSCursorFrameResizePosition::TopRight,
        (-1, 1) => NSCursorFrameResizePosition::BottomLeft,
        (1, 1) => NSCursorFrameResizePosition::BottomRight,
        (_, -1) => NSCursorFrameResizePosition::Top,
        (_, 1) => NSCursorFrameResizePosition::Bottom,
        (-1, _) => NSCursorFrameResizePosition::Left,
        _ => NSCursorFrameResizePosition::Right,
    }
}

fn points(pixels: i32) -> f64 {
    f64::from(pixels)
}

#[expect(clippy::cast_possible_truncation)]
fn pixels(points: f64) -> i32 {
    points.floor() as i32
}
