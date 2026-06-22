//! WebGPU rendering: one canvas, three stacked viewports.
//! - Bottom & middle regions: cell grids uploaded as R8 textures, drawn as
//!   full-viewport quads with per-region colour mapping.
//! - Top region: the 3D automaton as instanced, lit cubes with an orbiting
//!   perspective camera.
use crate::sim::ca3d::{X3, Y3, Z3};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

const QUAD_SHADER: &str = r#"
struct RegionU {
    fg: vec4f,
    bg: vec4f,
    trail: vec4f,
    misc: vec4f,
}
@group(0) @binding(0) var<uniform> u: RegionU;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VSOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VSOut {
    var p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
    let xy = p[vi];
    var out: VSOut;
    out.pos = vec4f(xy, 0.0, 1.0);
    out.uv = vec2f(xy.x * 0.5 + 0.5, 1.0 - (xy.y * 0.5 + 0.5));
    return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4f {
    let v = textureSample(tex, samp, in.uv).r;
    var col: vec3f;
    if v > 0.97 {
        col = u.fg.rgb;
    } else {
        col = mix(u.bg.rgb, u.trail.rgb, v * v);
    }
    // subtle vertical depth cue, brighter towards the bottom of each region
    col *= 0.9 + 0.1 * in.uv.y;
    return vec4f(col, 1.0);
}
"#;

const CUBE_SHADER: &str = r#"
struct U3 {
    view_proj: mat4x4f,
    light: vec4f,   // xyz = direction, w = ambient
    grid: vec4f,    // X, Y, Z, states
}
@group(0) @binding(0) var<uniform> u: U3;

struct VSIn {
    @location(0) pos: vec3f,
    @location(1) normal: vec3f,
    @location(2) ipos: vec3f,
    @location(3) istate: f32,
}
struct VSOut {
    @builtin(position) clip: vec4f,
    @location(0) col: vec3f,
}

@vertex
fn vs_main(in: VSIn) -> VSOut {
    let world = in.ipos + vec3f(0.5) + in.pos * 0.47;
    var out: VSOut;
    out.clip = u.view_proj * vec4f(world, 1.0);

    let states = max(u.grid.w, 2.0);
    var base: vec3f;
    if in.istate < 1.5 {
        base = vec3f(1.0, 0.86, 0.5); // alive: warm glow
    } else {
        let t = clamp((in.istate - 1.0) / (states - 1.0), 0.0, 1.0);
        base = mix(vec3f(0.93, 0.35, 0.3), vec3f(0.3, 0.12, 0.45), t);
    }
    // cool tint with height
    let hf = in.ipos.y / max(u.grid.y, 1.0);
    base = mix(base, base * vec3f(0.55, 0.8, 1.45), hf * 0.45);

    let ndl = max(dot(in.normal, -normalize(u.light.xyz)), 0.0);
    out.col = base * (u.light.w + (1.0 - u.light.w) * ndl);
    return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4f {
    return vec4f(in.col, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RegionUniform {
    fg: [f32; 4],
    bg: [f32; 4],
    trail: [f32; 4],
    misc: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CubeUniform {
    view_proj: [[f32; 4]; 4],
    light: [f32; 4],
    grid: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CubeVertex {
    pos: [f32; 3],
    normal: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct CubeInstance {
    pub pos: [f32; 3],
    pub state: f32,
}

/// Static orbit camera for the 3D region. The target sits at the centre of
/// the X/Z footprint at height `target_y`; the eye orbits it on a sphere.
/// `azimuth_deg` 0 looks along +X; `elevation_deg` lifts the eye above the
/// horizontal plane.
#[derive(Clone, Copy)]
pub struct Camera {
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub distance: f32,
    pub target_y: f32,
    pub fov_deg: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            azimuth_deg: 90.0,
            elevation_deg: 18.0, // 0.0
            distance: 206.0, // 7.0
            target_y: 40.0, // 24.0
            fov_deg: 20.0, // 69.0
        }
    }
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const CLEAR: wgpu::Color = wgpu::Color { r: 0.012, g: 0.016, b: 0.034, a: 1.0 };

fn cube_mesh() -> Vec<CubeVertex> {
    // 6 faces * 2 triangles; unit cube centred at origin, side 1
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        // (normal, u-axis, v-axis)
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ];
    let mut verts = Vec::with_capacity(36);
    for (n, u, v) in faces {
        let n3 = Vec3::from(n);
        let u3 = Vec3::from(u);
        let v3 = Vec3::from(v);
        let c = n3 * 0.5;
        let quad = [
            c - u3 * 0.5 - v3 * 0.5,
            c + u3 * 0.5 - v3 * 0.5,
            c + u3 * 0.5 + v3 * 0.5,
            c - u3 * 0.5 - v3 * 0.5,
            c + u3 * 0.5 + v3 * 0.5,
            c - u3 * 0.5 + v3 * 0.5,
        ];
        for p in quad {
            verts.push(CubeVertex { pos: p.into(), normal: n });
        }
    }
    verts
}

struct RegionTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,

    quad_pipeline: wgpu::RenderPipeline,
    quad_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    rule30_uniform: wgpu::Buffer,
    gol_uniform: wgpu::Buffer,
    rule30_tex: Option<RegionTexture>,
    gol_tex: Option<RegionTexture>,

    cube_pipeline: wgpu::RenderPipeline,
    cube_uniform: wgpu::Buffer,
    cube_bind_group: wgpu::BindGroup,
    cube_vbuf: wgpu::Buffer,
    instance_buf: wgpu::Buffer,
    instance_count: u32,

    upload_scratch: Vec<u8>,
}

impl Renderer {
    pub async fn new(canvas: web_sys::HtmlCanvasElement) -> Result<Renderer, String> {
        let width = canvas.width().max(8);
        let height = canvas.height().max(8);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| format!("create_surface: {e}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("request_adapter: {e}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| format!("request_device: {e}"))?;

        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or("surface not supported by adapter")?;
        config.present_mode = wgpu::PresentMode::Fifo;
        surface.configure(&device, &config);

        let depth_view = create_depth(&device, width, height);

        // --- 2D region resources ---
        let quad_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad"),
            source: wgpu::ShaderSource::Wgsl(QUAD_SHADER.into()),
        });
        let quad_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("quad bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let quad_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad pl"),
            bind_group_layouts: &[&quad_bgl],
            push_constant_ranges: &[],
        });
        let quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad pipeline"),
            layout: Some(&quad_pl),
            vertex: wgpu::VertexState {
                module: &quad_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &quad_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(config.format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let mk_uniform = |label: &str, u: RegionUniform| {
            use wgpu::util::DeviceExt;
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::bytes_of(&u),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        };
        // Rule 30: amber cells on near-black
        let rule30_uniform = mk_uniform(
            "rule30 u",
            RegionUniform {
                fg: [1.0, 0.72, 0.28, 1.0],
                bg: [0.016, 0.014, 0.03, 1.0],
                trail: [0.1, 0.05, 0.03, 1.0],
                misc: [0.0; 4],
            },
        );
        // Life: minty cells, teal trails
        let gol_uniform = mk_uniform(
            "gol u",
            RegionUniform {
                fg: [0.45, 1.0, 0.72, 1.0],
                // fg: [0.35, 0.55, 1.0, 1.0],
                bg: [0.012, 0.022, 0.036, 1.0],
                trail: [0.05, 0.28, 0.30, 1.0],
                // trail: [0.23, 0.31, 0.48, 1.0],
                misc: [1.0, 0.0, 0.0, 0.0],
            },
        );

        // --- 3D region resources ---
        let cube_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cube"),
            source: wgpu::ShaderSource::Wgsl(CUBE_SHADER.into()),
        });
        let cube_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cube bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let cube_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cube pl"),
            bind_group_layouts: &[&cube_bgl],
            push_constant_ranges: &[],
        });
        let cube_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cube pipeline"),
            layout: Some(&cube_pl),
            vertex: wgpu::VertexState {
                module: &cube_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<CubeVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<CubeInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![2 => Float32x3, 3 => Float32],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &cube_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(config.format.into())],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let cube_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cube u"),
            size: std::mem::size_of::<CubeUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cube_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cube bg"),
            layout: &cube_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: cube_uniform.as_entire_binding(),
            }],
        });
        let mesh = cube_mesh();
        let cube_vbuf = {
            use wgpu::util::DeviceExt;
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cube verts"),
                contents: bytemuck::cast_slice(&mesh),
                usage: wgpu::BufferUsages::VERTEX,
            })
        };
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cube instances"),
            size: (X3 * Y3 * Z3 * std::mem::size_of::<CubeInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Renderer {
            surface,
            device,
            queue,
            config,
            depth_view,
            quad_pipeline,
            quad_bgl,
            sampler,
            rule30_uniform,
            gol_uniform,
            rule30_tex: None,
            gol_tex: None,
            cube_pipeline,
            cube_uniform,
            cube_bind_group,
            cube_vbuf,
            instance_buf,
            instance_count: 0,
            upload_scratch: Vec::new(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(8);
        self.config.height = height.max(8);
        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth(&self.device, self.config.width, self.config.height);
    }

    fn region_texture(&self, label: &str, width: u32, height: u32, uniform: &wgpu::Buffer) -> RegionTexture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &self.quad_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        RegionTexture { texture, bind_group, width, height }
    }

    /// Upload the Rule 30 region as grayscale bytes (row-major, top first).
    pub fn upload_rule30(&mut self, width: u32, height: u32, data: &[u8]) {
        if self.rule30_tex.as_ref().map(|t| (t.width, t.height)) != Some((width, height)) {
            self.rule30_tex = Some(self.region_texture("rule30 tex", width, height, &self.rule30_uniform));
        }
        let tex = self.rule30_tex.as_ref().unwrap().texture.clone();
        Self::write_region(&self.queue, &mut self.upload_scratch, &tex, width, height, data);
    }

    pub fn upload_gol(&mut self, width: u32, height: u32, data: &[u8]) {
        if self.gol_tex.as_ref().map(|t| (t.width, t.height)) != Some((width, height)) {
            self.gol_tex = Some(self.region_texture("gol tex", width, height, &self.gol_uniform));
        }
        let tex = self.gol_tex.as_ref().unwrap().texture.clone();
        Self::write_region(&self.queue, &mut self.upload_scratch, &tex, width, height, data);
    }

    fn write_region(queue: &wgpu::Queue, scratch: &mut Vec<u8>, texture: &wgpu::Texture, width: u32, height: u32, data: &[u8]) {
        let padded = (width as usize).div_ceil(256) * 256;
        scratch.clear();
        scratch.resize(padded * height as usize, 0);
        for y in 0..height as usize {
            let src = &data[y * width as usize..(y + 1) * width as usize];
            scratch[y * padded..y * padded + width as usize].copy_from_slice(src);
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            scratch,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded as u32),
                rows_per_image: None,
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
    }

    pub fn upload_instances(&mut self, instances: &[CubeInstance]) {
        self.instance_count = instances.len() as u32;
        if !instances.is_empty() {
            self.queue.write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(instances));
        }
    }

    /// `b1`, `b2`: region boundaries in device pixels measured from the top
    /// (0 < b1 < b2 < height). `time` drives the orbiting camera.
    pub fn render(&mut self, b1: f32, b2: f32, cam: Camera, states: u8) {
        let (w, h) = (self.config.width as f32, self.config.height as f32);
        let b1 = b1.clamp(8.0, h - 16.0);
        let b2 = b2.clamp(b1 + 8.0, h - 8.0);

        // Static orbit camera for the 3D region (see `Camera`)
        let aspect = w / b1.max(1.0);
        let center = Vec3::new(X3 as f32 / 2.0, cam.target_y, Z3 as f32 / 2.0);
        let az = cam.azimuth_deg.to_radians();
        let el = cam.elevation_deg.to_radians();
        let eye = center
            + cam.distance
                * Vec3::new(el.cos() * az.cos(), el.sin(), el.cos() * az.sin());
        let view = Mat4::look_at_rh(eye, center, Vec3::Y);
        let proj = Mat4::perspective_rh(cam.fov_deg.to_radians(), aspect.max(0.05), 1.0, 600.0);
        let light = (center - eye + Vec3::new(0.0, -(Y3 as f32) * 1.5, 0.0)).normalize();
        let u3 = CubeUniform {
            view_proj: (proj * view).to_cols_array_2d(),
            light: [light.x, light.y, light.z, 0.35],
            grid: [X3 as f32, Y3 as f32, Z3 as f32, states as f32],
        };
        self.queue.write_buffer(&self.cube_uniform, 0, bytemuck::bytes_of(&u3));

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(_) => return,
                }
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.record_pass(&mut encoder, &view, b1, b2, w, h);
        self.queue.submit([encoder.finish()]);
        frame.present();
    }

    fn record_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        b1: f32,
        b2: f32,
        w: f32,
        h: f32,
    ) {
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Top region: 3D cubes
            if b1 >= 9.0 && self.instance_count > 0 {
                pass.set_viewport(0.0, 0.0, w, b1, 0.0, 1.0);
                pass.set_pipeline(&self.cube_pipeline);
                pass.set_bind_group(0, &self.cube_bind_group, &[]);
                pass.set_vertex_buffer(0, self.cube_vbuf.slice(..));
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                pass.draw(0..36, 0..self.instance_count);
            }

            // Middle region: Game of Life
            if let Some(t) = &self.gol_tex {
                pass.set_viewport(0.0, b1, w, b2 - b1, 0.0, 1.0);
                pass.set_pipeline(&self.quad_pipeline);
                pass.set_bind_group(0, &t.bind_group, &[]);
                pass.draw(0..3, 0..1);
            }

            // Bottom region: Rule 30
            if let Some(t) = &self.rule30_tex {
                pass.set_viewport(0.0, b2, w, h - b2, 0.0, 1.0);
                pass.set_pipeline(&self.quad_pipeline);
                pass.set_bind_group(0, &t.bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
        }
    }
}

fn create_depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}
