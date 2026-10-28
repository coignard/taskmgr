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

use crate::assets::{BIG, FONT, SANS};
use crate::chrome::{self, CAPTION, Caption};
use crate::controls::{self, BOX, ETCHED, Ink, RAISED, SUNKEN, State};
use crate::dialog::{
    About, COMBO, Columns, Control, Dialog, LINE, Message, Part, Prompt, SHOWN, TEXT, TOP,
};
use crate::field::Field;
use crate::geometry::Rect;
use crate::graphics::{BLACK, Canvas, FACE, HIGHLIGHT, WHITE};
use crate::metric::CHOICES;
use crate::scroll;
use crate::system::thousands;
use crate::ui::{Cues, Target, Ui};

const BORDER: i32 = 3;
const ARROW: i32 = 16;
const HELP: bool = false;
const ROWS: [i32; 12] = [65, 83, 101, 119, 137, 155, 173, 190, 208, 226, 244, 262];
const LANES: [i32; 2] = [14, 158];
const MEMORY: &str = "Physical memory available to Task Manager:";
const VERSION: &str = concat!(
    "Version ",
    env!("CARGO_PKG_VERSION"),
    " (Build ",
    env!("TASKMGR_BUILD"),
    ")"
);

pub(crate) fn draw(ui: &mut Ui, dialog: &mut Dialog, active: bool, caret: bool) {
    let size = dialog.size();
    let area = Rect::new(0, 0, size.width, size.height);
    controls::bevel(&mut ui.canvas, area, RAISED);
    ui.target(area, Target::Dialog(Part::Body));
    let bar = Caption {
        area: Rect::new(BORDER, BORDER, size.width - 2 * BORDER, CAPTION),
        title: dialog.title(),
        active,
        icon: false,
        drag: Target::Dialog(Part::Caption),
    };
    let mut knobs = Vec::new();
    if HELP && matches!(dialog, Dialog::Prompt(_)) {
        knobs.push(chrome::knob(Target::Dialog(Part::Help), true, true));
    }
    knobs.push(chrome::knob(
        Target::Dialog(Part::Close),
        false,
        dialog.closable(),
    ));
    chrome::caption(ui, &bar, &knobs);
    match dialog {
        Dialog::Prompt(prompt) => draw_prompt(ui, prompt, active, caret),
        Dialog::About(about) => draw_about(&mut ui.canvas, about),
        Dialog::Message(message) => draw_message(&mut ui.canvas, message),
        Dialog::Columns(columns) => draw_columns(ui, columns, active),
    }
    let default = dialog.default().filter(|_| active);
    let focus = dialog.focus();
    let font = if matches!(dialog, Dialog::Columns(_)) {
        &SANS
    } else {
        &FONT
    };
    for button in dialog.buttons() {
        let target = Target::Dialog(Part::Reply(button.reply));
        let state = match (button.enabled, ui.held(target)) {
            (false, _) => State::Disabled,
            (true, true) => State::Pushed,
            (true, false) => State::Normal,
        };
        let cues = Cues {
            keys: ui.cues.keys,
            focus: ui.cues.focus && active && focus == Control::Reply(button.reply),
        };
        controls::button(
            &mut ui.canvas,
            font,
            button.area,
            button.label,
            state,
            cues,
            default == Some(button.reply),
        );
        ui.target(button.area, target);
    }
}

fn draw_prompt(ui: &mut Ui, prompt: &mut Prompt, active: bool, caret: bool) {
    let cues = ui.cues;
    let canvas = &mut ui.canvas;
    canvas.draw(&BIG, 14, 40);
    for (line, text) in (0..).zip(prompt.purpose.message()) {
        FONT.draw(canvas, text, 57, 40 + LINE * line, BLACK);
    }
    controls::label(
        canvas,
        prompt.purpose.label(),
        14,
        85,
        Ink::Plain(BLACK),
        cues.keys,
    );
    controls::bevel(canvas, COMBO, SUNKEN);
    let inside = COMBO.inset(2);
    canvas.fill(inside, WHITE);
    let button = Rect::new(inside.right() - ARROW, inside.y, ARROW, inside.height);
    let lane = Rect::sides(inside.x + 1, inside.y, button.x - 1, inside.bottom());
    let focused = active && prompt.focus == Control::Field;
    edit(canvas, lane, &mut prompt.field, focused, caret);
    let pushed = prompt.dropped && ui.held(Target::Dialog(Part::Arrow));
    let state = if pushed { State::Pushed } else { State::Normal };
    scroll::arrow(&mut ui.canvas, button, true, true, state);
    ui.target(lane, Target::Dialog(Part::Field));
    ui.target(button, Target::Dialog(Part::Arrow));
}

fn edit(canvas: &mut Canvas, lane: Rect, field: &mut Field, focused: bool, caret: bool) {
    let room = lane.width - 5;
    let at = |field: &Field, index: usize| FONT.width(&field.text()[..index]);
    let caret_x = at(field, field.caret());
    if caret_x - field.shift > room {
        field.shift = caret_x - room;
    }
    if caret_x < field.shift {
        field.shift = caret_x;
    }
    let origin = lane.x + 3 - field.shift;
    let top = lane.y + 1;
    canvas.clipped(lane, |canvas| {
        FONT.draw(canvas, field.text(), origin, top, BLACK);
        let selection = field.selection();
        if focused && !selection.is_empty() {
            let left = origin + at(field, selection.start);
            let text = &field.text()[selection.clone()];
            canvas.fill(Rect::new(left, top, FONT.width(text), LINE), HIGHLIGHT);
            FONT.draw(canvas, text, left, top, WHITE);
        } else if focused && caret {
            canvas.fill(Rect::new(origin + caret_x, top, 1, LINE), BLACK);
        }
    });
}

pub(crate) fn draw_dropdown(ui: &mut Ui, prompt: &Prompt) {
    let size = ui.canvas.size();
    let area = Rect::new(0, 0, size.width, size.height);
    ui.canvas.frame(area, BLACK, BLACK);
    let inside = area.inset(1);
    ui.canvas.fill(inside, WHITE);
    let bar = Rect::new(inside.right() - ARROW, inside.y, ARROW, inside.height);
    let rows = Rect::sides(inside.x, inside.y, bar.x, inside.bottom());
    let highlight = prompt.highlight;
    for (index, text) in prompt.recent.iter().enumerate().take(SHOWN) {
        let row = Rect::new(
            rows.x,
            rows.y + LINE * i32::try_from(index).unwrap_or_default(),
            rows.width,
            LINE,
        );
        let lit = highlight == Some(index);
        if lit {
            ui.canvas.fill(row, HIGHLIGHT);
        }
        ui.canvas.clipped(row, |canvas| {
            FONT.draw(
                canvas,
                text,
                row.x + 2,
                row.y,
                if lit { WHITE } else { BLACK },
            );
        });
        ui.target(row, Target::Dialog(Part::Choice(index)));
    }
    let half = bar.height / 2;
    let fits = prompt.recent.len() <= SHOWN;
    let state = if fits { State::Disabled } else { State::Normal };
    scroll::arrow(
        &mut ui.canvas,
        Rect::new(bar.x, bar.y, ARROW, half.min(ARROW)),
        true,
        false,
        state,
    );
    let low = bar.bottom() - half.min(ARROW);
    scroll::arrow(
        &mut ui.canvas,
        Rect::new(bar.x, low, ARROW, half.min(ARROW)),
        true,
        true,
        state,
    );
    let track = Rect::sides(bar.x, bar.y + half.min(ARROW), bar.right(), low);
    for y in track.y..track.bottom() {
        for x in track.x..track.right() {
            ui.canvas
                .plot(x, y, if (x + y) % 2 == 0 { WHITE } else { FACE });
        }
    }
}

fn draw_about(canvas: &mut Canvas, about: &About) {
    canvas.draw(&BIG, 35, 33);
    let lines = [
        (33, "Task Manager"),
        (50, VERSION),
        (66, "Copyright (C) 2026 René Coignard"),
        (115, "This product is licensed to:"),
        (131, &about.licensee),
    ];
    for (top, text) in lines {
        FONT.draw(canvas, text, 108, top, BLACK);
    }
    controls::bevel(canvas, Rect::new(108, 168, 298, 2), ETCHED);
    FONT.draw(canvas, MEMORY, 108, 175, BLACK);
    let amount = format!("{} KB", thousands(about.memory));
    FONT.draw(canvas, &amount, 108 + FONT.width(MEMORY) + 11, 175, BLACK);
}

fn draw_message(canvas: &mut Canvas, message: &Message) {
    canvas.draw(message.notice.icon(), 14, TOP);
    let top = message.text();
    for (line, text) in (0..).zip(&message.lines) {
        FONT.draw(canvas, text, TEXT, top + LINE * line, BLACK);
    }
}

fn draw_columns(ui: &mut Ui, columns: &Columns, active: bool) {
    let cues = ui.cues;
    for (line, text) in (0..).zip([
        "Select the columns that will appear on the Process page",
        "of the Task Manager.",
    ]) {
        SANS.draw(&mut ui.canvas, text, 14, 30 + LINE * line, BLACK);
    }
    for (index, (&(text, _), checked)) in CHOICES.iter().zip(columns.checked).enumerate() {
        let (lane, row) = if index < 11 {
            (0, index)
        } else {
            (1, index - 11)
        };
        let (x, y) = (LANES[lane], ROWS[row]);
        let enabled = Columns::enabled(index);
        controls::checkbox(&mut ui.canvas, x, y, checked, enabled);
        let label = x + BOX + 5;
        let ink = if enabled {
            Ink::Plain(BLACK)
        } else {
            Ink::Embossed
        };
        controls::lettering(&mut ui.canvas, &SANS, text, label, y + 1, ink, false);
        let width = SANS.width(text);
        if cues.focus && active && columns.focus == Control::Check(index) {
            controls::focus(
                &mut ui.canvas,
                Rect::new(label - 1, y - 1, width + 3, 15),
                FACE,
            );
        }
        ui.target(
            Rect::new(x, y - 1, label + width + 2 - x, BOX + 2),
            Target::Dialog(Part::Check(index)),
        );
    }
}
