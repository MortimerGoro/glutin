#![cfg(target_os = "android")]

extern crate android_glue;

use libc;

use CreationError::{self, OsError};

use winit;

use Api;
use ContextError;
use GlAttributes;
use GlContext;
use PixelFormat;
use PixelFormatRequirements;
use WindowAttributes;

use api::egl;
use api::egl::Context as EglContext;

use std::cell::Cell;

mod ffi;

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
    winit_iterator: winit::WaitEventsIterator<'a>,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = winit::Event;

    fn next(&mut self) -> Option<winit::Event> {
        let event = self.winit_iterator.next();
        if let Some(event) = event {
            return self.window.handle_event(event);
        }
        None
    }
}

pub struct PollEventsIterator<'a> {
    window: &'a Window,
    winit_iterator: winit::PollEventsIterator<'a>,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = winit::Event;

    fn next(&mut self) -> Option<winit::Event> {
        let event = self.winit_iterator.next();
        if let Some(event) = event {
            return self.window.handle_event(event);
        }
        None
    }
}

pub struct Window {
    context: EglContext,
    winit_window: winit::Window,
    stopped: Cell<bool>
}

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;

#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

impl Window {
    pub fn new(_: &WindowAttributes,
               pf_reqs: &PixelFormatRequirements,
               opengl: &GlAttributes<&Window>,
               _: &PlatformSpecificWindowBuilderAttributes,
               winit_builder: winit::WindowBuilder)
               -> Result<Window, CreationError> {
        let winit_window = winit_builder.build().unwrap();
        let opengl = opengl.clone().map_sharing(|w| &w.context);
        let native_window = unsafe { android_glue::get_native_window() };
        if native_window.is_null() {
            return Err(OsError(format!("Android's native window is null")));
        }
        let context = try!(EglContext::new(egl::ffi::egl::Egl,
                                           pf_reqs,
                                           &opengl,
                                           egl::NativeDisplay::Android)
            .and_then(|p| p.finish(native_window as *const _)));
        Ok(Window {
            context: context,
            winit_window: winit_window,
            stopped: Cell::new(false)
        })
    }

    pub fn handle_event(&self, event: winit::Event) -> Option<winit::Event> {
        match event {
            winit::Event::Suspended(suspended) => {
                if suspended {
                    self.on_surface_destroyed();
                } else {
                    self.on_surface_created();
                }
            }
            _ => {}
        };

        Some(event)
    }

    // Android has started the activity or sent it to foreground.
    // Restore the EGL surface and animation loop.
    fn on_surface_created(&self) {
        if self.stopped.get() {
           self.stopped.set(false);
           unsafe {
               let native_window = android_glue::get_native_window();
               self.context.on_surface_created(native_window as *const _);
           }

           // We stopped the renderloop when on_surface_destroyed was called.
           // We need to wakeup the event loop again.
           android_glue::wake_event_loop();
        }
    }

    // Android has stopped the activity or sent it to background.
    // Release the EGL surface and stop the animation loop.
    fn on_surface_destroyed(&self) {
        if !self.stopped.get() {
            self.stopped.set(true);
            unsafe {
                self.context.on_surface_destroyed();
            }
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.get()
    }

    pub fn set_title(&self, title: &str) {
        self.winit_window.set_title(title)
    }

    pub fn show(&self) {
        self.winit_window.show()
    }

    pub fn hide(&self) {
        self.winit_window.hide()
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        self.winit_window.get_position()
    }

    pub fn set_position(&self, x: i32, y: i32) {
        self.winit_window.set_position(x, y)
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        self.winit_window.get_inner_size()
    }

    pub fn get_inner_size_points(&self) -> Option<(u32, u32)> {
        self.winit_window.get_inner_size()
    }

    pub fn get_inner_size_pixels(&self) -> Option<(u32, u32)> {
        self.winit_window.get_inner_size().map(|(x, y)| {
            let hidpi = self.hidpi_factor();
            ((x as f32 * hidpi) as u32, (y as f32 * hidpi) as u32)
        })
    }

    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.winit_window.get_outer_size()
    }

    pub fn set_inner_size(&self, x: u32, y: u32) {
        self.winit_window.set_inner_size(x, y)
    }

    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self,
            winit_iterator: self.winit_window.poll_events()
        }
    }

    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self,
            winit_iterator: self.winit_window.wait_events()
        }
    }

    pub unsafe fn platform_display(&self) -> *mut libc::c_void {
        self.winit_window.platform_display()
    }

    #[inline]
    pub fn as_winit_window(&self) -> &winit::Window {
        &self.winit_window
    }
 
    #[inline]
    pub fn as_winit_window_mut(&mut self) -> &mut winit::Window {
        &mut self.winit_window
    }

    pub unsafe fn platform_window(&self) -> *mut libc::c_void {
        self.winit_window.platform_window()
    }

    pub fn create_window_proxy(&self) -> winit::WindowProxy {
        self.winit_window.create_window_proxy()
    }

    pub fn set_window_resize_callback(&mut self, callback: Option<fn(u32, u32)>) {
        self.winit_window.set_window_resize_callback(callback);
    }

    pub fn set_cursor(&self, cursor: winit::MouseCursor) {
        self.winit_window.set_cursor(cursor);
    }

    pub fn hidpi_factor(&self) -> f32 {
        self.winit_window.hidpi_factor()
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        self.winit_window.set_cursor_position(x, y)
    }

    pub fn set_cursor_state(&self, state: winit::CursorState) -> Result<(), String> {
        self.winit_window.set_cursor_state(state)
    }
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl GlContext for Window {
    #[inline]
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        if !self.stopped.get() {
            return self.context.make_current();
        }
        Err(ContextError::ContextLost)
    }

    #[inline]
    fn is_current(&self) -> bool {
        if self.stopped.get() {
            return false;
        }
        self.context.is_current()
    }

    #[inline]
    fn get_proc_address(&self, addr: &str) -> *const () {
        self.context.get_proc_address(addr)
    }

    #[inline]
    fn swap_buffers(&self) -> Result<(), ContextError> {
        if !self.stopped.get() {
            return self.context.swap_buffers();
        }
        Err(ContextError::ContextLost)
    }

    #[inline]
    fn get_api(&self) -> Api {
        self.context.get_api()
    }

    #[inline]
    fn get_pixel_format(&self) -> PixelFormat {
        self.context.get_pixel_format()
    }
}

#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    #[inline]
    pub fn wakeup_event_loop(&self) {
        android_glue::wake_event_loop();
    }
}

pub struct HeadlessContext(EglContext);

impl HeadlessContext {
    /// See the docs in the crate root file.
    pub fn new(dimensions: (u32, u32),
               pf_reqs: &PixelFormatRequirements,
               opengl: &GlAttributes<&HeadlessContext>,
               _: &PlatformSpecificHeadlessBuilderAttributes)
               -> Result<HeadlessContext, CreationError> {
        let opengl = opengl.clone().map_sharing(|c| &c.0);
        let context = try!(EglContext::new(egl::ffi::egl::Egl,
                                           pf_reqs,
                                           &opengl,
                                           egl::NativeDisplay::Android));
        let context = try!(context.finish_pbuffer(dimensions));     // TODO:
        Ok(HeadlessContext(context))
    }
}

unsafe impl Send for HeadlessContext {}
unsafe impl Sync for HeadlessContext {}

impl GlContext for HeadlessContext {
    #[inline]
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        self.0.make_current()
    }

    #[inline]
    fn is_current(&self) -> bool {
        self.0.is_current()
    }

    #[inline]
    fn get_proc_address(&self, addr: &str) -> *const () {
        self.0.get_proc_address(addr)
    }

    #[inline]
    fn swap_buffers(&self) -> Result<(), ContextError> {
        self.0.swap_buffers()
    }

    #[inline]
    fn get_api(&self) -> Api {
        self.0.get_api()
    }

    #[inline]
    fn get_pixel_format(&self) -> PixelFormat {
        self.0.get_pixel_format()
    }
}
