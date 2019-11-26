// use crate::logging::*;
// use crate::rendering_utils_interface::*;
// use log::*;
// use std::collections::BTreeMap;
// use std::sync::Arc;
// use vulkano::{
//     buffer::{BufferUsage, CpuAccessibleBuffer},
//     command_buffer::{
//         AutoCommandBuffer, AutoCommandBufferBuilder, CommandBuffer, CommandBufferExecFuture,
//     },
//     device::{Device, DeviceExtensions, Features, Queue},
//     instance::{Instance, InstanceExtensions, PhysicalDevice},
//     sync::{GpuFuture, NowFuture},
// };

// pub struct VulkanTexture(i32);
// pub struct VulkanBuffer(i32);

// pub struct VulkanObject {
//     device_rc: Arc<Device>,
//     queue_rc: Arc<Queue>,
//     compiled_ops: BTreeMap<usize, CompiledOp>,
//     operations_invalidated: bool,
//     reuse_primary_command_buffer: bool,
//     primary_command_buffer: Arc<AutoCommandBuffer>,
// }

// impl VulkanObject {
//     pub fn new(reuse_primary_command_buffer: bool) -> Self {
//         let instance = ok_or_panic!(
//             Instance::new(None, &InstanceExtensions::none(), None),
//             "Vulkan initialization"
//         );

//         let physical_device = some_or_panic!(
//             PhysicalDevice::enumerate(&instance).next(),
//             "Vulkan: no device available"
//         );
//         let queue_family = some_or_panic!(
//             physical_device
//                 .queue_families()
//                 .find(|&q| q.supports_graphics()),
//             "Vulkan: couldn't find a graphical queue family"
//         );

//         let (device_rc, mut queues) = ok_or_panic!(
//             Device::new(
//                 physical_device,
//                 &Features::none(),
//                 &DeviceExtensions::none(),
//                 [(queue_family, 1f32)].iter().cloned()
//             ),
//             "Vulkan initialization"
//         );
//         let queue_rc = queues.next().unwrap();

//         let empty_command_buffer =
//             AutoCommandBufferBuilder::new(device_rc.clone(), queue_rc.family())
//                 .unwrap()
//                 .build()
//                 .unwrap();

//         VulkanObject {
//             device_rc,
//             queue_rc,
//             compiled_ops: BTreeMap::new(),
//             operations_invalidated: false,
//             reuse_primary_command_buffer,
//             primary_command_buffer: Arc::new(empty_command_buffer),
//         }
//     }

//     fn compile_and_insert_op(&mut self, index: usize, op: VulkanOp) {
//         let compiled_op = match op {
//             GraphicsOp::Rendering(_) => panic!(),
//             GraphicsOp::RenderingGroup(_) => panic!(),
//         };

//         self.compiled_ops.insert(index, compiled_op);
//         self.operations_invalidated = true;
//     }
// }

// impl GraphicsObject for VulkanObject {
//     type Buffer = VulkanBuffer;
//     type Texture = VulkanTexture;

//     fn create_texture() -> VulkanTexture {
//         VulkanTexture(0)
//     }

//     fn push_operation(&mut self, operation: GraphicsOp) -> usize {
//         let next_idx = match self.compiled_ops.iter().next_back() {
//             Some((i, _)) => i + 1,
//             None => 0,
//         };
//         self.compile_and_insert_op(next_idx, operation);
//         next_idx
//     }

//     fn set_operation(&mut self, index: usize, operation: GraphicsOp) {
//         self.compile_and_insert_op(index, operation);
//     }

//     fn render(&mut self) {
//         if self.operations_invalidated || !self.reuse_primary_command_buffer {
//             let empty_builder =
//                 AutoCommandBufferBuilder::new(self.device_rc.clone(), self.queue_rc.family())
//                     .unwrap();
//             let filled_builder = self.compiled_ops.iter().fold(empty_builder, |b, op| b);
//             self.primary_command_buffer = Arc::new(filled_builder.build().unwrap());
//         }

//         self.primary_command_buffer
//             .clone()
//             .execute(self.queue_rc.clone())
//             .unwrap()
//             .then_signal_fence_and_flush()
//             .unwrap()
//             .wait(None)
//             .unwrap();
//     }
// }
