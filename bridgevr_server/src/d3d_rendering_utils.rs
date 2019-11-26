// use bridgevr_common::{rendering_utils_interface::*, *};
// use std::ffi::c_void;
// use std::os::windows::ffi::OsStringExt;
// use std::ptr::*;
// use winapi::shared::{dxgi::*, dxgiformat::*, dxgitype::*, winerror::FAILED};
// use winapi::um::{d3d11::*, d3dcommon::*, winbase::*, winnt::HRESULT};
// use winapi::Interface;
// use wio::com::ComPtr;

// // https://github.com/retep998/winapi-rs/issues/80
// fn get_hr_error_message(hr: HRESULT) -> String {
//     let mut buf = [0; 2048];
//     unsafe {
//         if FormatMessageW(
//             FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
//             null_mut(),
//             hr as _,
//             0,
//             buf.as_mut_ptr(),
//             buf.len() as _,
//             null_mut(),
//         ) != 0
//         {
//             std::ffi::OsString::from_wide(&buf).into_string().unwrap()
//         } else {
//             String::from("Unknown D3D error")
//         }
//     }
// }

// macro_rules! s_ok_or_panic {
//     ($($d3d_call:ident).+($($params:tt)*)) => { unsafe {
//         let hr = $($d3d_call).+($($params)*);
//         if FAILED(hr) {
//             log_panic!(format!("{} HR=({}): {}()", get_hr_error_message(hr), hr, stringify!($($d3d_call).+)));
//         }
//     }};
// }

// macro_rules! addr_of {
//     ($e:expr) => {
//         &mut $e as *mut _ as _
//     };
// }

// pub struct Direct3DObject {
//     device: ComPtr<ID3D11Device>,
//     immediate_context: ComPtr<ID3D11DeviceContext>,
// }

// impl GraphicsObject for Direct3DObject {
//     type Texture = ComPtr<ID3D11Texture2D>;
//     type Buffer = ComPtr<ID3D11Buffer>;
//     type ShaderDesc = String;
//     type CommandsObject = ComPtr<ID3D11CommandList>;

//     fn new(adapter_index: u32) -> Self {
//         let mut factory_ptr: *mut IDXGIFactory1 = null_mut();
//         s_ok_or_panic!(CreateDXGIFactory1(
//             &IDXGIFactory1::uuidof(),
//             addr_of!(factory_ptr)
//         ));
//         let factory = unsafe { ComPtr::from_raw(factory_ptr) };

//         let mut adapter_ptr: *mut IDXGIAdapter = null_mut();
//         s_ok_or_panic!(factory.EnumAdapters(adapter_index, addr_of!(adapter_ptr)));
//         let adapter = unsafe { ComPtr::from_raw(adapter_ptr) };

//         let mut feature_level = 0;
//         let mut device_ptr = null_mut();
//         let mut context_ptr = null_mut();
//         s_ok_or_panic!(D3D11CreateDevice(
//             adapter.as_raw(),
//             D3D_DRIVER_TYPE_UNKNOWN,
//             null_mut(),
//             0,
//             null_mut(),
//             0,
//             D3D11_SDK_VERSION,
//             addr_of!(device_ptr),
//             addr_of!(feature_level),
//             addr_of!(context_ptr),
//         ));
//         if feature_level < D3D_FEATURE_LEVEL_11_0 {
//             log_panic!("D3D11 level hardware required!");
//         }

//         unsafe {
//             Direct3DObject {
//                 device: ComPtr::from_raw(device_ptr),
//                 immediate_context: ComPtr::from_raw(context_ptr),
//             }
//         }
//     }

//     fn device_ptr(&self) -> NonNull<c_void> {
//         NonNull::new(self.device.as_raw()).unwrap().cast()
//     }

//     fn create_texture(
//         &self,
//         width: u32,
//         height: u32,
//         format: TextureFormat,
//     ) -> ComPtr<ID3D11Texture2D> {
//         let desc = D3D11_TEXTURE2D_DESC {
//             Width: width,
//             Height: height,
//             Format: match format {
//                 TextureFormat::RGBA8 => DXGI_FORMAT_R8G8B8A8_UNORM,
//                 TextureFormat::SRGBA8 => DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
//                 TextureFormat::BGRA8 => DXGI_FORMAT_B8G8R8A8_UNORM,
//             },

//             MipLevels: 1,
//             ArraySize: 1,
//             SampleDesc: DXGI_SAMPLE_DESC {
//                 Count: 1,
//                 Quality: 0,
//             },
//             Usage: D3D11_USAGE_DEFAULT,
//             BindFlags: D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE,
//             CPUAccessFlags: 0,
//             MiscFlags: 0,
//         };

//         let mut texture_ptr = null_mut();
//         s_ok_or_panic!(self
//             .device
//             .CreateTexture2D(&desc, null_mut(), addr_of!(texture_ptr)));
//         unsafe { ComPtr::from_raw(texture_ptr) }
//     }

//     fn texture_ptr(texture: &Self::Texture) -> NonNull<c_void> {
//         NonNull::new(texture.as_raw()).unwrap().cast()
//     }

//     fn texture_from_handle(&self, handle: u64) -> ComPtr<ID3D11Texture2D> {
//         let mut texture_ptr: *mut ID3D11Texture2D = null_mut();
//         s_ok_or_panic!(self.device.OpenSharedResource(
//             handle as _,
//             &ID3D11Texture2D::uuidof(),
//             addr_of!(texture_ptr)
//         ));
//         unsafe { ComPtr::from_raw(texture_ptr) }
//     }

//     fn wait_for_texture_signal(texture: &ComPtr<ID3D11Texture2D>) {
//         let mut texture_mutex_ptr: *mut IDXGIKeyedMutex = null_mut();
//         s_ok_or_panic!(
//             texture.QueryInterface(&IDXGIKeyedMutex::uuidof(), addr_of!(texture_mutex_ptr))
//         );
//         let timeout_ms = 10;
//         let texture_mutex = unsafe { ComPtr::from_raw(texture_mutex_ptr) };
//         s_ok_or_panic!(texture_mutex.AcquireSync(0, timeout_ms));

//         // Release immediately the lock.
//         s_ok_or_panic!(texture_mutex.ReleaseSync(0));
//     }

//     fn create_buffer(&self, data: &[u8], usage: BufferUsage) -> ComPtr<ID3D11Buffer> {
//         let desc: D3D11_BUFFER_DESC = D3D11_BUFFER_DESC {
//             Usage: match usage {
//                 BufferUsage::Default => D3D11_USAGE_DEFAULT,
//                 BufferUsage::Immutable => D3D11_USAGE_IMMUTABLE,
//             },
//             ByteWidth: data.len() as _,
//             BindFlags: D3D11_BIND_CONSTANT_BUFFER,
//             StructureByteStride: 0,
//             CPUAccessFlags: 0,
//             MiscFlags: 0,
//         };
//         let subresource = D3D11_SUBRESOURCE_DATA {
//             pSysMem: &data as *const _ as _,
//             SysMemPitch: 0,
//             SysMemSlicePitch: 0,
//         };

//         let mut buffer_ptr = null_mut();
//         s_ok_or_panic!(self
//             .device
//             .CreateBuffer(&desc, &subresource, addr_of!(buffer_ptr)));
//         unsafe { ComPtr::from_raw(buffer_ptr) }
//     }

//     fn create_commands_object(
//         &self,
//         operations: Vec<GraphicsOperation<ComPtr<ID3D11Texture2D>, ComPtr<ID3D11Buffer>, String>>,
//     ) -> ComPtr<ID3D11CommandList> {
//         std::unimplemented!();
//     }

//     fn render(&self, commands_object: &ComPtr<ID3D11CommandList>) {
//         unsafe {
//             self.immediate_context
//                 .ExecuteCommandList(commands_object.as_raw(), true as _);

//             // todo: check if needed
//             self.immediate_context.Flush();
//         };
//     }
// }
