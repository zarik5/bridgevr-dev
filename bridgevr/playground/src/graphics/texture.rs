use super::context::*;
use ash::{version::DeviceV1_0, *};
use bridgevr_common::*;
use std::sync::Arc;
use winit::window::Window;

unsafe fn create_vendor_surface(
    entry: &Entry,
    instance: &Instance,
    window: &Window,
) -> Result<vk::SurfaceKHR, vk::Result> {
    use ash::extensions::khr::XlibSurface;
    use winit::platform::unix::WindowExtUnix;

    let x11_display = window.xlib_display().unwrap();
    let x11_window = window.xlib_window().unwrap();
    let x11_create_info = vk::XlibSurfaceCreateInfoKHR::builder()
        .window(x11_window)
        .dpy(x11_display as *mut vk::Display);

    let xlib_surface_loader = XlibSurface::new(entry, instance);
    xlib_surface_loader.create_xlib_surface(&x11_create_info, None)
}

enum TextureInternal {
    Image(vk::Image),
    Surface(vk::SurfaceKHR),
}

pub struct Texture {
    graphics_context: Arc<GraphicsContext>,
    texture_handle: TextureInternal,
}

impl Texture {
    pub fn new_surface(
        graphics_context: Arc<GraphicsContext>,
        window: &Window,
    ) -> StrResult<Texture> {
        let texture_handle = TextureInternal::Surface(unsafe {
            trace_err!(create_vendor_surface(
                &graphics_context.entry,
                &graphics_context.instance,
                window
            ))?
        });

        Ok(Self {
            graphics_context,
            texture_handle,
        })
    }

    pub(super) fn get_surface(&self) -> StrResult<vk::SurfaceKHR> {
        match self.texture_handle {
            TextureInternal::Surface(surface) => Ok(surface),
            _ => trace_str!("Texture is an image!"),
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        match self.texture_handle {
            TextureInternal::Surface(surface) => unsafe {
                self.graphics_context
                    .surface_loader
                    .destroy_surface(surface, None)
            },
            TextureInternal::Image(_) => {}
        }
    }
}
