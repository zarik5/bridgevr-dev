use crate::{constants::BVR_NAME, StrResult};
use gfx_hal::{
    adapter::MemoryType,
    buffer,
    command::CommandBuffer,
    command::*,
    format::*,
    image::*,
    memory,
    pass::Subpass,
    pool::*,
    prelude::*,
    pso::*,
    queue::{QueueGroup, Submission},
    *,
};
use log::{debug, error};
use parking_lot::Mutex;
use std::{
    any::TypeId, ffi::c_void, fmt::Debug, iter, marker::PhantomData, mem::ManuallyDrop, mem::*,
    ptr, sync::Arc, time::Duration,
};

pub use gfx_hal::format::Format;

#[cfg(any(target_os = "linux", target_os = "android"))]
use gfx_backend_vulkan as back;

#[cfg(windows)]
use gfx_backend_dx11 as back;

#[cfg(target_os = "macos")]
use gfx_backend_metal as back;

const GRAPHICS_TIMEOUT: Duration = Duration::from_secs(1);

const TRACE_CONTEXT: &str = "Rendering Utils";

type InstanceImpl = <back::Backend as gfx_hal::Backend>::Instance;
type PhysicalDeviceImpl = <back::Backend as gfx_hal::Backend>::PhysicalDevice;
type DeviceImpl = <back::Backend as gfx_hal::Backend>::Device;
type BufferImpl = <back::Backend as gfx_hal::Backend>::Buffer;
type MemoryImpl = <back::Backend as gfx_hal::Backend>::Memory;
type ImageImpl = <back::Backend as gfx_hal::Backend>::Image;
type ImageViewImpl = <back::Backend as gfx_hal::Backend>::ImageView;

#[cfg(windows)]
macro_rules! addr_of {
    ($e:expr) => {
        &mut $e as *mut _ as _
    };
}

#[cfg(windows)]
pub fn format_from_native(dxgi_format: u32) -> Format {
    use winapi::shared::dxgiformat::*;
    match dxgi_format {
        DXGI_FORMAT_R8G8B8A8_UNORM => Format::Rgba8Unorm,
        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => Format::Rgba8Srgb,
        _ => Format::Rgba8Unorm,
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn format_from_native(dxgi_format: u32) -> Format {
    todo!();
}

pub struct TextureGuard {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    ptr: ash::vk::Image,

    #[cfg(windows)]
    ptr: wio::com::ComPtr<winapi::um::d3d11::ID3D11Texture2D>,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn lock_texture_from_handle(handle: u64, timeout: Duration) -> StrResult<TextureGuard> {
    todo!()
}

#[cfg(windows)]
pub fn lock_texture_from_handle(handle: u64, timeout: Duration) -> StrResult<TextureGuard> {
    todo!()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn unlock_texture(texture_guard: TextureGuard) {
    todo!()
}

#[cfg(windows)]
pub fn unlock_texture(texture_guard: TextureGuard) {
    todo!()
}

#[derive(Clone, Copy)]
pub struct TextureBounds {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
}

pub enum BufferUsage {
    Mutable,
    Immutable,
}

// Dependency graph is inferred by the order of the operations and variant fields.
#[derive(Clone)]
pub enum OperationDesc {
    Rendering {
        input_textures: Vec<Arc<Texture>>,
        uniform_buffer: Option<Arc<UniformBuffer>>,
        shader: String,
        output_textures: Vec<Arc<Texture>>,
        alpha: bool,
    },
    CopyTexture {
        input: Arc<Texture>,
        bounds: TextureBounds,
        output: Arc<Texture>,
    },
}

// Graphics abstraction layer 2D
// Expose a higher level API for 2D post-processing using shaders
pub struct GraphicsAbstractionLayer {
    instance: InstanceImpl,
    physical_device: PhysicalDeviceImpl,
    device: DeviceImpl,
    queue_group: QueueGroup<back::Backend>,
    memory_types: Vec<MemoryType>,
    limits: Limits,
}

impl GraphicsAbstractionLayer {
    pub fn new(adapter_index: Option<usize>) -> StrResult<Self> {
        let instance = trace_err!(back::Instance::create(BVR_NAME, 1))?;

        let mut adapters = instance.enumerate_adapters();
        let adapter_index = adapter_index.unwrap_or(0);

        debug!("Selecting graphics adapter {} of:", adapter_index);
        for i in 0..adapters.len() {
            debug!("{}: {:?}", i, adapters[i].info);
        }
        let adapter = adapters.remove(adapter_index);
        let physical_device = adapter.physical_device;
        let memory_types = physical_device.memory_properties().memory_types;
        let limits = physical_device.limits();

        let queue_family = trace_none!(adapter
            .queue_families
            .iter()
            .find(|qf| qf.queue_type().supports_graphics()))?; // todo filter for surface support

        let features = Features::empty(); // todo add features
        let mut gpu =
            trace_err!(unsafe { physical_device.open(&[(&queue_family, &[1.0])], features) })?;

        let device = gpu.device;
        let queue_group = trace_none!(gpu.queue_groups.pop())?;

        Ok(GraphicsAbstractionLayer {
            instance,
            physical_device,
            device,
            queue_group,
            memory_types,
            limits,
        })
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn from_device_ptr(device_ptr: u64) -> StrResult<Self> {
        todo!();
    }

    #[cfg(windows)]
    pub fn from_device_ptr(device_ptr: u64) -> StrResult<Self> {
        todo!();
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn instance_ptr(&self) -> u64 {
        use ash::{version::InstanceV1_0, vk::Handle};

        self.instance.raw.0.handle().as_raw()
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn physical_device_ptr(&self) -> u64 {
        use ash::vk::{self, Handle};

        struct PhysicalDeviceMock {
            _instance: Arc<back::RawInstance>,
            handle: vk::PhysicalDevice,
        }

        // todo: remove transmute if gfx will support accessing raw handle
        let physical_device_mock: &PhysicalDeviceMock =
            unsafe { &*(&self.physical_device as *const _ as *const _) };
        physical_device_mock.handle.as_raw()
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn device_ptr(&self) -> u64 {
        use ash::vk::Handle;

        // Backend::Device has a field raw but it is private.
        // Extracting the memory with transmute
        // todo: remove transmute if gfx will support accessing raw handle
        let raw: &Arc<gfx_backend_vulkan::RawDevice> =
            unsafe { &*(&self.device as *const _ as *const _) };
        raw.0.handle().as_raw()
    }

    #[cfg(windows)]
    pub fn device_ptr(&self) -> u64 {
        use winapi::um::d3d11;
        use wio::com::ComPtr;

        // todo: remove transmute if gfx will support accessing raw handle
        let raw: &ComPtr<d3d11::ID3D11Device> = unsafe { &*(&self.device as *const _ as *const _) };
        raw.as_raw() as _
    }
}

// impl Drop for Graphics2DAbstractionLayer {
//     fn drop(&mut self) {
//         unsafe {
//             // NB: inverse order of creation
//             // ManuallyDrop::drop(&mut self.graphics_queue_group);
//             // ManuallyDrop::drop(&mut self.device);
//             // ManuallyDrop::drop(&mut self.instance);
//         }
//     }
// }

pub struct UniformBuffer {
    graphics_al: Arc<GraphicsAbstractionLayer>,
    buffer_handle: ManuallyDrop<BufferImpl>,
    buffer_memory: ManuallyDrop<MemoryImpl>,
    struct_type: TypeId,
}

impl UniformBuffer {
    pub fn new<T: 'static>(graphics_al: Arc<GraphicsAbstractionLayer>) -> StrResult<Self> {
        let dev = &graphics_al.device;

        let non_coherent_alignment = graphics_al.limits.non_coherent_atom_size;
        let buffer_length = ((size_of::<T>() + non_coherent_alignment - 1)
            / non_coherent_alignment)
            * non_coherent_alignment;
        let mut buffer_handle = ManuallyDrop::new(trace_err!(unsafe {
            dev.create_buffer(buffer_length as u64, buffer::Usage::UNIFORM)
        })?);

        let buffer_requirements = unsafe { dev.get_buffer_requirements(&buffer_handle) };

        let mem_type_id =
            trace_none!(graphics_al
                .memory_types
                .iter()
                .enumerate()
                .position(|(id, mem_type)| {
                    buffer_requirements.type_mask & (1 << id) != 0
                        && mem_type
                            .properties
                            .contains(memory::Properties::CPU_VISIBLE)
                }))?
            .into();

        let buffer_memory = ManuallyDrop::new(unsafe {
            let mem = trace_err!(dev.allocate_memory(mem_type_id, buffer_requirements.size))?;
            trace_err!(dev.bind_buffer_memory(&mem, 0, &mut buffer_handle))?;
            mem
        });

        Ok(UniformBuffer {
            graphics_al,
            buffer_handle,
            buffer_memory,
            struct_type: TypeId::of::<T>(),
        })
    }

    pub fn write<T: 'static>(&self, data: &T) -> StrResult {
        // Cannot check this at compilation time: UniformBuffer cannot have type parameters because
        // I want multiple UniformBuffer with different struct types in the same operation desc vec.
        debug_assert_eq!(TypeId::of::<T>(), self.struct_type);

        let data_size = size_of::<T>();
        unsafe {
            let mapping = trace_err!(self
                .graphics_al
                .device
                .map_memory(&self.buffer_memory, 0..data_size as _))?;

            ptr::copy_nonoverlapping(data as *const _ as *const u8, mapping, data_size);

            // do not early return if flush fails because the buffer memory must be unmapped
            self.graphics_al
                .device
                .flush_mapped_memory_ranges(iter::once((&*self.buffer_memory, 0..data_size as _)))
                .map_err(|e| error!("[Graphics] Buffer map flush: {}", e))
                .ok();
            self.graphics_al.device.unmap_memory(&self.buffer_memory);
        }
        Ok(())
    }
}

impl Drop for UniformBuffer {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.buffer_memory);
            ManuallyDrop::drop(&mut self.buffer_handle);
        }
    }
}

pub struct Texture {
    graphics_al: Arc<GraphicsAbstractionLayer>,
    image_handle: ManuallyDrop<ImageImpl>,
    image_memory: ManuallyDrop<MemoryImpl>,
    image_view: ManuallyDrop<ImageViewImpl>,
    resolution: (u32, u32),
    format: Format,
}

impl Texture {
    fn create_image_memory_view(
        mut image_handle: &mut ManuallyDrop<ImageImpl>,
        graphics_al: Arc<GraphicsAbstractionLayer>,
        format: Format,
    ) -> StrResult<(ManuallyDrop<MemoryImpl>, ManuallyDrop<ImageViewImpl>)> {
        let dev = &graphics_al.device;

        let image_requirements = unsafe { dev.get_image_requirements(&image_handle) };

        let mem_type_id = trace_none!(graphics_al.memory_types.iter().enumerate().position(
            |(id, memory_type)| {
                image_requirements.type_mask & (1 << id) != 0
                    && memory_type
                        .properties
                        .contains(memory::Properties::DEVICE_LOCAL)
            }
        ))?
        .into();

        let image_memory = ManuallyDrop::new(unsafe {
            let mem = trace_err!(dev.allocate_memory(mem_type_id, image_requirements.size))?;
            trace_err!(dev.bind_image_memory(&mem, 0, &mut image_handle))?;
            mem
        });

        let image_view = ManuallyDrop::new(trace_err!(unsafe {
            dev.create_image_view(
                &image_handle,
                ViewKind::D2,
                format,
                Swizzle::NO,
                SubresourceRange {
                    aspects: format::Aspects::COLOR,
                    levels: 0..1,
                    layers: 0..1,
                },
            )
        })?);

        Ok((image_memory, image_view))
    }

    pub fn new(
        graphics_al: Arc<GraphicsAbstractionLayer>,
        (width, height): (u32, u32),
        format: Format,
        sample_count: Option<u8>,
    ) -> StrResult<Self> {
        let dev = &graphics_al.device;

        let sample_count = sample_count.unwrap_or(1);
        let kind = Kind::D2(width, height, /*layers*/ 1, sample_count);
        //todo check usage bits
        let usage = image::Usage::SAMPLED | image::Usage::STORAGE | image::Usage::COLOR_ATTACHMENT;

        let mut image_handle = ManuallyDrop::new(trace_err!(unsafe {
            dev.create_image(
                kind,
                1,
                format,
                Tiling::Optimal,
                usage,
                ViewCapabilities::empty(),
            )
        })?);

        let (image_memory, image_view) =
            Texture::create_image_memory_view(&mut image_handle, graphics_al.clone(), format)?;

        Ok(Self {
            graphics_al,
            image_handle,
            image_memory,
            image_view,
            resolution: (width, height),
            format,
        })
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn from_handle(handle: u64, graphics_al: Arc<GraphicsAbstractionLayer>) -> StrResult<Self> {
        trace_str!("Cannot create image from handle")
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn from_handle_and_desc(
        handle: u64,
        graphics_al: Arc<GraphicsAbstractionLayer>,
        (width, height): (u32, u32),
        format: Format,
    ) -> StrResult<Self> {
        todo!();
    }

    #[cfg(windows)]
    pub fn from_handle(handle: u64, graphics_al: Arc<GraphicsAbstractionLayer>) -> StrResult<Self> {
        todo!();
    }

    #[cfg(windows)]
    pub fn from_ptr(ptr: u64, graphics_al: Arc<GraphicsAbstractionLayer>) -> StrResult<Self> {
        todo!();
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn as_ptr(&self) -> u64 {
        todo!();
    }

    #[cfg(windows)]
    pub fn as_ptr(&self) -> u64 {
        todo!();
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn as_handle(&self) -> u64 {
        self.as_ptr()
    }

    #[cfg(windows)]
    pub fn as_handle(&self) -> u64 {
        todo!();
    }

    pub fn read(&self) -> StrResult<Vec<u8>> {
        todo!();
    }

    pub fn write(&self, data: Vec<u8>) -> StrResult {
        todo!();
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.image_view);
            ManuallyDrop::drop(&mut self.image_memory);
            ManuallyDrop::drop(&mut self.image_handle);
        }
    }
}

// including Graphics2DAbstractionLayer inside Texture ensures that image is destroyed before the device
// impl Drop for Texture {
//     fn drop(&mut self) {
//         if self.graphics_al.device.wait_idle().is_ok() {
//             unsafe {
//                 self.graphics_al
//                     .device
//                     .destroy_image(ManuallyDrop::into_inner(ptr::read(&mut self.image)));
//                 // todo: use ManuallyDrop::read/take when stabilized
//             }
//         }
//     }
// }

// enum Operation {
//     RenderPass(<back::Backend as gfx_hal::Backend>::RenderPass),
//     FillBuffer,
//     WaitForTexture,
// }

pub struct OperationBuffer {
    graphics_al: Arc<GraphicsAbstractionLayer>,
    // command_pool: ManuallyDrop<CommandPool<back::Backend, Graphics>>,
    // command_buffer: ManuallyDrop<CommandBuffer<back::Backend, Graphics, MultiShot>>,
    // render_passes: Vec<<back::Backend as gfx_hal::Backend>::RenderPass>,
    // fences: Vec<<back::Backend as gfx_hal::Backend>::Fence>,
    // semaphores: Vec<<back::Backend as gfx_hal::Backend>::Semaphore>,
}

impl OperationBuffer {
    pub fn new(
        graphics_al: Arc<GraphicsAbstractionLayer>,
        operation_descs: &[OperationDesc],
    ) -> StrResult<OperationBuffer> {
        let dev = &graphics_al.device;

        let mut command_pool = ManuallyDrop::new(trace_err!(unsafe {
            dev.create_command_pool(
                graphics_al.queue_group.family,
                CommandPoolCreateFlags::RESET_INDIVIDUAL,
            )
        })?);

        let mut command_buffer = unsafe { command_pool.allocate_one(command::Level::Primary) };
        unsafe { command_buffer.begin_primary(CommandBufferFlags::EMPTY) };

        for op_desc in operation_descs {
            match op_desc {
                OperationDesc::Rendering {
                    input_textures,
                    uniform_buffer,
                    shader,
                    output_textures,
                    alpha,
                } => {}
                OperationDesc::CopyTexture {
                    input,
                    output,
                    bounds,
                } => {}
            }
        }

        let bindings = [
            DescriptorSetLayoutBinding {
                binding: 0,
                ty: pso::DescriptorType::SampledImage,
                count: 1,
                stage_flags: ShaderStageFlags::FRAGMENT,
                immutable_samplers: false,
            },
            DescriptorSetLayoutBinding {
                binding: 1,
                ty: pso::DescriptorType::Sampler,
                count: 1,
                stage_flags: ShaderStageFlags::FRAGMENT,
                immutable_samplers: false,
            },
        ];
        let set_layout = ManuallyDrop::new(trace_err!(unsafe {
            dev.create_descriptor_set_layout(&bindings, &[])
        })?);

        let mut desc_pool = ManuallyDrop::new(trace_err!(unsafe {
            dev.create_descriptor_pool(
                1, // sets
                &[
                    DescriptorRangeDesc {
                        ty: DescriptorType::SampledImage,
                        count: 1,
                    },
                    DescriptorRangeDesc {
                        ty: DescriptorType::Sampler,
                        count: 1,
                    },
                ],
                DescriptorPoolCreateFlags::empty(),
            )
        })?);
        let desc_set = trace_err!(unsafe { desc_pool.allocate_set(&set_layout) })?;

        let sampler = trace_err!(unsafe {
            dev.create_sampler(&SamplerDesc::new(Filter::Linear, WrapMode::Clamp))
        })?;

        // let mut command_buffer =
        //     ManuallyDrop::new(command_pool.acquire_command_buffer::<MultiShot>());

        // let fences = vec![];
        // let semaphores = vec![];
        // let render_passes = vec![];

        //todo: use Arc::ptr_eq for checking if textures are the same object

        // unsafe {
        //     command_buffer.begin(/*pending resubmits*/ false)
        // };

        // command_buffer.begin_render_pass_inline (
        // )

        // let fence = graphics_al
        //     .device
        //     .create_fence(/*signaled*/ true)
        //     .unwrap();
        // let semaphore = graphics_al.device.create_semaphore().unwrap();

        // let render_pass = {
        //     let color_attachment = Attachment {
        //         format: Some(Format::Rgba8Unorm), // todo: infer from texture
        //         samples: 1,
        //         ops: AttachmentOps {
        //             load: AttachmentLoadOp::DontCare,
        //             store: AttachmentStoreOp::Store,
        //         },
        //         stencil_ops: AttachmentOps::DONT_CARE,
        //         layouts: Layout::Undefined..Layout::Present,
        //     };
        //     let subpass = SubpassDesc {
        //         colors: &[(0, Layout::ColorAttachmentOptimal)],
        //         depth_stencil: None,
        //         inputs: &[],
        //         resolves: &[],
        //         preserves: &[],
        //     };

        //     unsafe {
        //         ok_or_panic!(graphics_al.device.create_render_pass(
        //             &[color_attachment],
        //             &[subpass],
        //             &[/*subpass dependency*/],
        //         ))
        //     }
        // };

        //let image: <back::Device as gfx_hal::Device>::Backend::Image;
        // ok_or_panic!(device.create_image_view(
        //     &image,
        //     ViewKind::D2,
        //     Format::Rgba8Unorm, // todo: infer from texture
        //     Swizzle::NO,
        //     SubresourceRange {
        //         aspects: Aspects::COLOR,
        //         levels: ..1,
        //         layers: ..1,
        //     },
        // ), "Image view");

        // let framebuffer = ok_or_panic!(graphics_al.device.create_framebuffer(
        //     &render_pass,
        //     vec![image_view],
        //     Extent {
        //         width: extent.width as u32,
        //         height: extent.height as u32,
        //         depth: 1,
        //     },
        // ), "Framebuffer");

        todo!();

        // Self {
        //     graphics_al,
        //     command_pool,
        //     command_buffer,
        //     fences,
        //     semaphores,
        //     render_passes,
        // }
    }

    pub fn execute(&self) {
        todo!();
    }
}

// impl Drop for OperationBuffer {
//     fn drop(&mut self) {
//         if self.graphics_al.device.wait_idle().is_ok() {
//             unsafe {
//                 // for f in self.fences.drain(..) {
//                 //     self.graphics_al.device.destroy_fence(f);
//                 // }

//                 // let command_buffer = ManuallyDrop::into_inner(ptr::read(&mut self.command_buffer));

//                 // self.graphics_al.device.destroy_command_pool(
//                 //     ManuallyDrop::into_inner(ptr::read(&mut self.command_pool)).into_raw(),
//                 // );
//             }
//         }
//     }
// }
