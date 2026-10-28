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

use std::cell::{Cell, OnceCell, RefCell};
use std::ffi::c_void;

use jiff::{Timestamp, Zoned};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSEvent,
    NSEventModifierFlags, NSEventType, NSPanel, NSScreen, NSWindow, NSWindowDelegate,
    NSWindowOrderingMode,
};
use objc2_foundation::{
    MainThreadMarker, NSDate, NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect,
    NSRunLoop, NSRunLoopCommonModes, NSSize, NSString, NSTimer,
};
use taskmgr::geometry::Point;
use taskmgr::{App, Effect, Event, Key, Keystroke};

use crate::hotkey;
use crate::keyboard;
use crate::pasteboard;
use crate::sound::{self, Sound};
use crate::view::CanvasView;
use crate::window;

const BORDER: f64 = 4.0;
const ROW: f64 = 14.0;
const NOTCH: f64 = 3.0;

struct Stage {
    window: Retained<NSWindow>,
    view: Retained<CanvasView>,
    popups: RefCell<Vec<(Retained<NSPanel>, Retained<CanvasView>)>>,
    timer: Retained<NSTimer>,
}

pub(crate) struct Ivars {
    app: RefCell<App>,
    stage: OnceCell<Stage>,
    restored: Cell<Option<NSRect>>,
    wheel: Cell<(f64, f64)>,
    chord: Sound,
    ding: Sound,
    title: RefCell<String>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    pub(crate) struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl NSApplicationDelegate for Delegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            self.launch();
        }

        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn should_terminate_after_last_window_closed(&self, _application: &NSApplication) -> bool {
            true
        }

        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn should_handle_reopen(&self, _application: &NSApplication, visible: bool) -> bool {
            if !visible {
                self.show();
            }
            true
        }
    }

    unsafe impl NSWindowDelegate for Delegate {
        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, _notification: &NSNotification) {
            self.resized();
        }

        #[unsafe(method(windowDidMove:))]
        fn window_did_move(&self, _notification: &NSNotification) {
            self.placed();
        }

        #[unsafe(method(windowDidBecomeKey:))]
        fn window_did_become_key(&self, _notification: &NSNotification) {
            self.send(Event::Activate(true));
        }

        #[unsafe(method(windowDidResignKey:))]
        fn window_did_resign_key(&self, _notification: &NSNotification) {
            self.send(Event::Activate(false));
        }
    }

    impl Delegate {
        #[unsafe(method(tick:))]
        fn tick(&self, _timer: &NSTimer) {
            self.send(Event::Tick);
        }
    }
);

impl Delegate {
    pub(crate) fn new(mtm: MainThreadMarker, app: App) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars {
            app: RefCell::new(app),
            stage: OnceCell::new(),
            restored: Cell::new(None),
            wheel: Cell::new((0.0, 0.0)),
            chord: Sound::new(sound::CHORD),
            ding: Sound::new(sound::DING),
            title: RefCell::new(String::new()),
        });
        unsafe { msg_send![super(this), init] }
    }

    pub(crate) fn input(&self, event: &NSEvent) {
        let Some(stage) = self.ivars().stage.get() else {
            return;
        };
        let at = window::locate(stage.window.frame(), screen(event, self.mtm()));
        match event.r#type() {
            NSEventType::LeftMouseDown => self.send(Event::Press {
                at,
                clicks: u8::try_from(event.clickCount()).unwrap_or(u8::MAX),
                held: keyboard::held(event),
            }),
            NSEventType::LeftMouseDragged | NSEventType::MouseMoved => self.send(Event::Move(at)),
            NSEventType::LeftMouseUp => self.send(Event::Release(at)),
            NSEventType::ScrollWheel => {
                if let Some(by) = self.wheel(event) {
                    self.send(Event::Scroll { at, by });
                }
            }
            NSEventType::KeyDown => self.typed(event),
            NSEventType::KeyUp => {
                if let Some(key) = keyboard::key(event) {
                    self.send(Event::KeyUp(key));
                }
            }
            NSEventType::FlagsChanged => {
                let held = keyboard::held(event);
                match keyboard::option(event) {
                    Some(false) => self.send(Event::KeyUp(Key::Alt)),
                    Some(true) => self.send(Event::KeyDown(Keystroke::new(Key::Alt, held))),
                    None if held.alt => self.send(Event::KeyDown(Keystroke::new(Key::Alt, held))),
                    None => {}
                }
            }
            _ => {}
        }
    }

    fn typed(&self, event: &NSEvent) {
        let Some(keystroke) = keyboard::keystroke(event) else {
            return;
        };
        if keystroke.held.command && keystroke.key == Key::Character('v') {
            if let Some(text) = pasteboard::text() {
                self.send(Event::Paste(text));
            }
            return;
        }
        self.send(Event::KeyDown(keystroke));
        let flags = event.modifierFlags();
        if !flags.intersects(NSEventModifierFlags::Command | NSEventModifierFlags::Control)
            && let Some(text) = keyboard::text(event)
        {
            self.send(Event::Text(text));
        }
    }

    fn wheel(&self, event: &NSEvent) -> Option<Point> {
        let scale = if event.hasPreciseScrollingDeltas() {
            1.0
        } else {
            NOTCH * ROW
        };
        let (x, y) = self.ivars().wheel.get();
        let (x, y) = (
            x - event.scrollingDeltaX() * scale,
            y - event.scrollingDeltaY() * scale,
        );
        let (across, down) = (x.trunc(), y.trunc());
        self.ivars().wheel.set((x - across, y - down));
        #[expect(clippy::cast_possible_truncation)]
        let by = Point::new(across as i32, down as i32);
        (by != Point::default()).then_some(by)
    }

    fn launch(&self) {
        let mtm = self.mtm();
        let (placement, floats, hotkey) = {
            let app = self.ivars().app.borrow();
            (app.placement(), app.floats(), app.hotkey())
        };
        if let Some(keystroke) = hotkey {
            let data = std::ptr::from_ref(self).cast_mut().cast::<c_void>();
            hotkey::register(keystroke, pressed, data);
        }
        let window = window::frame(mtm, App::SIZE);
        let view = CanvasView::new(self);
        window.setContentView(Some(&view));
        window.makeFirstResponder(Some(&view));
        window.setLevel(window::level(floats));
        let saved = placement
            .map(|placement| window::screen_rect(placement.frame))
            .filter(|&frame| visible(mtm, frame));
        match saved {
            Some(frame) => window.setFrame_display(frame, false),
            None => window.center(),
        }
        window.setDelegate(Some(ProtocolObject::from_ref(self)));
        let timer = unsafe {
            NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
                NSDate::distantFuture().timeIntervalSinceNow(),
                self,
                sel!(tick:),
                None,
                true,
            )
        };
        unsafe { NSRunLoop::currentRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
        let shown = window.clone();
        let _ = self.ivars().stage.set(Stage {
            window,
            view,
            popups: RefCell::new(Vec::new()),
            timer,
        });
        if let Some(stage) = self.ivars().stage.get()
            && placement.is_some_and(|placement| placement.zoomed)
        {
            self.maximize(stage);
        }
        self.resized();
        shown.makeKeyAndOrderFront(None);
        activate(&NSApplication::sharedApplication(mtm));
    }

    fn send(&self, event: Event) {
        let effect = self.ivars().app.borrow_mut().handle(event, Zoned::now());
        if let (Some(effect), Some(stage)) = (effect, self.ivars().stage.get()) {
            self.perform(stage, effect);
        }
        self.render();
    }

    fn perform(&self, stage: &Stage, effect: Effect) {
        let window = &stage.window;
        match effect {
            Effect::Place(rect) => {
                window.setFrame_display(window::place(window.frame(), rect), false);
                if let Some(area) = window.screen().map(|screen| screen.visibleFrame()) {
                    let mut frame = window.frame();
                    let top = area.origin.y + area.size.height - frame.size.height;
                    frame.origin.y = frame.origin.y.min(top);
                    window.setFrame_display(frame, true);
                }
            }
            Effect::Minimize => window.miniaturize(None),
            Effect::Hide => NSApplication::sharedApplication(self.mtm()).hide(None),
            Effect::Dock(docked) => {
                dock(self.mtm(), docked);
                self.show();
            }
            Effect::Zoom(true) => self.maximize(stage),
            Effect::Zoom(false) => {
                if let Some(frame) = self.ivars().restored.take() {
                    window.setFrame_display(frame, true);
                }
            }
            Effect::Close => NSApplication::sharedApplication(self.mtm()).terminate(None),
            Effect::Float(floats) => window.setLevel(window::level(floats)),
            Effect::Copy(text) => pasteboard::copy(&text),
            Effect::Beep => self.ivars().ding.play(),
            Effect::Alert => self.ivars().chord.play(),
        }
    }

    fn toggle(&self) {
        let Some(stage) = self.ivars().stage.get() else {
            return;
        };
        let application = NSApplication::sharedApplication(self.mtm());
        let window = &stage.window;
        if application.isActive() && window.isVisible() && !window.isMiniaturized() {
            application.hide(None);
        } else {
            self.show();
        }
    }

    fn show(&self) {
        let Some(stage) = self.ivars().stage.get() else {
            return;
        };
        let application = NSApplication::sharedApplication(self.mtm());
        application.unhide(None);
        if stage.window.isMiniaturized() {
            stage.window.deminiaturize(None);
        }
        stage.window.makeKeyAndOrderFront(None);
        activate(&application);
    }

    fn maximize(&self, stage: &Stage) {
        let window = &stage.window;
        self.ivars().restored.set(Some(window.frame()));
        if let Some(screen) = window.screen().or_else(|| NSScreen::mainScreen(self.mtm())) {
            let area = screen.visibleFrame();
            let frame = NSRect::new(
                NSPoint::new(area.origin.x - BORDER, area.origin.y - BORDER),
                NSSize::new(
                    area.size.width + 2.0 * BORDER,
                    area.size.height + 2.0 * BORDER,
                ),
            );
            window.setFrame_display(frame, true);
        }
    }

    fn resized(&self) {
        if let Some(stage) = self.ivars().stage.get() {
            let size = window::size(stage.window.frame());
            self.send(Event::Resize(size));
            self.placed();
        }
    }

    fn placed(&self) {
        if let Some(stage) = self.ivars().stage.get() {
            self.send(Event::Placed(window::world(stage.window.frame())));
        }
    }

    fn render(&self) {
        let Some(stage) = self.ivars().stage.get() else {
            return;
        };
        let (scene, wake, title) = {
            let mut app = self.ivars().app.borrow_mut();
            (app.scene(), app.wake(), app.title())
        };
        if *self.ivars().title.borrow() != title {
            stage.window.setTitle(&NSString::from_str(&title));
            self.ivars().title.replace(title);
        }
        stage.view.present(&scene.window);
        let frame = stage.window.frame();
        let mut popups = stage.popups.borrow_mut();
        while popups.len() < scene.popups.len() {
            let panel = window::popup(self.mtm());
            let view = CanvasView::new(self);
            panel.setContentView(Some(&view));
            popups.push((panel, view));
        }
        for (index, (panel, view)) in popups.iter().enumerate() {
            if let Some((rect, canvas)) = scene.popups.get(index) {
                panel.setFrame_display(window::place(frame, *rect), false);
                view.present(canvas);
                if panel.parentWindow().is_none() {
                    unsafe {
                        stage
                            .window
                            .addChildWindow_ordered(panel, NSWindowOrderingMode::Above);
                    };
                }
            } else if panel.parentWindow().is_some() {
                stage.window.removeChildWindow(panel);
                panel.orderOut(None);
            }
        }
        window::cursor(scene.cursor).set();
        let fire = wake.map_or_else(NSDate::distantFuture, |wake| {
            let wait = wake.duration_since(Timestamp::now()).as_secs_f64().max(0.0);
            NSDate::dateWithTimeIntervalSinceNow(wait)
        });
        stage.timer.setFireDate(&fire);
    }
}

fn activate(application: &NSApplication) {
    if application.respondsToSelector(sel!(activate)) {
        application.activate();
    } else {
        #[expect(deprecated)]
        application.activateIgnoringOtherApps(true);
    }
}

pub(crate) fn dock(mtm: MainThreadMarker, docked: bool) {
    let policy = if docked {
        NSApplicationActivationPolicy::Regular
    } else {
        NSApplicationActivationPolicy::Accessory
    };
    NSApplication::sharedApplication(mtm).setActivationPolicy(policy);
}

extern "C" fn pressed(_call: *mut c_void, _event: *mut c_void, data: *mut c_void) -> i32 {
    let delegate = unsafe { &*data.cast::<Delegate>() };
    delegate.toggle();
    0
}

fn screen(event: &NSEvent, mtm: MainThreadMarker) -> NSPoint {
    event
        .window(mtm)
        .map_or_else(NSEvent::mouseLocation, |window| {
            window.convertPointToScreen(event.locationInWindow())
        })
}

fn visible(mtm: MainThreadMarker, frame: NSRect) -> bool {
    NSScreen::screens(mtm).iter().any(|screen| {
        let area = screen.visibleFrame();
        frame.origin.x < area.origin.x + area.size.width
            && area.origin.x < frame.origin.x + frame.size.width
            && frame.origin.y < area.origin.y + area.size.height
            && area.origin.y < frame.origin.y + frame.size.height
    })
}
