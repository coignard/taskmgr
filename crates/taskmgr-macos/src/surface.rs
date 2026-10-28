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

use std::ptr;

use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_core_foundation::{CFData, CFRetained, CFType};
use objc2_core_graphics::{
    CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo,
    kCGColorSpaceSRGB,
};
use objc2_foundation::NSRect;
use objc2_quartz_core::{CALayer, CATransaction, kCAFilterNearest};
use taskmgr::graphics::Canvas;

const BYTES_PER_PIXEL: usize = size_of::<[u8; 4]>();
const BITS_PER_COMPONENT: usize = u8::BITS as usize;

pub(crate) struct Surface {
    layer: Retained<CALayer>,
    color_space: Option<CFRetained<CGColorSpace>>,
}

impl Surface {
    pub(crate) fn new() -> Self {
        let layer = CALayer::new();
        layer.setMagnificationFilter(unsafe { kCAFilterNearest });
        layer.setMinificationFilter(unsafe { kCAFilterNearest });
        Self {
            layer,
            color_space: CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })),
        }
    }

    pub(crate) fn host_in(&self, view: &NSView) {
        view.setLayer(Some(&self.layer));
        view.setWantsLayer(true);
    }

    pub(crate) fn present(&self, canvas: &Canvas, frame: NSRect) {
        let Some(image) = self.image(canvas) else {
            return;
        };
        let contents: &CFType = &image;
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        self.layer.setFrame(frame);
        unsafe { self.layer.setContents(Some(contents.as_ref())) };
        CATransaction::commit();
    }

    fn image(&self, canvas: &Canvas) -> Option<CFRetained<CGImage>> {
        let width = usize::try_from(canvas.size().width).ok()?;
        let height = usize::try_from(canvas.size().height).ok()?;
        let data = CFData::from_bytes(canvas.rgba());
        let provider = CGDataProvider::with_cf_data(Some(&data))?;
        unsafe {
            CGImage::new(
                width,
                height,
                BITS_PER_COMPONENT,
                BITS_PER_COMPONENT * BYTES_PER_PIXEL,
                width * BYTES_PER_PIXEL,
                self.color_space.as_deref(),
                CGBitmapInfo(CGImageAlphaInfo::PremultipliedLast.0),
                Some(&provider),
                ptr::null(),
                false,
                CGColorRenderingIntent::RenderingIntentDefault,
            )
        }
    }
}
