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

use crate::assets::FONT;
use crate::command::Command;
use crate::controls::{self, BUTTON, Ink, State};
use crate::geometry::Rect;
use crate::graphics::{BLACK, Canvas, DARK, FACE, SHADOW, WHITE};
use crate::scroll;
use crate::ui::{Cues, Target, Ui};

const STRIP: i32 = 20;
const SPIN: i32 = 16;
const MARGIN: i32 = 16;
const RISE: i32 = 18;
const BUTTON_HEIGHT: i32 = 23;
const GAP: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Focus {
    Tabs(usize),
    List,
    Button(Command),
}

pub(crate) struct Strip<'a> {
    pub(crate) depth: usize,
    pub(crate) titles: &'a [&'a str],
    pub(crate) selected: Option<usize>,
    pub(crate) first: usize,
    pub(crate) focused: bool,
    pub(crate) drop: Option<usize>,
    pub(crate) guest: Option<(usize, &'a str)>,
    pub(crate) absent: Option<usize>,
}

fn widths<'a>(titles: &'a [&str]) -> impl Iterator<Item = i32> + 'a {
    titles.iter().map(|title| controls::width(title) + 12)
}

fn room(area: Rect, titles: &[&str]) -> Option<i32> {
    let total: i32 = widths(titles).sum();
    (total > area.width - 4).then_some(area.width - 2 - 2 * SPIN)
}

pub(crate) fn scrolled(area: Rect, titles: &[&str], first: usize, selected: usize) -> usize {
    let Some(room) = room(area, titles) else {
        return 0;
    };
    let widths: Vec<i32> = widths(titles).collect();
    let last = (0..widths.len())
        .find(|&start| widths[start..].iter().sum::<i32>() <= room)
        .unwrap_or(widths.len() - 1);
    let mut first = first.min(selected).min(last);
    while first < selected && widths[first..=selected].iter().sum::<i32>() > room {
        first += 1;
    }
    first
}

pub(crate) fn last(area: Rect, titles: &[&str]) -> usize {
    scrolled(area, titles, usize::MAX, titles.len().saturating_sub(1))
}

pub(crate) fn rows(area: Rect, count: usize) -> Vec<Rect> {
    (0..i32::try_from(count).unwrap_or(1))
        .map(|row| Rect::new(area.x, area.y + RISE * row, area.width, STRIP))
        .collect()
}

pub(crate) fn panel(area: Rect, count: usize) -> Rect {
    let rows = i32::try_from(count).unwrap_or(1).max(1);
    area.shrink(0, STRIP + RISE * (rows - 1), 0, 0)
}

pub(crate) fn content(panel: Rect) -> Rect {
    let row = panel.bottom() - MARGIN - BUTTON_HEIGHT;
    Rect::sides(panel.x + 18, panel.y + 17, panel.right() - MARGIN, row - 6)
}

pub(crate) fn tabs(ui: &mut Ui, area: Rect, strips: &[Strip<'_>]) -> Rect {
    let panel = panel(area, strips.len());
    controls::bevel(&mut ui.canvas, panel, BUTTON);
    for (strip, band) in strips.iter().zip(rows(area, strips.len())) {
        row(ui, band, strip);
    }
    panel
}

fn row(ui: &mut Ui, area: Rect, strip: &Strip<'_>) {
    let mut entries: Vec<(Option<usize>, &str)> = strip
        .titles
        .iter()
        .enumerate()
        .map(|(index, &title)| (Some(index), title))
        .collect();
    if let Some((gap, title)) = strip.guest {
        entries.insert(gap.min(entries.len()), (None, title));
    }
    entries.retain(|&(index, _)| index.is_none() || index != strip.absent);
    let titles: Vec<&str> = entries.iter().map(|&(_, title)| title).collect();
    let room = room(area, &titles);
    let gap = strip.guest.map_or(0, |(gap, _)| gap);
    let first = entries
        .iter()
        .position(|&(index, _)| index.unwrap_or(gap) >= strip.first)
        .filter(|_| room.is_some())
        .unwrap_or(0);
    let clip = Rect::sides(
        area.x,
        area.y,
        room.map_or(area.right(), |room| area.x + 2 + room),
        area.bottom() + 2,
    );
    let mut left = area.x + 2;
    let mut chosen = None;
    let mut leading = None;
    for (&(index, title), width) in entries.iter().zip(widths(&titles)).skip(first) {
        let tab = Rect::new(left, area.y + 2, width, STRIP - 2);
        left = tab.right();
        if tab.x >= clip.right() {
            break;
        }
        let Some(index) = index else {
            ui.canvas
                .clipped(clip, |canvas| draw_tab(canvas, tab, title, 6, true));
            continue;
        };
        ui.target(tab.intersect(clip), Target::Tab(strip.depth, index));
        leading.get_or_insert(tab);
        if strip.selected == Some(index) {
            chosen = Some((tab, title));
        } else {
            let lit = strip.drop == Some(index);
            ui.canvas
                .clipped(clip, |canvas| draw_tab(canvas, tab, title, 6, lit));
        }
    }
    if let Some((tab, title)) = chosen {
        let raised = Rect::sides(tab.x - 2, area.y, tab.right() + 2, tab.bottom() + 1);
        let lit = strip.drop.is_some() && strip.drop == strip.selected;
        ui.canvas
            .clipped(clip, |canvas| draw_tab(canvas, raised, title, 8, lit));
        if strip.focused && ui.cues.focus {
            let inner = Rect::new(tab.x + 1, area.y + 3, tab.width - 2, 16);
            controls::focus(&mut ui.canvas, inner.intersect(clip), FACE);
        }
    }
    if let (None, Some(tab), None) = (chosen, leading, strip.absent)
        && strip.focused
        && ui.cues.focus
    {
        let inner = Rect::new(tab.x + 3, tab.y + 2, tab.width - 6, 14);
        controls::focus(&mut ui.canvas, inner.intersect(clip), FACE);
    }
    if room.is_some() {
        let last = last(area, strip.titles);
        for (forth, enabled) in [(false, first > 0), (true, first < last)] {
            let button = Rect::new(
                area.right() - SPIN * (2 - i32::from(forth)),
                area.y,
                SPIN,
                SPIN,
            );
            let target = Target::Spin(strip.depth, forth);
            let state = match (enabled, ui.held(target)) {
                (false, _) => State::Disabled,
                (true, true) => State::Pushed,
                (true, false) => State::Normal,
            };
            scroll::arrow(&mut ui.canvas, button, false, forth, state);
            ui.target(button, target);
        }
    }
}

fn draw_tab(canvas: &mut Canvas, tab: Rect, label: &str, indent: i32, lit: bool) {
    canvas.fill(tab.shrink(1, 1, 2, 0), FACE);
    canvas.fill(Rect::new(tab.x, tab.y + 2, 1, tab.height - 2), WHITE);
    canvas.plot(tab.x + 1, tab.y + 1, WHITE);
    canvas.fill(Rect::new(tab.x + 2, tab.y, tab.width - 4, 1), WHITE);
    canvas.plot(tab.right() - 2, tab.y + 1, DARK);
    canvas.fill(
        Rect::new(tab.right() - 2, tab.y + 2, 1, tab.height - 2),
        SHADOW,
    );
    canvas.fill(
        Rect::new(tab.right() - 1, tab.y + 2, 1, tab.height - 2),
        DARK,
    );
    controls::label(
        canvas,
        label,
        tab.x + indent,
        tab.y + 3,
        Ink::Plain(BLACK),
        false,
    );
    if lit {
        let inner = Rect::new(
            tab.x + indent - 3,
            tab.y + 1,
            controls::width(label) + 6,
            16,
        );
        controls::focus(canvas, inner, FACE);
    }
}

pub(crate) fn buttons(
    ui: &mut Ui,
    panel: Rect,
    focus: Option<Focus>,
    buttons: &[(&str, Command, bool)],
    width: i32,
) {
    let row = panel.bottom() - MARGIN - BUTTON_HEIGHT;
    let mut right = content(panel).right();
    for &(label, command, enabled) in buttons.iter().rev() {
        let button = Rect::new(right - width, row, width, BUTTON_HEIGHT);
        let target = Target::Button(command);
        let state = match (enabled, ui.held(target)) {
            (false, _) => State::Disabled,
            (true, true) => State::Pushed,
            (true, false) => State::Normal,
        };
        let cues = Cues {
            keys: ui.cues.keys,
            focus: ui.cues.focus && focus == Some(Focus::Button(command)),
        };
        controls::button(&mut ui.canvas, &FONT, button, label, state, cues, false);
        if enabled {
            ui.target(button, target);
        }
        right = button.x - GAP;
    }
}
