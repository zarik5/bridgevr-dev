mod context;
mod texture;

use ash::{extensions::khr, version::DeviceV1_0, *};
use bridgevr_common::*;
use std::sync::Arc;

pub use context::*;
pub use texture::*;

pub enum Operation {}

pub enum OperationBufferInternal {
    Render,
    Present {
        swapchain_loader: khr::Swapchain,
        swapchain: vk::SwapchainKHR,
        swapchain_images: Vec<vk::Image>,
        swapchain_format: vk::Format,
        swapchain_extent: vk::Extent2D,
    },
}

pub struct OperationBuffer {
    graphics_context: Arc<GraphicsContext>,
    internal_data: OperationBufferInternal,
}

impl OperationBuffer {
    pub fn new_present(
        graphics_context: Arc<GraphicsContext>,
        texture: Arc<Texture>,
    ) -> StrResult<Self> {
        let surface = trace_err!(texture.get_surface())?;

        let swapchain_capabilities = trace_err!(unsafe {
            graphics_context
                .surface_loader
                .get_physical_device_surface_capabilities(graphics_context.physical_device, surface)
        })?;
        let swapchain_formats = trace_err!(unsafe {
            graphics_context
                .surface_loader
                .get_physical_device_surface_formats(graphics_context.physical_device, surface)
        })?;
        let swapchain_present_modes = trace_err!(unsafe {
            graphics_context
                .surface_loader
                .get_physical_device_surface_present_modes(
                    graphics_context.physical_device,
                    surface,
                )
        })?;

        let surface_format = trace_none!(swapchain_formats.iter().find(|f| {
            f.format == vk::Format::B8G8R8A8_SRGB
                && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
        }))?;

        let present_mode = trace_none!(swapchain_present_modes
            .iter()
            .find(|&&pm| pm == vk::PresentModeKHR::FIFO))?;

        let swapchain_extent = swapchain_capabilities.current_extent;

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface)
            .min_image_count(swapchain_capabilities.min_image_count + 1)
            .image_color_space(surface_format.color_space)
            .image_format(surface_format.format)
            .image_extent(swapchain_extent)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(swapchain_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(*present_mode)
            .clipped(true)
            .image_array_layers(1); // > 1 if using VR (multiview?)

        // queue_family_indices: only if using more than one queue
        // image_sharing_mode(vk::SharingMode::CONCURRENT) if using more than one queue

        let swapchain_loader =
            khr::Swapchain::new(&graphics_context.instance, &graphics_context.device);

        let swapchain =
            trace_err!(unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None) })?;

        let swapchain_images =
            trace_err!(unsafe { swapchain_loader.get_swapchain_images(swapchain) })?;

        Ok(Self {
            graphics_context,
            internal_data: OperationBufferInternal::Present {
                swapchain_loader,
                swapchain,
                swapchain_format: surface_format.format,
                swapchain_extent,
                swapchain_images,
            },
        })
    }
}

impl Drop for OperationBuffer {
    fn drop(&mut self) {
        if let OperationBufferInternal::Present {
            swapchain_loader,
            swapchain,
            ..
        } = &self.internal_data
        {
            unsafe { swapchain_loader.destroy_swapchain(*swapchain, None) };
        }
    }
}
