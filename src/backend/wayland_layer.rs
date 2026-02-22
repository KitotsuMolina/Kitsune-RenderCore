use crate::backend::LayerBackend;
use crate::frame_source::{FrameSource, VideoOptions};
use crate::monitor::{LayerRole, MonitorInfo, MonitorSurfaceSpec};
use crate::video_map::{
    map_file_path_from_env, merge_maps, parse_video_map_env, parse_video_map_file,
};
use bytemuck::{Pod, Zeroable};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::time::{Duration, Instant, SystemTime};
use wayland_client::protocol::{
    wl_callback, wl_compositor, wl_output, wl_registry, wl_surface, wl_surface::WlSurface,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, delegate_noop};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, ZwlrLayerSurfaceV1},
};

#[derive(Default)]
pub struct WaylandLayerBackend {
    bootstrapped: bool,
    connection: Option<Connection>,
    event_queue: Option<EventQueue<WaylandLayerState>>,
    wgpu_shared: Option<WgpuShared>,
    frame_index: u64,
    state: WaylandLayerState,
}

impl LayerBackend for WaylandLayerBackend {
    fn name(&self) -> &'static str {
        "wayland-layer"
    }

    fn bootstrap(&mut self) -> Result<(), String> {
        let connection = Connection::connect_to_env()
            .map_err(|err| format!("failed to connect wayland display: {err}"))?;
        let mut event_queue = connection.new_event_queue();
        let qh = event_queue.handle();

        connection.display().get_registry(&qh, ());
        event_queue
            .roundtrip(&mut self.state)
            .map_err(|err| format!("wayland roundtrip failed: {err}"))?;

        if self.state.compositor.is_none() {
            return Err("wl_compositor is not available".to_string());
        }
        if self.state.layer_shell.is_none() {
            return Err(
                "zwlr_layer_shell_v1 is not available (compositor may not support layer-shell)"
                    .to_string(),
            );
        }
        if self.state.outputs.is_empty() {
            return Err("no wl_output globals discovered".to_string());
        }

        self.state.create_layer_surfaces(&qh)?;
        event_queue
            .roundtrip(&mut self.state)
            .map_err(|err| format!("wayland post-surface roundtrip failed: {err}"))?;

        let wgpu_shared =
            init_wgpu_shared(&connection, &self.state.outputs, &self.state.layer_surfaces)?;

        self.bootstrapped = true;
        self.connection = Some(connection);
        self.event_queue = Some(event_queue);
        self.wgpu_shared = Some(wgpu_shared);
        self.frame_index = 0;

        println!(
            "[backend:{}] wayland connected outputs={} layer-surfaces={}",
            self.name(),
            self.state.outputs.len(),
            self.state.layer_surfaces.len()
        );
        Ok(())
    }

    fn discover_monitors(&mut self) -> Result<Vec<MonitorInfo>, String> {
        if !self.bootstrapped {
            return Err("backend not bootstrapped".to_string());
        }

        let monitors = self
            .state
            .outputs
            .values()
            .map(|out| MonitorInfo {
                name: out
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("wl-output-{}", out.global_name)),
                width: out.width.unwrap_or(1920),
                height: out.height.unwrap_or(1080),
                refresh_hz: out.refresh_hz.unwrap_or(60),
            })
            .collect::<Vec<_>>();

        if monitors.is_empty() {
            return Err("no outputs tracked in wayland state".to_string());
        }
        Ok(monitors)
    }

    fn build_surfaces(
        &mut self,
        monitors: &[MonitorInfo],
    ) -> Result<Vec<MonitorSurfaceSpec>, String> {
        if !self.bootstrapped {
            return Err("backend not bootstrapped".to_string());
        }

        Ok(monitors
            .iter()
            .cloned()
            .map(|monitor| MonitorSurfaceSpec {
                monitor,
                layer: LayerRole::Background,
            })
            .collect())
    }

    fn render_frame(&mut self, surfaces: &[MonitorSurfaceSpec]) -> Result<(), String> {
        if !self.bootstrapped {
            return Err("backend not bootstrapped".to_string());
        }

        let queue = self
            .event_queue
            .as_mut()
            .ok_or_else(|| "missing wayland event queue".to_string())?;
        queue
            .dispatch_pending(&mut self.state)
            .map_err(|err| format!("wayland dispatch_pending failed: {err}"))?;
        let qh = queue.handle();
        if self.state.ready_output_ids().is_empty() {
            queue
                .blocking_dispatch(&mut self.state)
                .map_err(|err| format!("wayland blocking_dispatch failed: {err}"))?;
        }

        let configured = self
            .state
            .layer_surfaces
            .iter()
            .filter(|slot| slot.configured)
            .count();
        let ready = self
            .state
            .layer_surfaces
            .iter()
            .filter(|slot| slot.configured && slot.needs_redraw)
            .count();
        let pending_callbacks = self
            .state
            .layer_surfaces
            .iter()
            .filter(|slot| slot.frame_callback_pending)
            .count();
        let outputs = self
            .state
            .layer_surfaces
            .iter()
            .map(|slot| {
                format!(
                    "{}:{}",
                    slot.output_global_name,
                    slot.layer_surface.id().protocol_id()
                )
            })
            .collect::<Vec<_>>()
            .join(",");

        let ready_outputs = self.state.ready_output_ids();
        if let Some(shared) = self.wgpu_shared.as_mut() {
            shared.render_textured(self.frame_index, &self.state.outputs, &ready_outputs)?;
        }
        if !ready_outputs.is_empty() {
            self.state
                .mark_presented_and_request_frames(&qh, &ready_outputs);
            if let Some(conn) = self.connection.as_ref() {
                conn.flush()
                    .map_err(|err| format!("wayland connection flush failed: {err}"))?;
            }
            self.frame_index = self.frame_index.wrapping_add(1);
        }

        if self.frame_index % 120 == 0 {
            println!(
                "[backend:{}] render frame surfaces={} live-layer-surfaces={} configured={} ready={} pending_callbacks={} uploaded_video_frames={} outputs=[{}]",
                self.name(),
                surfaces.len(),
                self.state.layer_surfaces.len(),
                configured,
                ready,
                pending_callbacks,
                shared_uploaded_frames(self),
                outputs
            );
        }
        Ok(())
    }
}

fn shared_uploaded_frames(backend: &WaylandLayerBackend) -> u64 {
    backend
        .wgpu_shared
        .as_ref()
        .map(|s| s.uploaded_video_frames)
        .unwrap_or(0)
}

#[derive(Default)]
struct WaylandLayerState {
    compositor: Option<wl_compositor::WlCompositor>,
    layer_shell: Option<ZwlrLayerShellV1>,
    outputs: BTreeMap<u32, OutputSlot>,
    layer_surfaces: Vec<LayerSurfaceSlot>,
}

impl WaylandLayerState {
    fn create_layer_surfaces(&mut self, qh: &QueueHandle<Self>) -> Result<(), String> {
        if !self.layer_surfaces.is_empty() {
            return Ok(());
        }

        let compositor = self
            .compositor
            .as_ref()
            .ok_or_else(|| "missing wl_compositor".to_string())?
            .clone();
        let layer_shell = self
            .layer_shell
            .as_ref()
            .ok_or_else(|| "missing zwlr_layer_shell_v1".to_string())?
            .clone();

        for output in self.outputs.values() {
            let surface = compositor.create_surface(qh, ());
            let layer_surface = layer_shell.get_layer_surface(
                &surface,
                Some(&output.output),
                zwlr_layer_shell_v1::Layer::Background,
                "kitsune-rendercore".to_string(),
                qh,
                self.layer_surfaces.len() as u32,
            );

            layer_surface.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
            layer_surface.set_exclusive_zone(-1);
            layer_surface.set_size(0, 0);
            surface.commit();

            self.layer_surfaces.push(LayerSurfaceSlot {
                surface,
                layer_surface,
                output_global_name: output.global_name,
                configured: false,
                needs_redraw: false,
                frame_callback_pending: false,
                frame_callback: None,
            });
        }

        Ok(())
    }

    fn ready_output_ids(&self) -> Vec<u32> {
        self.layer_surfaces
            .iter()
            .filter(|slot| slot.configured && slot.needs_redraw)
            .map(|slot| slot.output_global_name)
            .collect()
    }

    fn mark_presented_and_request_frames(&mut self, qh: &QueueHandle<Self>, outputs: &[u32]) {
        for (index, slot) in self.layer_surfaces.iter_mut().enumerate() {
            if !outputs.iter().any(|id| *id == slot.output_global_name) {
                continue;
            }
            slot.needs_redraw = false;
            if !slot.frame_callback_pending {
                let cb = slot.surface.frame(qh, index as u32);
                slot.frame_callback = Some(cb);
                slot.frame_callback_pending = true;
                slot.surface.commit();
            }
        }
    }
}

struct OutputSlot {
    global_name: u32,
    output: wl_output::WlOutput,
    name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    refresh_hz: Option<u32>,
}

struct LayerSurfaceSlot {
    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    output_global_name: u32,
    configured: bool,
    needs_redraw: bool,
    frame_callback_pending: bool,
    frame_callback: Option<wl_callback::WlCallback>,
}

struct WgpuShared {
    _instance: wgpu::Instance,
    _adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_surfaces: Vec<RenderSurface>,
    program: RenderProgram,
    started_at: Instant,
    video_streams: BTreeMap<u32, VideoStream>,
    video_map_state: VideoMapState,
    uploaded_video_frames: u64,
}

struct RenderSurface {
    output_global_name: u32,
    width: u32,
    height: u32,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

struct RenderProgram {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
}

struct VideoStream {
    bind_group: wgpu::BindGroup,
    source_texture: wgpu::Texture,
    source_width: u32,
    source_height: u32,
    frame_source: FrameSource,
    frame_pixels: Vec<u8>,
    current_video: Option<String>,
}

struct VideoMapState {
    map_file: PathBuf,
    default_video: Option<String>,
    env_map: BTreeMap<String, String>,
    merged_map: BTreeMap<String, String>,
    last_mtime: Option<SystemTime>,
    last_reload_check: Instant,
    reload_interval: Duration,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameUniform {
    time_sec: f32,
    aspect: f32,
    _pad: [f32; 2],
}

const FRAME_SHADER_WGSL: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct FrameUniform {
    time_sec: f32,
    aspect: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: FrameUniform;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0)
    );
    let p = pos[vid];
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = 0.5 * (p + vec2<f32>(1.0, 1.0));
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let base_uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
    let wave = vec2<f32>(
        sin(uniforms.time_sec * 0.45 + base_uv.y * 8.0) * 0.005,
        cos(uniforms.time_sec * 0.40 + base_uv.x * 7.0) * 0.005 * uniforms.aspect
    );
    let uv = fract(base_uv + wave);
    let col = textureSample(src_tex, src_sampler, uv).rgb;
    return vec4<f32>(col, 1.0);
}
"#;

fn init_wgpu_shared(
    connection: &Connection,
    outputs: &BTreeMap<u32, OutputSlot>,
    layer_surfaces: &[LayerSurfaceSlot],
) -> Result<WgpuShared, String> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .ok_or_else(|| "wgpu request_adapter returned None".to_string())?;
    let adapter_limits = adapter.limits();

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("kitsune-rendercore-device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter_limits.clone(),
            memory_hints: wgpu::MemoryHints::Performance,
        },
        None,
    ))
    .map_err(|err| format!("wgpu request_device failed: {err}"))?;

    let display_ptr = NonNull::new(connection.backend().display_ptr() as *mut _)
        .ok_or_else(|| "wayland display pointer is null".to_string())?;
    let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display_ptr));

    let mut render_surfaces = Vec::new();
    for slot in layer_surfaces {
        let Some(out) = outputs.get(&slot.output_global_name) else {
            continue;
        };
        let width = out.width.unwrap_or(1920).max(1);
        let height = out.height.unwrap_or(1080).max(1);
        let window_ptr = NonNull::new(slot.surface.id().as_ptr() as *mut _)
            .ok_or_else(|| "wayland surface pointer is null".to_string())?;
        let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(window_ptr));

        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle,
                    raw_window_handle,
                })
                .map_err(|err| format!("wgpu create_surface_unsafe failed: {err}"))?
        };

        let caps = surface.get_capabilities(&adapter);
        if caps.formats.is_empty() {
            return Err("wgpu surface has no supported formats".to_string());
        }
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };
        let alpha_mode = caps
            .alpha_modes
            .iter()
            .copied()
            .find(|m| *m == wgpu::CompositeAlphaMode::Auto)
            .unwrap_or(caps.alpha_modes[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode,
            alpha_mode,
            view_formats: vec![format],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        render_surfaces.push(RenderSurface {
            output_global_name: slot.output_global_name,
            width,
            height,
            surface,
            config,
        });
    }
    let surface_format = render_surfaces
        .first()
        .map(|s| s.config.format)
        .ok_or_else(|| "no render surfaces created for outputs".to_string())?;
    let program = init_render_program(&device, surface_format)?;
    let source_size = choose_source_resolution(adapter_limits.max_texture_dimension_2d);
    println!(
        "[rendercore] source texture selected={}x{} (max_texture_dimension_2d={})",
        source_size.0, source_size.1, adapter_limits.max_texture_dimension_2d
    );
    let video_options = VideoOptions::from_env();
    let map_file = map_file_path_from_env();
    let env_map = std::env::var("KRC_VIDEO_MAP")
        .ok()
        .map(|v| parse_video_map_env(&v))
        .unwrap_or_default();
    let file_map = parse_video_map_file(&map_file);
    let merged_map = merge_maps(env_map.clone(), file_map);
    let last_mtime = std::fs::metadata(&map_file)
        .ok()
        .and_then(|m| m.modified().ok());
    let video_map_state = VideoMapState {
        map_file,
        default_video: std::env::var("KRC_VIDEO_DEFAULT")
            .ok()
            .or_else(|| std::env::var("KRC_VIDEO").ok()),
        env_map,
        merged_map,
        last_mtime,
        last_reload_check: Instant::now(),
        reload_interval: Duration::from_millis(1000),
    };
    let mut video_streams = BTreeMap::new();
    for (output_id, out) in outputs {
        let output_name = out
            .name
            .clone()
            .unwrap_or_else(|| format!("wl-output-{output_id}"));
        let selected_video = video_map_state
            .merged_map
            .get(&output_name)
            .cloned()
            .or_else(|| video_map_state.default_video.clone());
        let stream = init_video_stream(
            &device,
            &queue,
            &program,
            source_size,
            selected_video,
            video_options,
            output_id,
            &output_name,
        )?;
        video_streams.insert(*output_id, stream);
    }

    Ok(WgpuShared {
        _instance: instance,
        _adapter: adapter,
        device,
        queue,
        render_surfaces,
        program,
        started_at: Instant::now(),
        video_streams,
        video_map_state,
        uploaded_video_frames: 0,
    })
}

impl WgpuShared {
    fn maybe_reload_video_map(&mut self, outputs: &BTreeMap<u32, OutputSlot>) {
        if self.video_map_state.last_reload_check.elapsed() < self.video_map_state.reload_interval {
            return;
        }
        self.video_map_state.last_reload_check = Instant::now();

        let current_mtime = std::fs::metadata(&self.video_map_state.map_file)
            .ok()
            .and_then(|m| m.modified().ok());
        if current_mtime == self.video_map_state.last_mtime {
            return;
        }
        self.video_map_state.last_mtime = current_mtime;

        let file_map = parse_video_map_file(&self.video_map_state.map_file);
        self.video_map_state.merged_map =
            merge_maps(self.video_map_state.env_map.clone(), file_map);

        for (output_id, out) in outputs {
            let output_name = out
                .name
                .clone()
                .unwrap_or_else(|| format!("wl-output-{output_id}"));
            let desired = self
                .video_map_state
                .merged_map
                .get(&output_name)
                .cloned()
                .or_else(|| self.video_map_state.default_video.clone());
            let Some(stream) = self.video_streams.get_mut(output_id) else {
                continue;
            };
            if stream.current_video == desired {
                continue;
            }
            stream.current_video = desired.clone();
            stream.frame_source = if let Some(path) = desired {
                println!(
                    "[rendercore] reloaded monitor={} (id={}) video={}",
                    output_name, output_id, path
                );
                FrameSource::from_video_path(
                    path,
                    stream.source_width,
                    stream.source_height,
                    VideoOptions::from_env(),
                )
            } else {
                println!(
                    "[rendercore] reloaded monitor={} (id={}) video=<none> (procedural fallback)",
                    output_name, output_id
                );
                FrameSource::None
            };
        }
    }

    fn render_textured(
        &mut self,
        frame_index: u64,
        outputs: &BTreeMap<u32, OutputSlot>,
        ready_outputs: &[u32],
    ) -> Result<(), String> {
        self.maybe_reload_video_map(outputs);
        if ready_outputs.is_empty() {
            return Ok(());
        }

        for rs in &mut self.render_surfaces {
            let Some(out) = outputs.get(&rs.output_global_name) else {
                continue;
            };
            let width = out.width.unwrap_or(1920).max(1);
            let height = out.height.unwrap_or(1080).max(1);
            if width != rs.width || height != rs.height {
                rs.width = width;
                rs.height = height;
                rs.config.width = width;
                rs.config.height = height;
                rs.surface.configure(&self.device, &rs.config);
            }
        }

        let mut acquired = Vec::new();
        let should_render = |output_id: u32| {
            ready_outputs
                .iter()
                .any(|candidate| *candidate == output_id)
        };
        for (idx, rs) in self.render_surfaces.iter_mut().enumerate() {
            if !should_render(rs.output_global_name) {
                continue;
            }
            let frame = match rs.surface.get_current_texture() {
                Ok(frame) => frame,
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    rs.surface.configure(&self.device, &rs.config);
                    rs.surface.get_current_texture().map_err(|err| {
                        format!("wgpu reacquire surface texture failed on output {idx}: {err}")
                    })?
                }
                Err(wgpu::SurfaceError::Timeout) => {
                    continue;
                }
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    return Err("wgpu surface out of memory".to_string());
                }
                Err(wgpu::SurfaceError::Other) => {
                    continue;
                }
            };
            acquired.push((rs.output_global_name, frame));
        }

        if acquired.is_empty() {
            return Ok(());
        }

        for output_id in ready_outputs {
            let Some(stream) = self.video_streams.get_mut(output_id) else {
                continue;
            };
            if stream
                .frame_source
                .fill_next_frame(&mut stream.frame_pixels)
            {
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &stream.source_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &stream.frame_pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stream.source_width * 4),
                        rows_per_image: Some(stream.source_height),
                    },
                    wgpu::Extent3d {
                        width: stream.source_width,
                        height: stream.source_height,
                        depth_or_array_layers: 1,
                    },
                );
                self.uploaded_video_frames = self.uploaded_video_frames.wrapping_add(1);
            }
        }

        let elapsed = self.started_at.elapsed().as_secs_f32();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kitsune-rendercore-frame-encoder"),
            });

        for (output_id, frame) in &acquired {
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let aspect =
                (frame.texture.width() as f32 / (frame.texture.height().max(1) as f32)).max(0.0001);
            let uniform = FrameUniform {
                time_sec: elapsed + frame_index as f32 * 0.0001,
                aspect,
                _pad: [0.0; 2],
            };
            self.queue.write_buffer(
                &self.program.uniform_buffer,
                0,
                bytemuck::bytes_of(&uniform),
            );
            let bind_group = self
                .video_streams
                .get(output_id)
                .map(|s| &s.bind_group)
                .ok_or_else(|| format!("missing video stream for output {output_id}"))?;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kitsune-rendercore-textured-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.program.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit([encoder.finish()]);
        for (_, frame) in acquired {
            frame.present();
        }
        Ok(())
    }
}

fn init_render_program(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> Result<RenderProgram, String> {
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("kitsune-rendercore-source-sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("kitsune-rendercore-frame-uniform"),
        size: std::mem::size_of::<FrameUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("kitsune-rendercore-frame-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("kitsune-rendercore-frame-shader"),
        source: wgpu::ShaderSource::Wgsl(FRAME_SHADER_WGSL.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("kitsune-rendercore-frame-pipeline-layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("kitsune-rendercore-frame-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    Ok(RenderProgram {
        pipeline,
        bind_group_layout,
        sampler,
        uniform_buffer,
    })
}

fn init_video_stream(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    program: &RenderProgram,
    source_size: (u32, u32),
    selected_video: Option<String>,
    video_options: VideoOptions,
    output_id: &u32,
    output_name: &str,
) -> Result<VideoStream, String> {
    let (source_width, source_height) = source_size;
    let frame_pixels = procedural_pixels(source_width, source_height);
    let source_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kitsune-rendercore-source-texture"),
        size: wgpu::Extent3d {
            width: source_width,
            height: source_height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &source_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &frame_pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(source_width * 4),
            rows_per_image: Some(source_height),
        },
        wgpu::Extent3d {
            width: source_width,
            height: source_height,
            depth_or_array_layers: 1,
        },
    );
    let texture_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("kitsune-rendercore-frame-bg"),
        layout: &program.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&program.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: program.uniform_buffer.as_entire_binding(),
            },
        ],
    });

    let frame_source = if let Some(path) = selected_video.clone() {
        println!(
            "[rendercore] output={} (id={}) video={}",
            output_name, output_id, path
        );
        FrameSource::from_video_path(path, source_width, source_height, video_options)
    } else {
        println!(
            "[rendercore] output={} (id={}) video=<none> (procedural fallback)",
            output_name, output_id
        );
        FrameSource::None
    };
    let current_video = selected_video;

    Ok(VideoStream {
        bind_group,
        source_texture,
        source_width,
        source_height,
        frame_source,
        frame_pixels,
        current_video,
    })
}

fn procedural_pixels(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let i = ((y * width + x) * 4) as usize;
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            let stripe = (((x / 32) + (y / 32)) % 2) as f32;
            pixels[i] = (30.0 + 150.0 * fx + 40.0 * stripe) as u8;
            pixels[i + 1] = (40.0 + 170.0 * fy) as u8;
            pixels[i + 2] = (80.0 + 100.0 * (1.0 - fx) + 35.0 * stripe) as u8;
            pixels[i + 3] = 255;
        }
    }
    pixels
}

fn choose_source_resolution(max_texture_dimension_2d: u32) -> (u32, u32) {
    let preset = std::env::var("KRC_QUALITY").ok().and_then(|v| {
        let v = v.to_ascii_lowercase();
        match v.as_str() {
            "low" | "720p" => Some((1280u32, 720u32)),
            "medium" | "1080p" => Some((1920u32, 1080u32)),
            "high" | "1440p" => Some((2560u32, 1440u32)),
            "ultra" | "4k" | "2160p" => Some((3840u32, 2160u32)),
            _ => None,
        }
    });

    let mut width = preset.map(|p| p.0).unwrap_or(960);
    let mut height = preset.map(|p| p.1).unwrap_or(540);

    if let Some(w) = std::env::var("KRC_SOURCE_WIDTH")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v > 0)
    {
        width = w;
    }
    if let Some(h) = std::env::var("KRC_SOURCE_HEIGHT")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v > 0)
    {
        height = h;
    }

    if width <= max_texture_dimension_2d && height <= max_texture_dimension_2d {
        return (width, height);
    }

    let scale_w = max_texture_dimension_2d as f64 / width as f64;
    let scale_h = max_texture_dimension_2d as f64 / height as f64;
    let scale = scale_w.min(scale_h).min(1.0);
    let clamped_w = ((width as f64 * scale).floor() as u32).max(1);
    let clamped_h = ((height as f64 * scale).floor() as u32).max(1);
    eprintln!(
        "[rendercore] requested source {}x{} exceeds GPU max {}; clamped to {}x{}",
        width, height, max_texture_dimension_2d, clamped_w, clamped_h
    );
    (clamped_w, clamped_h)
}

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandLayerState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    let v = version.min(6);
                    state.compositor = Some(registry.bind(name, v, qh, ()));
                }
                "zwlr_layer_shell_v1" => {
                    let v = version.min(4);
                    state.layer_shell = Some(registry.bind(name, v, qh, ()));
                }
                "wl_output" => {
                    let v = version.min(4);
                    let output: wl_output::WlOutput = registry.bind(name, v, qh, name);
                    state.outputs.insert(
                        name,
                        OutputSlot {
                            global_name: name,
                            output,
                            name: None,
                            width: None,
                            height: None,
                            refresh_hz: None,
                        },
                    );
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_output::WlOutput, u32> for WaylandLayerState {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        global_name: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(out) = state.outputs.get_mut(global_name) else {
            return;
        };

        match event {
            wl_output::Event::Name { name } => {
                out.name = Some(name);
            }
            wl_output::Event::Mode {
                flags,
                width,
                height,
                refresh,
            } => {
                if let WEnum::Value(bits) = flags {
                    if bits.contains(wl_output::Mode::Current) {
                        out.width = Some(width.max(1) as u32);
                        out.height = Some(height.max(1) as u32);
                        out.refresh_hz = Some(((refresh as f32) / 1000.0).round().max(1.0) as u32);
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, u32> for WaylandLayerState {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        index: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                layer_surface.ack_configure(serial);
                if let Some(slot) = state.layer_surfaces.get_mut(*index as usize) {
                    slot.configured = true;
                    slot.needs_redraw = true;
                    if width > 0 && height > 0 {
                        slot.surface.commit();
                    }
                }
            }
            zwlr_layer_surface_v1::Event::Closed => {
                if let Some(slot) = state.layer_surfaces.get_mut(*index as usize) {
                    slot.configured = false;
                    slot.needs_redraw = false;
                    slot.frame_callback_pending = false;
                    slot.frame_callback = None;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_callback::WlCallback, u32> for WaylandLayerState {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        index: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            if let Some(slot) = state.layer_surfaces.get_mut(*index as usize) {
                slot.frame_callback_pending = false;
                slot.frame_callback = None;
                if slot.configured {
                    slot.needs_redraw = true;
                }
            }
        }
    }
}

delegate_noop!(WaylandLayerState: ignore wl_compositor::WlCompositor);
delegate_noop!(WaylandLayerState: ignore wl_surface::WlSurface);
delegate_noop!(WaylandLayerState: ignore ZwlrLayerShellV1);
