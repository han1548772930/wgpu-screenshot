use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowId},
};
// ===== 配置常量定义区域 =====
// 颜色常量 (RGB 0.0-1.0 范围)
const RED: [f32; 3] = [1.0, 0.0, 0.0]; // 红色
const GREEN: [f32; 3] = [0.0, 1.0, 0.0]; // 绿色
const BLUE: [f32; 3] = [0.0, 0.0, 1.0]; // 蓝色
const CYAN: [f32; 3] = [0.0, 1.0, 1.0]; // 青色
const YELLOW: [f32; 3] = [1.0, 1.0, 0.0]; // 黄色
const MAGENTA: [f32; 3] = [1.0, 0.0, 1.0]; // 紫色
const WHITE: [f32; 3] = [1.0, 1.0, 1.0]; // 白色
const BLACK: [f32; 3] = [0.0, 0.0, 0.0]; // 黑色
const GRAY: [f32; 3] = [0.5, 0.5, 0.5]; // 灰色

// 默认配置常量
const DEFAULT_BORDER_WIDTH: f32 = 1.0; // 默认边框宽度
const DEFAULT_HANDLE_SIZE: f32 = 12.0; // 默认手柄大小
const DEFAULT_HANDLE_BORDER_WIDTH: f32 = 1.0; // 默认手柄边框宽度
const DEFAULT_BORDER_COLOR: [f32; 3] = CYAN; // 默认边框颜色
const DEFAULT_HANDLE_COLOR: [f32; 3] = CYAN; // 默认手柄颜色

// 拖拽配置常量
const MIN_BOX_SIZE: f32 = 20.0; // 最小框大小
const FRAME_LIMIT_DRAG: u128 = 8; // 拖拽时帧率限制 (8ms = 120fps)
const FRAME_LIMIT_IDLE: u128 = 33; // 静止时帧率限制 (33ms = 30fps)

// 测试纹理配置
const TEST_TEXTURE_SIZE: u32 = 512; // 测试纹理大小
const TEST_TEXTURE_COLOR: [u8; 4] = [255, 0, 0, 255]; // 测试纹理颜色(红色)

// Uniform缓冲区对齐常量
const UNIFORM_BUFFER_SIZE: usize = 18; // 
// ===== 常量定义结束 =====

struct State {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: winit::dpi::PhysicalSize<u32>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    uniform_buffer: wgpu::Buffer,

    // 可配置的边框参数
    border_width: f32,
    handle_size: f32,
    handle_border_width: f32,
    border_color: [f32; 3],
    handle_color: [f32; 3],
}

impl State {
    async fn new(window: Arc<Window>) -> State {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();

        let surface = instance.create_surface(window.clone()).unwrap();
        let cap = surface.get_capabilities(&adapter);
        let surface_format = cap.formats[0];

        // 简单的着色器
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        // 绑定组布局
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            label: None,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 16,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x4,
                    }],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 全屏四边形
        let vertices = [
            [-1.0f32, -1.0, 0.0, 1.0],
            [1.0, -1.0, 1.0, 1.0],
            [1.0, 1.0, 1.0, 0.0],
            [-1.0, -1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0, 0.0],
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let size = window.inner_size();
        let box_data = [
            -1.0f32,                     // 0: box_min.x
            -1.0f32,                     // 1: box_min.y
            -1.0f32,                     // 2: box_max.x
            -1.0f32,                     // 3: box_max.y
            size.width as f32,           // 4: screen_size.x
            size.height as f32,          // 5: screen_size.y
            DEFAULT_BORDER_WIDTH,        // 6: border_width
            DEFAULT_HANDLE_SIZE,         // 7: handle_size
            DEFAULT_HANDLE_BORDER_WIDTH, // 8: handle_border_width
            0.0f32,                      // 18: _padding1[0] - 新增
            0.0f32,                      // 18: _padding1[0] - 新增
            0.0f32,                      // 18: _padding1[0] - 新增
            DEFAULT_BORDER_COLOR[0],     // 10: border_color.r
            DEFAULT_BORDER_COLOR[1],     // 11: border_color.g
            DEFAULT_BORDER_COLOR[2],     // 12: border_color.b
            1.0f32,                      // 13: border_color.a
            DEFAULT_HANDLE_COLOR[0],     // 14: handle_color.r
            DEFAULT_HANDLE_COLOR[1],     // 15: handle_color.g
            DEFAULT_HANDLE_COLOR[2],     // 16: handle_color.b
            1.0f32,                      // 17: handle_color.a
        ];

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&box_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let mut state = State {
            window,
            device,
            queue,
            size,
            surface,
            surface_format,
            render_pipeline,
            vertex_buffer,
            bind_group: None,
            uniform_buffer,
            border_width: DEFAULT_BORDER_WIDTH,
            handle_size: DEFAULT_HANDLE_SIZE,
            handle_border_width: DEFAULT_HANDLE_BORDER_WIDTH,
            border_color: DEFAULT_BORDER_COLOR,
            handle_color: DEFAULT_HANDLE_COLOR,
        };

        state.configure_surface();
        state.load_screenshot();
        state
    }
    fn load_screenshot_from_data(&mut self, rgba: Vec<u8>, width: u32, height: u32) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: None,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self
            .device
            .create_sampler(&wgpu::SamplerDescriptor::default());

        let bind_group_layout = &self.render_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
            ],
            label: None,
        });

        self.bind_group = Some(bind_group);
    }
    fn configure_surface(&self) {
        if self.size.width > 0 && self.size.height > 0 {
            self.surface.configure(
                &self.device,
                &wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: self.surface_format,
                    width: self.size.width,
                    height: self.size.height,
                    present_mode: wgpu::PresentMode::Mailbox,
                    alpha_mode: wgpu::CompositeAlphaMode::Auto,
                    view_formats: vec![],
                    desired_maximum_frame_latency: 1,
                },
            );
        }
    }
    // 设置手柄边框宽度
    fn set_handle_border_width(&mut self, width: f32) {
        self.handle_border_width = width;
        self.update_uniforms();
    }

    // 更新所有uniform数据
    fn update_uniforms(&mut self) {
        if let Some((min_x, min_y, max_x, max_y)) = self.get_current_box() {
            self.update_box_with_params(min_x, min_y, max_x, max_y);
        }
    }
    fn update_box(&mut self, min_x: f32, min_y: f32, max_x: f32, max_y: f32) {
        self.update_box_with_params(min_x, min_y, max_x, max_y);
    }

    fn load_screenshot(&mut self) {
        // 使用常量创建测试纹理
        let mut data = Vec::new();
        for _ in 0..(TEST_TEXTURE_SIZE * TEST_TEXTURE_SIZE) {
            data.extend_from_slice(&TEST_TEXTURE_COLOR);
        }
        self.load_screenshot_from_data(data, TEST_TEXTURE_SIZE, TEST_TEXTURE_SIZE);
    }

    fn render(&mut self) {
        if self.size.width == 0 || self.size.height == 0 {
            return;
        }

        let output = self.surface.get_current_texture().unwrap();
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), // 改为黑色背景
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(bind_group) = &self.bind_group {
                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..6, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.configure_surface();
        // 不在这里更新框，让App来处理
    }
    fn resize_with_box(
        &mut self,
        new_size: winit::dpi::PhysicalSize<u32>,
        current_box: Option<(f32, f32, f32, f32)>,
    ) {
        self.size = new_size;
        self.configure_surface();

        let box_data = if let Some((min_x, min_y, max_x, max_y)) = current_box {
            [
                min_x,
                min_y,
                max_x,
                max_y,
                new_size.width as f32,
                new_size.height as f32,
                self.border_width,        // border_width
                self.handle_size,         // handle_size
                self.handle_border_width, // handle_border_width
                0.0f32,                   // 18: _padding1[0] - 新增
                0.0f32,                   // 18: _padding1[0] - 新增
                0.0f32,                   // 18: _padding1[0] - 新增
                self.border_color[0],     // border_color.r
                self.border_color[1],     // border_color.g
                self.border_color[2],     // border_color.b
                1.0,                      // border_color.a
                self.handle_color[0],     // handle_color.r
                self.handle_color[1],     // handle_color.g
                self.handle_color[2],     // handle_color.b
                1.0,                      // handle_color.a
            ]
        } else {
            // 没有框时，使用无效坐标
            [
                -1.0f32,
                -1.0f32,
                -1.0f32,
                -1.0f32,
                new_size.width as f32,
                new_size.height as f32,
                self.border_width,        // border_width
                self.handle_size,         // handle_size
                self.handle_border_width, // handle_border_width
                0.0f32,                   // 18: _padding1[0] - 新增
                0.0f32,                   // 18: _padding1[0] - 新增
                0.0f32,                   // 18: _padding1[0] - 新增
                self.border_color[0],     // border_color.r
                self.border_color[1],     // border_color.g
                self.border_color[2],     // border_color.b
                1.0,                      // border_color.a
                self.handle_color[0],     // handle_color.r
                self.handle_color[1],     // handle_color.g
                self.handle_color[2],     // handle_color.b
                1.0,                      // handle_color.a
            ]
        };

        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&box_data));
    }
    // 设置边框宽度
    fn set_border_width(&mut self, width: f32) {
        self.border_width = width;
        self.update_uniforms();
    }

    // 设置手柄大小
    fn set_handle_size(&mut self, size: f32) {
        self.handle_size = size;
        self.update_uniforms();
    }

    // 设置边框颜色
    fn set_border_color(&mut self, r: f32, g: f32, b: f32) {
        self.border_color = [r, g, b];
        self.update_uniforms();
    }

    // 设置手柄颜色
    fn set_handle_color(&mut self, r: f32, g: f32, b: f32) {
        self.handle_color = [r, g, b];
        self.update_uniforms();
    }

    // 获取当前框坐标的辅助方法
    fn get_current_box(&self) -> Option<(f32, f32, f32, f32)> {
        // 这个需要从App传递，或者存储在State中
        None // 临时返回
    }
    fn update_box_with_params(&mut self, min_x: f32, min_y: f32, max_x: f32, max_y: f32) {
        let box_data = [
            min_x,                    // 0: box_min.x
            min_y,                    // 1: box_min.y
            max_x,                    // 2: box_max.x
            max_y,                    // 3: box_max.y
            self.size.width as f32,   // 4: screen_size.x
            self.size.height as f32,  // 5: screen_size.y
            self.border_width,        // 6: border_width
            self.handle_size,         // 7: handle_size
            self.handle_border_width, // 8: handle_border_width
            0.0f32,                   // 18: _padding1[0] - 新增
            0.0f32,                   // 18: _padding1[0] - 新增
            0.0f32,                   // 18: _padding1[0] - 新增
            self.border_color[0],     // 10: border_color.r
            self.border_color[1],     // 11: border_color.g
            self.border_color[2],     // 12: border_color.b
            1.0,                      // 13: border_color.a
            self.handle_color[0],     // 14: handle_color.r
            self.handle_color[1],     // 15: handle_color.g
            self.handle_color[2],     // 16: handle_color.b
            1.0,                      // 17: handle_color.a
        ];
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&box_data));
    }
}

struct App {
    state: Option<State>,
    mouse_pressed: bool,
    box_start: (f32, f32),
    first_drag_move: bool,
    box_created: bool,                         // 添加标志：框是否已经创建
    current_box: Option<(f32, f32, f32, f32)>, // 当前框的坐标 (min_x, min_y, max_x, max_y)
    drag_mode: DragMode,                       // 拖拽模式
    last_update_time: std::time::Instant,      // 添加时间追踪
    needs_redraw: bool,                        // 添加重绘标志
    mouse_press_position: Option<(f32, f32)>,  // 添加鼠标按下位置
}
#[derive(PartialEq)]
enum DragMode {
    Creating,               // 创建新框
    Moving,                 // 移动现有框
    Resizing(ResizeHandle), // 调整大小，包含具体的手柄
    None,                   // 不在拖拽状态
}
#[derive(PartialEq, Clone, Copy)]
enum ResizeHandle {
    TopLeft,      // 左上
    TopCenter,    // 上中
    TopRight,     // 右上
    MiddleRight,  // 右中
    BottomRight,  // 右下
    BottomCenter, // 下中
    BottomLeft,   // 左下
    MiddleLeft,   // 左中
}
impl Default for App {
    fn default() -> Self {
        Self {
            state: None,
            mouse_pressed: false,
            box_start: (0.0, 0.0),
            first_drag_move: false,
            box_created: false,
            current_box: None,
            drag_mode: DragMode::None,
            last_update_time: std::time::Instant::now(),
            needs_redraw: false,
            mouse_press_position: None, // 添加初始化
        }
    }
}
// 静态函数，不依赖self
fn get_handle_at_position_static(
    mouse_x: f32,
    mouse_y: f32,
    current_box: Option<(f32, f32, f32, f32)>,
    handle_size: f32,
) -> Option<ResizeHandle> {
    if let Some((min_x, min_y, max_x, max_y)) = current_box {
        let center_x = (min_x + max_x) * 0.5;
        let center_y = (min_y + max_y) * 0.5;
        let half_handle = handle_size * 0.5;

        let handles = [
            (min_x, min_y, ResizeHandle::TopLeft),
            (center_x, min_y, ResizeHandle::TopCenter),
            (max_x, min_y, ResizeHandle::TopRight),
            (max_x, center_y, ResizeHandle::MiddleRight),
            (max_x, max_y, ResizeHandle::BottomRight),
            (center_x, max_y, ResizeHandle::BottomCenter),
            (min_x, max_y, ResizeHandle::BottomLeft),
            (min_x, center_y, ResizeHandle::MiddleLeft),
        ];

        for (handle_x, handle_y, handle_type) in handles.iter() {
            if mouse_x >= handle_x - half_handle
                && mouse_x <= handle_x + half_handle
                && mouse_y >= handle_y - half_handle
                && mouse_y <= handle_y + half_handle
            {
                return Some(*handle_type);
            }
        }
    }
    None
}

fn is_mouse_in_box_body_static(
    mouse_x: f32,
    mouse_y: f32,
    current_box: Option<(f32, f32, f32, f32)>,
    handle_size: f32,
) -> bool {
    if let Some((min_x, min_y, max_x, max_y)) = current_box {
        mouse_x >= min_x
            && mouse_x <= max_x
            && mouse_y >= min_y
            && mouse_y <= max_y
            && get_handle_at_position_static(mouse_x, mouse_y, current_box, handle_size).is_none()
    } else {
        false
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // 方法1：先截图再创建窗口
        let screenshot_data = if let Ok(screens) = screenshots::Screen::all() {
            if let Some(screen) = screens.first() {
                screen.capture().ok().map(|img| {
                    let dimensions = (img.width(), img.height());
                    let rgba = img.into_vec();
                    (rgba, dimensions.0, dimensions.1)
                })
            } else {
                None
            }
        } else {
            None
        };

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_decorations(false)
                        .with_transparent(false) // 改为不透明
                        .with_visible(false)
                        .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
                        .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None))),
                )
                .unwrap(),
        );

        let mut state = pollster::block_on(State::new(window.clone()));

        // 如果有预先截取的数据，使用它
        if let Some((rgba, width, height)) = screenshot_data {
            state.load_screenshot_from_data(rgba, width, height);
        }
        window.set_visible(true);
        self.state = Some(state);
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // 设置控制流：只在有事件时处理
        event_loop.set_control_flow(ControlFlow::Wait);
        if let Some(state) = self.state.as_mut() {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::RedrawRequested => {
                    state.render();
                }
                WindowEvent::Resized(size) => {
                    state.resize_with_box(size, self.current_box);
                }
                WindowEvent::MouseInput {
                    state: button_state,
                    button,
                    ..
                } => {
                    use winit::event::{ElementState, MouseButton};

                    if button == MouseButton::Left {
                        match button_state {
                            ElementState::Pressed => {
                                self.mouse_pressed = true;
                                self.first_drag_move = true;
                                event_loop.set_control_flow(ControlFlow::Poll);

                                if !self.box_created {
                                    // 第一次创建框
                                    self.drag_mode = DragMode::Creating;
                                } else {
                                    // 框已存在，暂时设置为Moving，在CursorMoved中确定具体模式
                                    self.drag_mode = DragMode::Moving;
                                }
                            }
                            ElementState::Released => {
                                self.mouse_pressed = false;
                                self.first_drag_move = false;
                                self.mouse_press_position = None;
                                event_loop.set_control_flow(ControlFlow::Wait);

                                match self.drag_mode {
                                    DragMode::Creating => {
                                        // 创建框完成，标记为已创建
                                        self.box_created = true;
                                    }
                                    DragMode::Resizing(_) => {
                                        // 调整大小完成
                                    }
                                    DragMode::Moving => {
                                        // 移动完成
                                    }
                                    DragMode::None => {}
                                }

                                self.drag_mode = DragMode::None;
                            }
                        }
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    if self.box_created && !self.mouse_pressed {
                        let mouse_x = position.x as f32;
                        let mouse_y = position.y as f32;
                        let current_box = self.current_box;
                        let handle_size = state.handle_size;

                        // 检测鼠标在哪个区域
                        if let Some(handle) = get_handle_at_position_static(
                            mouse_x,
                            mouse_y,
                            current_box,
                            handle_size,
                        ) {
                            // 在手柄上，设置对应的调整大小指针
                            let cursor = match handle {
                                ResizeHandle::TopLeft | ResizeHandle::BottomRight => {
                                    winit::window::CursorIcon::NwseResize
                                }
                                ResizeHandle::TopRight | ResizeHandle::BottomLeft => {
                                    winit::window::CursorIcon::NeswResize
                                }
                                ResizeHandle::TopCenter | ResizeHandle::BottomCenter => {
                                    winit::window::CursorIcon::NsResize
                                }
                                ResizeHandle::MiddleLeft | ResizeHandle::MiddleRight => {
                                    winit::window::CursorIcon::EwResize
                                }
                            };
                            state.window.set_cursor(cursor);
                        } else if is_mouse_in_box_body_static(
                            mouse_x,
                            mouse_y,
                            current_box,
                            handle_size,
                        ) {
                            // 在框内部，设置移动指针
                            state.window.set_cursor(winit::window::CursorIcon::Move);
                        } else {
                            // 在框外，设置默认指针
                            state.window.set_cursor(winit::window::CursorIcon::NotAllowed);
                        }
                    } else if !self.box_created && !self.mouse_pressed {
                        // 框未创建时，显示十字指针
                        state
                            .window
                            .set_cursor(winit::window::CursorIcon::Crosshair);
                    }
                    // 存储鼠标位置，用于按下时的检测
                    if self.mouse_pressed && self.mouse_press_position.is_none() {
                        self.mouse_press_position = Some((position.x as f32, position.y as f32));

                        // 如果框已创建，根据按下位置确定拖拽模式
                        if self.box_created {
                            let mouse_x = position.x as f32;
                            let mouse_y = position.y as f32;

                            // 先获取需要的数据，避免借用冲突
                            let current_box = self.current_box;
                            let handle_size = state.handle_size;

                            // 释放对state的借用，然后调用检测方法
                            let handle = get_handle_at_position_static(
                                mouse_x,
                                mouse_y,
                                current_box,
                                handle_size,
                            );

                            if let Some(handle) = handle {
                                self.drag_mode = DragMode::Resizing(handle);
                            } else if is_mouse_in_box_body_static(
                                mouse_x,
                                mouse_y,
                                current_box,
                                handle_size,
                            ) {
                                self.drag_mode = DragMode::Moving;
                            } else {
                                // 点击在框外，创建新框
                                self.drag_mode = DragMode::None;
                                self.mouse_pressed = false; // 取消鼠标按下状态
                            }
                        }
                    }

                    if !self.mouse_pressed || self.drag_mode == DragMode::None {
                        return;
                    }

                    let frame_limit = match self.drag_mode {
                        DragMode::Creating | DragMode::Moving | DragMode::Resizing(_) => {
                            FRAME_LIMIT_DRAG
                        }
                        DragMode::None => FRAME_LIMIT_IDLE,
                    };

                    let now = std::time::Instant::now();
                    if now.duration_since(self.last_update_time).as_millis() < frame_limit {
                        return;
                    }
                    self.last_update_time = now;

                    match self.drag_mode {
                        DragMode::Creating => {
                            // 创建新框的逻辑
                            if self.first_drag_move {
                                self.box_start = (position.x as f32, position.y as f32);
                                self.first_drag_move = false;
                            } else {
                                let current_pos = (position.x as f32, position.y as f32);
                                let min_x = self.box_start.0.min(current_pos.0);
                                let min_y = self.box_start.1.min(current_pos.1);
                                let max_x = self.box_start.0.max(current_pos.0);
                                let max_y = self.box_start.1.max(current_pos.1);

                                self.current_box = Some((min_x, min_y, max_x, max_y));
                                state.update_box(min_x, min_y, max_x, max_y);
                                self.needs_redraw = true;
                            }
                        }
                        DragMode::Moving => {
                            // 移动逻辑
                            if let Some((box_min_x, box_min_y, box_max_x, box_max_y)) =
                                self.current_box
                            {
                                if self.first_drag_move {
                                    self.box_start = (position.x as f32, position.y as f32);
                                    self.first_drag_move = false;
                                } else {
                                    let current_pos = (position.x as f32, position.y as f32);
                                    let offset_x = current_pos.0 - self.box_start.0;
                                    let offset_y = current_pos.1 - self.box_start.1;

                                    let new_min_x = box_min_x + offset_x;
                                    let new_min_y = box_min_y + offset_y;
                                    let new_max_x = box_max_x + offset_x;
                                    let new_max_y = box_max_y + offset_y;

                                    let screen_width = state.size.width as f32;
                                    let screen_height = state.size.height as f32;

                                    let clamped_min_x = new_min_x.max(0.0);
                                    let clamped_min_y = new_min_y.max(0.0);
                                    let clamped_max_x = new_max_x.min(screen_width);
                                    let clamped_max_y = new_max_y.min(screen_height);

                                    let box_width = box_max_x - box_min_x;
                                    let box_height = box_max_y - box_min_y;

                                    let (final_min_x, final_max_x) =
                                        if clamped_max_x == screen_width {
                                            (screen_width - box_width, screen_width)
                                        } else if clamped_min_x == 0.0 {
                                            (0.0, box_width)
                                        } else {
                                            (clamped_min_x, clamped_max_x)
                                        };

                                    let (final_min_y, final_max_y) =
                                        if clamped_max_y == screen_height {
                                            (screen_height - box_height, screen_height)
                                        } else if clamped_min_y == 0.0 {
                                            (0.0, box_height)
                                        } else {
                                            (clamped_min_y, clamped_max_y)
                                        };

                                    self.current_box =
                                        Some((final_min_x, final_min_y, final_max_x, final_max_y));
                                    self.box_start = current_pos;

                                    state.update_box(
                                        final_min_x,
                                        final_min_y,
                                        final_max_x,
                                        final_max_y,
                                    );
                                    state.window.request_redraw();
                                }
                            }
                        }
                        DragMode::Resizing(handle) => {
                            // 调整大小逻辑
                            if let Some((mut min_x, mut min_y, mut max_x, mut max_y)) =
                                self.current_box
                            {
                                let current_pos = (position.x as f32, position.y as f32);

                                match handle {
                                    ResizeHandle::TopLeft => {
                                        min_x = current_pos.0;
                                        min_y = current_pos.1;
                                    }
                                    ResizeHandle::TopCenter => {
                                        min_y = current_pos.1;
                                    }
                                    ResizeHandle::TopRight => {
                                        max_x = current_pos.0;
                                        min_y = current_pos.1;
                                    }
                                    ResizeHandle::MiddleRight => {
                                        max_x = current_pos.0;
                                    }
                                    ResizeHandle::BottomRight => {
                                        max_x = current_pos.0;
                                        max_y = current_pos.1;
                                    }
                                    ResizeHandle::BottomCenter => {
                                        max_y = current_pos.1;
                                    }
                                    ResizeHandle::BottomLeft => {
                                        min_x = current_pos.0;
                                        max_y = current_pos.1;
                                    }
                                    ResizeHandle::MiddleLeft => {
                                        min_x = current_pos.0;
                                    }
                                }

                                if min_x > max_x {
                                    std::mem::swap(&mut min_x, &mut max_x);
                                }
                                if min_y > max_y {
                                    std::mem::swap(&mut min_y, &mut max_y);
                                }

                                if max_x - min_x < MIN_BOX_SIZE {
                                    if matches!(
                                        handle,
                                        ResizeHandle::TopLeft
                                            | ResizeHandle::MiddleLeft
                                            | ResizeHandle::BottomLeft
                                    ) {
                                        min_x = max_x - MIN_BOX_SIZE;
                                    } else {
                                        max_x = min_x + MIN_BOX_SIZE;
                                    }
                                }
                                if max_y - min_y < MIN_BOX_SIZE {
                                    if matches!(
                                        handle,
                                        ResizeHandle::TopLeft
                                            | ResizeHandle::TopCenter
                                            | ResizeHandle::TopRight
                                    ) {
                                        min_y = max_y - MIN_BOX_SIZE;
                                    } else {
                                        max_y = min_y + MIN_BOX_SIZE;
                                    }
                                }

                                let screen_width = state.size.width as f32;
                                let screen_height = state.size.height as f32;

                                min_x = min_x.max(0.0);
                                min_y = min_y.max(0.0);
                                max_x = max_x.min(screen_width);
                                max_y = max_y.min(screen_height);

                                self.current_box = Some((min_x, min_y, max_x, max_y));
                                state.update_box(min_x, min_y, max_x, max_y);
                                state.window.request_redraw();
                            }
                        }
                        DragMode::None => {}
                    }

                    if self.needs_redraw {
                        state.window.request_redraw();
                        self.needs_redraw = false;
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    use winit::keyboard::{KeyCode, PhysicalKey};

                    if event.state == winit::event::ElementState::Pressed {
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::KeyR) => {
                                // R键：重置，清除框
                                self.box_created = false;
                                self.current_box = None;
                                self.drag_mode = DragMode::None;
                                state.update_box(-1.0, -1.0, -1.0, -1.0);
                                state.window.request_redraw();
                            }

                            PhysicalKey::Code(KeyCode::Escape) => {
                                event_loop.exit();
                            }
                            _ => {}
                        }
                    }
                }
                _ => (),
            }
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();

    // 设置事件循环为按需处理模式
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
