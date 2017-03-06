#![cfg(target_os = "android")]

extern crate android_glue;

use libc;

use CreationError::{self, OsError};

use winit;

use Api;
use ContextError;
use GlAttributes;
use PixelFormat;
use PixelFormatRequirements;

use api::egl;
use api::egl::Context as EglContext;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use {Event, WindowEvent};

mod ffi;

pub struct Context {
    egl_context: EglContext,
    stopped: Cell<bool>,
}

pub struct EventsLoop {
    winit_events_loop: winit::EventsLoop,
    contexts: RefCell<HashMap<winit::WindowId, Weak<Context>>>,
}

impl EventsLoop {
    /// Builds a new events loop.
    pub fn new() -> EventsLoop {
        EventsLoop {
            winit_events_loop: winit::EventsLoop::new(),
            contexts: RefCell::new(HashMap::new()),
        }
    }

    fn handle_suspended(&self, window_id: winit::WindowId, suspended: bool) {
        if let Some(context) = self.contexts.borrow()[&window_id].upgrade() {
            if context.stopped.get() == suspended {
                return;
            }
            context.stopped.set(suspended);
            if suspended {
                // Android has stopped the activity or sent it to background.
                // Release the EGL surface and stop the animation loop.
                unsafe {
                    context.egl_context.on_surface_destroyed();
                }
            } else {
                // Android has started the activity or sent it to foreground.
                // Restore the EGL surface and animation loop.
                unsafe {
                    let native_window = android_glue::get_native_window();
                    context.egl_context.on_surface_created(native_window as *const _);
                }
            }
        }
    }

    fn insert_window(&self,
                     window_id: winit::WindowId,
                     context: &Arc<Context>)
    {
        self.contexts.borrow_mut().insert(window_id, Arc::downgrade(context));
    }

    /// Fetches all the events that are pending, calls the callback function for each of them,
    /// and returns.
    #[inline]
    pub fn poll_events<F>(&self, mut callback: F)
        where F: FnMut(Event)
    {
        self.winit_events_loop.poll_events(|event| {
            if let Event::WindowEvent { window_id, event: WindowEvent::Suspended(suspended) } = event {
                self.handle_suspended(window_id, suspended)
            }
            callback(event);
        })
    }

    /// Runs forever until `interrupt()` is called. Whenever an event happens, calls the callback.
    #[inline]
    pub fn run_forever<F>(&self, mut callback: F)
        where F: FnMut(Event)
    {
        self.winit_events_loop.run_forever(|event| {
            if let Event::WindowEvent { window_id, event: WindowEvent::Suspended(suspended) } = event {
                self.handle_suspended(window_id, suspended)
            }
            callback(event);
        })
    }

    /// If we called `run_forever()`, stops the process of waiting for events.
    #[inline]
    pub fn interrupt(&self) {
        self.winit_events_loop.interrupt()
    }
}

#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

impl Context {
    pub fn new(
        window_builder: winit::WindowBuilder,
        events_loop: &winit::EventsLoop,
        pf_reqs: &PixelFormatRequirements,
        gl_attr: &GlAttributes<&Self>,
    ) -> Result<(winit::Window, Self), CreationError>
    {
        let window = try!(window_builder.build(events_loop));
        let gl_attr = gl_attr.clone().map_sharing(|c| &c.egl_context);
        let native_window = unsafe { android_glue::get_native_window() };
        if native_window.is_null() {
            return Err(OsError(format!("Android's native window is null")));
        }
        let egl = egl::ffi::egl::Egl;
        let native_display = egl::NativeDisplay::Android;
        let context = try!(EglContext::new(egl, pf_reqs, &gl_attr, native_display)
            .and_then(|p| p.finish(native_window as *const _)));
        let context = Context { egl_context: context, stopped: Cell::new(false) };
        Ok((window, context))
    }

    #[inline]
    pub unsafe fn make_current(&self) -> Result<(), ContextError> {
        self.egl_context.make_current()
    }

    #[inline]
    pub fn resize(&self, _: u32, _: u32) {
    }

    #[inline]
    pub fn is_current(&self) -> bool {
        self.egl_context.is_current()
    }

    #[inline]
    pub fn get_proc_address(&self, addr: &str) -> *const () {
        self.egl_context.get_proc_address(addr)
    }

    #[inline]
    pub fn swap_buffers(&self) -> Result<(), ContextError> {
        self.egl_context.swap_buffers()
    }

    #[inline]
    pub fn get_api(&self) -> Api {
        self.egl_context.get_api()
    }

    #[inline]
    pub fn get_pixel_format(&self) -> PixelFormat {
        self.egl_context.get_pixel_format()
    }
}

pub struct HeadlessContext(EglContext);

unsafe impl Send for HeadlessContext {}
unsafe impl Sync for HeadlessContext {}

impl HeadlessContext {
    /// See the docs in the crate root file.
    pub fn new(
        dimensions: (u32, u32),
        pf_reqs: &PixelFormatRequirements,
        gl_attr: &GlAttributes<&HeadlessContext>,
        _: &PlatformSpecificHeadlessBuilderAttributes,
    ) -> Result<Self, CreationError>
    {
        let gl_attr = gl_attr.clone().map_sharing(|c| &c.0);
        let context = try!(EglContext::new(egl::ffi::egl::Egl,
                                           pf_reqs,
                                           &gl_attr,
                                           egl::NativeDisplay::Android));
        let context = try!(context.finish_pbuffer(dimensions));     // TODO:
        Ok(HeadlessContext(context))
    }

    #[inline]
    pub unsafe fn make_current(&self) -> Result<(), ContextError> {
        self.0.make_current()
    }

    #[inline]
    pub fn is_current(&self) -> bool {
        self.0.is_current()
    }

    #[inline]
    pub fn get_proc_address(&self, addr: &str) -> *const () {
        self.0.get_proc_address(addr)
    }

    #[inline]
    pub fn swap_buffers(&self) -> Result<(), ContextError> {
        self.0.swap_buffers()
    }

    #[inline]
    pub fn get_api(&self) -> Api {
        self.0.get_api()
    }

    #[inline]
    pub fn get_pixel_format(&self) -> PixelFormat {
        self.0.get_pixel_format()
    }
}
