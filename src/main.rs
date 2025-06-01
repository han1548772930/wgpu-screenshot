#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowId},
};
// ===== 配置常量定义区域 =====、
// 在常量定义区域添加工具栏相关常量
const TOOLBAR_HEIGHT: f32 = 40.0; // 工具栏高度
const TOOLBAR_BACKGROUND_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 0.9]; // 白色半透明背景
const TOOLBAR_BUTTON_SIZE: f32 = 40.0; // 工具按钮大小
const TOOLBAR_BUTTON_MARGIN: f32 = 10.0; // 按钮间距
const TOOLBAR_ICON_SIZE: f32 = 24.0; // 图标大小
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
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tool {
    Rectangle, // 画框
    Circle,    // 画圆
    Arrow,     // 箭头
    Pen,       // 笔画
    Text,      // 文字
    Undo,      // 撤销
    Save,      // 保存
    Exit,      // 退出
    Complete,  // 完成
}

// 工具栏按钮结构
struct ToolbarButton {
    tool: Tool,
    rect: (f32, f32, f32, f32), // x, y, width, height
    label: &'static str,
    is_selected: bool,
}
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
    // 工具栏相关
    toolbar_buttons: Vec<ToolbarButton>,
    current_tool: Tool,
    show_toolbar: bool,
    current_box_coords: Option<(f32, f32, f32, f32)>, // 添加这个字段
    mouse_position: Option<(f32, f32)>,               // 添加鼠标位置跟踪
    hovered_button: Option<usize>,                    // 添加悬停按钮索引
    toolbar_active: bool, // 新增：工具栏是否处于激活状态（点击过工具栏）
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
        let show_toolbar_initial = false;
        let box_data = [
            -1.0f32,                                      // 0: box_min.x
            -1.0f32,                                      // 1: box_min.y
            -1.0f32,                                      // 2: box_max.x
            -1.0f32,                                      // 3: box_max.y
            size.width as f32,                            // 4: screen_size.x
            size.height as f32,                           // 5: screen_size.y
            DEFAULT_BORDER_WIDTH,                         // 6: border_width
            DEFAULT_HANDLE_SIZE,                          // 7: handle_size
            DEFAULT_HANDLE_BORDER_WIDTH,                  // 8: handle_border_width
            if show_toolbar_initial { 1.0 } else { 0.0 }, // 9: show_toolbar
            TOOLBAR_HEIGHT,                               // 10: toolbar_height
            -1.0f32,                                      // 11: hovered_button (初始无悬停)
            0.0f32,                                       // 12: toolbar_active (初始未激活)
            -1.0f32,                                      // 13: selected_button (初始无选中)
            0.0f32,                                       // 14: _padding.x
            0.0f32,                                       // 15: _padding.y
            DEFAULT_BORDER_COLOR[0],                      // 16: border_color.r
            DEFAULT_BORDER_COLOR[1],                      // 17: border_color.g
            DEFAULT_BORDER_COLOR[2],                      // 18: border_color.b
            1.0f32,                                       // 19: border_color.a
            DEFAULT_HANDLE_COLOR[0],                      // 20: handle_color.r
            DEFAULT_HANDLE_COLOR[1],                      // 21: handle_color.g
            DEFAULT_HANDLE_COLOR[2],                      // 22: handle_color.b
            1.0f32,                                       // 23: handle_color.a
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
            toolbar_buttons: Vec::new(),
            current_tool: Tool::Rectangle,
            show_toolbar: show_toolbar_initial,
            current_box_coords: None, // 初始化
            mouse_position: None,
            hovered_button: None,
            toolbar_active: false, // 新增
        };

        state.configure_surface();
        state.load_screenshot();
        state.initialize_toolbar();
        state
    }
    fn update_mouse_position(&mut self, x: f32, y: f32) {
        self.mouse_position = Some((x, y));

        // 检查是否悬停在工具栏按钮上
        let old_hovered = self.hovered_button;
        self.hovered_button = None;
        if self.show_toolbar {
            for (i, button) in self.toolbar_buttons.iter().enumerate() {
                let (btn_x, btn_y, btn_w, btn_h) = button.rect;
                if x >= btn_x && x <= btn_x + btn_w && y >= btn_y && y <= btn_y + btn_h {
                    self.hovered_button = Some(i);
                    break;
                }
            }
        }
        // 如果悬停状态发生变化，更新uniform数据
        if old_hovered != self.hovered_button {
            self.update_uniforms();
        }
    }
    // 初始化工具栏
    fn initialize_toolbar(&mut self) {
        self.toolbar_buttons = vec![
            ToolbarButton {
                tool: Tool::Rectangle,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                label: "⬛", // 矩形
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Circle,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                label: "⭕", // 圆形
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Arrow,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                label: "➤", // 箭头
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Pen,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                label: "✏️", // 笔
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Text,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                label: "𝐀", // 文字
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Undo,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                label: "↶", // 撤销
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Save,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                label: "💾", // 保存
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Exit,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                label: "❌", // 退出
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Complete,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                label: "✅", // 完成
                is_selected: false,
            },
        ];
        self.update_toolbar_layout();
    }
    // 更新工具栏布局
    fn update_toolbar_layout(&mut self) {
        if let Some((box_min_x, box_min_y, box_max_x, box_max_y)) = self.get_current_box_coords() {
            // 首先尝试在框的下方显示工具栏
            let mut toolbar_y = box_max_y + 10.0;
            let toolbar_start_x = box_min_x;

            // 计算工具栏总宽度
            let total_width = (self.toolbar_buttons.len() as f32)
                * (TOOLBAR_BUTTON_SIZE + TOOLBAR_BUTTON_MARGIN)
                - TOOLBAR_BUTTON_MARGIN;

            // 检查工具栏是否超出屏幕下边界
            let toolbar_bottom = toolbar_y + TOOLBAR_HEIGHT;
            if toolbar_bottom > self.size.height as f32 {
                // 如果超出下边界，将工具栏移到框的上方
                toolbar_y = box_min_y - TOOLBAR_HEIGHT - 10.0;

                // 如果移到上方还是超出屏幕，则放在屏幕顶部
                if toolbar_y < 0.0 {
                    toolbar_y = 10.0;
                }
            }

            // 调整X坐标，确保工具栏不超出屏幕左右边界
            let adjusted_x = if toolbar_start_x + total_width > self.size.width as f32 {
                (self.size.width as f32 - total_width).max(0.0)
            } else {
                toolbar_start_x.max(0.0)
            };

            // 更新所有按钮的位置
            for (i, button) in self.toolbar_buttons.iter_mut().enumerate() {
                let x = adjusted_x + (i as f32) * (TOOLBAR_BUTTON_SIZE + TOOLBAR_BUTTON_MARGIN);
                button.rect = (x, toolbar_y, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE);
            }
        }
    }

    // 获取当前框坐标
    fn get_current_box_coords(&self) -> Option<(f32, f32, f32, f32)> {
        // 这里需要存储当前框坐标
        self.current_box_coords
    }
    // 更新当前框坐标
    fn set_current_box_coords(&mut self, coords: Option<(f32, f32, f32, f32)>) {
        self.current_box_coords = coords;
        // 移除这里的show_toolbar调用，避免重复借用
        if coords.is_some() {
            self.update_toolbar_layout();
        }
    }
    // 显示工具栏
    fn show_toolbar(&mut self) {
        self.show_toolbar = true;
        if self.current_box_coords.is_some() {
            self.update_toolbar_layout();
        }
    }

    // 隐藏工具栏
    fn hide_toolbar(&mut self) {
        self.show_toolbar = false;
    }

    // 检查鼠标是否在工具栏按钮上
    fn get_toolbar_button_at(&self, x: f32, y: f32) -> Option<Tool> {
        if !self.show_toolbar {
            return None;
        }

        for button in &self.toolbar_buttons {
            let (btn_x, btn_y, btn_w, btn_h) = button.rect;
            if x >= btn_x && x <= btn_x + btn_w && y >= btn_y && y <= btn_y + btn_h {
                return Some(button.tool);
            }
        }
        None
    }

    // 设置当前工具
    fn set_current_tool(&mut self, tool: Tool) {
        self.current_tool = tool;
    }

    // 处理工具栏按钮点击
    fn handle_toolbar_click(&mut self, tool: Tool) -> bool {
        println!("Toolbar clicked: {:?}", tool); // 调试信息

        // 激活工具栏状态
        self.toolbar_active = true;

        // 更新按钮选中状态 - 所有按钮先设为未选中
        for button in &mut self.toolbar_buttons {
            button.is_selected = false;
        }

        // 设置当前点击的按钮为选中状态
        for (i, button) in self.toolbar_buttons.iter_mut().enumerate() {
            if button.tool == tool {
                button.is_selected = true;
                println!("Button {} ({:?}) selected", i, tool); // 调试信息
                break;
            }
        }

        // 验证选中状态
        let selected_count = self
            .toolbar_buttons
            .iter()
            .filter(|b| b.is_selected)
            .count();
        println!("Total selected buttons: {}", selected_count); // 调试信息
        match tool {
            Tool::Rectangle | Tool::Circle | Tool::Arrow | Tool::Pen | Tool::Text => {
                self.set_current_tool(tool);
                self.update_uniforms(); // 更新uniform数据
                false // 不退出应用
            }
            Tool::Undo => {
                // TODO: 实现撤销功能
                println!("撤销操作");
                self.update_uniforms(); // 重要：更新uniform数据
                false
            }
            Tool::Save => {
                // TODO: 实现保存功能
                println!("保存截图");
                self.update_uniforms(); // 重要：更新uniform数据
                false
            }
            Tool::Exit => {
                // self.update_uniforms(); // 重要：更新uniform数据
                true // 退出应用
            }
            Tool::Complete => {
                // TODO: 完成截图并复制到剪贴板
                println!("完成截图");
                self.update_uniforms(); // 重要：更新uniform数据
                false // 改为不退出，让用户看到选中效果
            }
        }
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
                    desired_maximum_frame_latency: 3,
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
        let hovered_index = self.hovered_button.map(|i| i as f32).unwrap_or(-1.0);
        let selected_index = self
            .toolbar_buttons
            .iter()
            .position(|btn| btn.is_selected)
            .map(|i| i as f32)
            .unwrap_or(-1.0);

        let uniform_data = [
            self.current_box_coords
                .map_or(-1.0, |(min_x, _, _, _)| min_x), // 0: box_min.x
            self.current_box_coords
                .map_or(-1.0, |(_, min_y, _, _)| min_y), // 1: box_min.y
            self.current_box_coords
                .map_or(-1.0, |(_, _, max_x, _)| max_x), // 2: box_max.x
            self.current_box_coords
                .map_or(-1.0, |(_, _, _, max_y)| max_y), // 3: box_max.y
            self.size.width as f32,                      // 4: screen_size.x
            self.size.height as f32,                     // 5: screen_size.y
            self.border_width,                           // 6: border_width
            self.handle_size,                            // 7: handle_size
            self.handle_border_width,                    // 8: handle_border_width
            if self.show_toolbar { 1.0 } else { 0.0 },   // 9: show_toolbar
            TOOLBAR_HEIGHT,                              // 10: toolbar_height
            hovered_index,                               // 11: hovered_button
            if self.toolbar_active { 1.0 } else { 0.0 }, // 12: toolbar_active
            selected_index,                              // 13: selected_button
            0.0,                                         // 14: _padding.x
            0.0,                                         // 15: _padding.y
            self.border_color[0],                        // 16: border_color.r
            self.border_color[1],                        // 17: border_color.g
            self.border_color[2],                        // 18: border_color.b
            1.0,                                         // 19: border_color.a
            self.handle_color[0],                        // 20: handle_color.r
            self.handle_color[1],                        // 21: handle_color.g
            self.handle_color[2],                        // 22: handle_color.b
            1.0,                                         // 23: handle_color.a
        ];

        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&uniform_data));
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

        let hovered_index = self.hovered_button.map(|i| i as f32).unwrap_or(-1.0);
        let selected_index = self
            .toolbar_buttons
            .iter()
            .position(|btn| btn.is_selected)
            .map(|i| i as f32)
            .unwrap_or(-1.0);

        let box_data = if let Some((min_x, min_y, max_x, max_y)) = current_box {
            [
                min_x,                                       // 0: box_min.x
                min_y,                                       // 1: box_min.y
                max_x,                                       // 2: box_max.x
                max_y,                                       // 3: box_max.y
                new_size.width as f32,                       // 4: screen_size.x
                new_size.height as f32,                      // 5: screen_size.y
                self.border_width,                           // 6: border_width
                self.handle_size,                            // 7: handle_size
                self.handle_border_width,                    // 8: handle_border_width
                if self.show_toolbar { 1.0 } else { 0.0 },   // 9: show_toolbar
                TOOLBAR_HEIGHT,                              // 10: toolbar_height
                hovered_index,                               // 11: hovered_button
                if self.toolbar_active { 1.0 } else { 0.0 }, // 12: toolbar_active
                selected_index,                              // 13: selected_button
                0.0,                                         // 14: _padding.x
                0.0,                                         // 15: _padding.y
                self.border_color[0],                        // 16: border_color.r
                self.border_color[1],                        // 17: border_color.g
                self.border_color[2],                        // 18: border_color.b
                1.0,                                         // 19: border_color.a
                self.handle_color[0],                        // 20: handle_color.r
                self.handle_color[1],                        // 21: handle_color.g
                self.handle_color[2],                        // 22: handle_color.b
                1.0,                                         // 23: handle_color.a
            ]
        } else {
            // 没有框时，使用无效坐标
            [
                -1.0f32,                                     // 0: box_min.x
                -1.0f32,                                     // 1: box_min.y
                -1.0f32,                                     // 2: box_max.x
                -1.0f32,                                     // 3: box_max.y
                new_size.width as f32,                       // 4: screen_size.x
                new_size.height as f32,                      // 5: screen_size.y
                self.border_width,                           // 6: border_width
                self.handle_size,                            // 7: handle_size
                self.handle_border_width,                    // 8: handle_border_width
                if self.show_toolbar { 1.0 } else { 0.0 },   // 9: show_toolbar
                TOOLBAR_HEIGHT,                              // 10: toolbar_height
                hovered_index,                               // 11: hovered_button
                if self.toolbar_active { 1.0 } else { 0.0 }, // 12: toolbar_active
                selected_index,                              // 13: selected_button
                0.0,                                         // 14: _padding.x
                0.0,                                         // 15: _padding.y
                self.border_color[0],                        // 16: border_color.r
                self.border_color[1],                        // 17: border_color.g
                self.border_color[2],                        // 18: border_color.b
                1.0,                                         // 19: border_color.a
                self.handle_color[0],                        // 20: handle_color.r
                self.handle_color[1],                        // 21: handle_color.g
                self.handle_color[2],                        // 22: handle_color.b
                1.0,                                         // 23: handle_color.a
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
        // 更新当前框坐标
        self.current_box_coords = Some((min_x, min_y, max_x, max_y));

        let hovered_index = self.hovered_button.map(|i| i as f32).unwrap_or(-1.0);
        let selected_index = self
            .toolbar_buttons
            .iter()
            .position(|btn| btn.is_selected)
            .map(|i| i as f32)
            .unwrap_or(-1.0);

        let box_data = [
            min_x,                                       // 0: box_min.x
            min_y,                                       // 1: box_min.y
            max_x,                                       // 2: box_max.x
            max_y,                                       // 3: box_max.y
            self.size.width as f32,                      // 4: screen_size.x
            self.size.height as f32,                     // 5: screen_size.y
            self.border_width,                           // 6: border_width
            self.handle_size,                            // 7: handle_size
            self.handle_border_width,                    // 8: handle_border_width
            if self.show_toolbar { 1.0 } else { 0.0 },   // 9: show_toolbar
            TOOLBAR_HEIGHT,                              // 10: toolbar_height
            hovered_index,                               // 11: hovered_button
            if self.toolbar_active { 1.0 } else { 0.0 }, // 12: toolbar_active
            selected_index,                              // 13: selected_button
            0.0,                                         // 14: _padding.x
            0.0,                                         // 15: _padding.y
            self.border_color[0],                        // 16: border_color.r
            self.border_color[1],                        // 17: border_color.g
            self.border_color[2],                        // 18: border_color.b
            1.0,                                         // 19: border_color.a
            self.handle_color[0],                        // 20: handle_color.r
            self.handle_color[1],                        // 21: handle_color.g
            self.handle_color[2],                        // 22: handle_color.b
            1.0,                                         // 23: handle_color.a
        ];

        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&box_data));

        // 更新工具栏布局
        if self.show_toolbar {
            self.update_toolbar_layout();
        }
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
impl App {
    // 修改App结构，添加工具栏支持
    fn get_current_box(&self) -> Option<(f32, f32, f32, f32)> {
        self.current_box
    }

    // 更新State中的框坐标获取方法
    fn update_state_box_coords(&mut self) {
        if let Some(state) = &mut self.state {
            state.set_current_box_coords(self.current_box);
        }
    }
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
                                if let Some(state) = &self.state {
                                    if state.toolbar_active {
                                        // 工具栏激活后，只允许工具栏点击，禁用框拖拽
                                        return;
                                    }
                                }
                                self.mouse_pressed = true;
                                self.first_drag_move = true;
                                event_loop.set_control_flow(ControlFlow::Poll);

                                if !self.box_created {
                                    self.drag_mode = DragMode::Creating;
                                } else {
                                    self.drag_mode = DragMode::Moving;
                                }
                            }
                            ElementState::Released => {
                                self.mouse_pressed = false;
                                self.first_drag_move = false;
                                self.mouse_press_position = None;
                                event_loop.set_control_flow(ControlFlow::Wait);
                                if let Some(mouse_pos) = state.mouse_position {
                                    // 检查是否点击了工具栏 - 在这里处理点击
                                    let toolbar_tool =
                                        state.get_toolbar_button_at(mouse_pos.0, mouse_pos.1);
                                    if let Some(tool) = toolbar_tool {
                                        let should_exit = state.handle_toolbar_click(tool);
                                        state.window.request_redraw();
                                        if should_exit {
                                            event_loop.exit();
                                            return;
                                        }
                                        self.mouse_pressed = false; // 重要：阻止后续拖拽
                                        return;
                                    }
                                }
                                match self.drag_mode {
                                    DragMode::Creating => {
                                        if let Some((min_x, min_y, max_x, max_y)) = self.current_box
                                        {
                                            if max_x - min_x >= MIN_BOX_SIZE
                                                && max_y - min_y >= MIN_BOX_SIZE
                                            {
                                                self.box_created = true;
                                                // 分别调用，避免借用冲突
                                                state.show_toolbar();
                                                state.set_current_box_coords(self.current_box);
                                                state.update_box(min_x, min_y, max_x, max_y);
                                                state.window.request_redraw();
                                            }
                                        }
                                    }
                                    DragMode::Resizing(_) | DragMode::Moving => {
                                        state.set_current_box_coords(self.current_box);
                                        state.window.request_redraw();
                                    }
                                    DragMode::None => {}
                                }

                                self.drag_mode = DragMode::None;
                            }
                        }
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let old_hovered = state.hovered_button;
                    state.update_mouse_position(position.x as f32, position.y as f32);

                    // 如果悬停状态发生变化，请求重绘
                    if old_hovered != state.hovered_button {
                        state.window.request_redraw();
                    }
                    // 处理工具栏点击检查 - 移到最前面避免借用冲突
                    if self.mouse_pressed && self.mouse_press_position.is_none() {
                        let mouse_pos = (position.x as f32, position.y as f32);
                        self.mouse_press_position = Some(mouse_pos);

                        // 如果框已创建，根据按下位置确定拖拽模式
                        if self.box_created {
                            let current_box = self.current_box;
                            let handle_size = state.handle_size;

                            let handle = get_handle_at_position_static(
                                mouse_pos.0,
                                mouse_pos.1,
                                current_box,
                                handle_size,
                            );

                            if let Some(handle) = handle {
                                self.drag_mode = DragMode::Resizing(handle);
                            } else if is_mouse_in_box_body_static(
                                mouse_pos.0,
                                mouse_pos.1,
                                current_box,
                                handle_size,
                            ) {
                                self.drag_mode = DragMode::Moving;
                            } else {
                                self.drag_mode = DragMode::None;
                                self.mouse_pressed = false;
                            }
                        }
                    }

                    // 处理鼠标指针样式
                    if !self.mouse_pressed {
                        let mouse_x = position.x as f32;
                        let mouse_y = position.y as f32;

                        // 先提取需要的值，避免在检查过程中持续借用state
                        let toolbar_button_exists =
                            state.get_toolbar_button_at(mouse_x, mouse_y).is_some();
                        let current_box = self.current_box;
                        let handle_size = state.handle_size;
                        let toolbar_active = state.toolbar_active; // 提前获取这个值

                        // 优先检查工具栏
                        if toolbar_button_exists {
                            state.window.set_cursor(winit::window::CursorIcon::Pointer); // 改为手型指针
                        } else if self.box_created && !toolbar_active {
                            // 只有在工具栏未激活时才显示调整大小指针
                            if let Some(handle) = get_handle_at_position_static(
                                mouse_x,
                                mouse_y,
                                current_box,
                                handle_size,
                            ) {
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
                                state.window.set_cursor(winit::window::CursorIcon::Move);
                            } else {
                                state
                                    .window
                                    .set_cursor(winit::window::CursorIcon::NotAllowed); // 改为十字指针，更适合截图
                            }
                        } else if self.box_created && toolbar_active {
                            // 工具栏激活时，需要区分框内和框外
                            if is_mouse_in_box_body_static(
                                mouse_x,
                                mouse_y,
                                current_box,
                                handle_size,
                            ) {
                                // 在框内：显示默认指针
                                state.window.set_cursor(winit::window::CursorIcon::Default);
                            } else {
                                // 在框外：显示禁止指针
                                state
                                    .window
                                    .set_cursor(winit::window::CursorIcon::NotAllowed);
                            }
                        } else {
                            // 没有框时，显示十字指针
                            state
                                .window
                                .set_cursor(winit::window::CursorIcon::Crosshair);
                        }
                    }
                    // 工具栏激活后禁用所有拖拽操作
                    if state.toolbar_active {
                        return;
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
                            if self.first_drag_move {
                                self.box_start = (position.x as f32, position.y as f32);
                                self.first_drag_move = false;
                            } else {
                                let current_pos = (position.x as f32, position.y as f32);
                                let min_x = self.box_start.0.min(current_pos.0);
                                let min_y = self.box_start.1.min(current_pos.1);
                                let max_x = self.box_start.0.max(current_pos.0);
                                let max_y = self.box_start.1.max(current_pos.1);

                                if max_x - min_x >= MIN_BOX_SIZE && max_y - min_y >= MIN_BOX_SIZE {
                                    self.current_box = Some((min_x, min_y, max_x, max_y));
                                    state.update_box(min_x, min_y, max_x, max_y);
                                    self.needs_redraw = true;
                                }
                            }
                        }
                        DragMode::Moving => {
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

                                    // 调整大小时更新工具栏位置
                                    if self.box_created {
                                        state.set_current_box_coords(self.current_box);
                                    }
                                    state.window.request_redraw();
                                }
                            }
                        }
                        DragMode::Resizing(handle) => {
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

                                // 调整大小时更新工具栏位置
                                if self.box_created {
                                    state.set_current_box_coords(self.current_box);
                                }

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
                                // R键重置
                                self.box_created = false;
                                self.current_box = None;
                                self.drag_mode = DragMode::None;
                                state.hide_toolbar();
                                state.set_current_box_coords(None);
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
