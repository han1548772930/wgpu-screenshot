#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod text_renderer;

use resvg::tiny_skia::Pixmap;
use std::sync::Arc;
use text_renderer::TextRenderer;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowId},
};

// ===== 配置常量定义区域 =====
const TOOLBAR_HEIGHT: f32 = 40.0;
const TOOLBAR_BUTTON_SIZE: f32 = 30.0;
const TOOLBAR_BUTTON_MARGIN: f32 = 10.0;

// 颜色常量
const CYAN: [f32; 3] = [0.0, 1.0, 1.0];
const RED: [f32; 3] = [1.0, 0.0, 0.0];

// 默认配置常量
const DEFAULT_BORDER_WIDTH: f32 = 1.0;
const DEFAULT_HANDLE_SIZE: f32 = 16.0;
const MIN_ELLIPSE_RADIUS: f32 = 5.0; // 椭圆最小半径，防止椭圆消失
const MIN_RECTANGLE_SIZE: f32 = 5.0; // 矩形最小尺寸，防止矩形消失

// 🚀 新增：保存时的最小尺寸限制
const MIN_SAVE_SIZE: f32 = 20.0; // 保存图形的最小尺寸（像素）
const MIN_SAVE_RADIUS: f32 = 10.0; // 保存圆形的最小半径（像素）
const MIN_ARROW_LENGTH: f32 = 30.0; // 保存箭头的最小长度（像素）
const DEFAULT_HANDLE_BORDER_WIDTH: f32 = 1.0;
const DEFAULT_BORDER_COLOR: [f32; 3] = CYAN;
const DEFAULT_HANDLE_COLOR: [f32; 3] = CYAN;

// 拖拽配置常量
const MIN_BOX_SIZE: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Tool {
    None, // 🚀 无选择状态
    Rectangle,
    Circle,
    Arrow,
    Pen,
    Text,
    Undo,
    Save,
    Exit,
    Complete,
}
// 新增：绘图元素类型
#[derive(Debug, Clone)]
enum DrawingElement {
    Rectangle {
        start: (f32, f32),
        end: (f32, f32),
        color: [f32; 3],
        thickness: f32,
    },
    Circle {
        center: (f32, f32),
        radius_x: f32, // 水平半径
        radius_y: f32, // 垂直半径
        color: [f32; 3],
        thickness: f32,
    },
    Arrow {
        start: (f32, f32),
        end: (f32, f32),
        color: [f32; 3],
        thickness: f32,
    },
    Pen {
        points: Vec<(f32, f32)>,
        color: [f32; 3],
        thickness: f32,
    },
    Text {
        position: (f32, f32),
        content: String,
        color: [f32; 3],
        font_size: f32,
        is_editing: bool,      // 是否正在编辑状态
        rotation: Option<f32>, // 🚀 新增：旋转角度（弧度）
    },
}
#[derive(Debug, Clone, Copy, PartialEq)]
enum DrawingState {
    Idle,
    Drawing,
    Editing, // 🚀 新增：编辑模式
}

// 🚀 手柄类型枚举
#[derive(Debug, Clone, Copy, PartialEq)]
enum HandleType {
    // 矩形的8个调整手柄
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    // 圆形现在使用矩形手柄类型（TopLeft, TopCenter等）
    // 箭头的2个调整手柄
    ArrowStart,
    ArrowEnd,
    // 移动手柄
    Move,
    // 🚀 新增：旋转手柄
    Rotate,
}

// 🚀 手柄结构
#[derive(Debug, Clone)]
struct Handle {
    handle_type: HandleType,
    position: (f32, f32),
    size: f32,
    element_index: usize, // 关联的绘图元素索引
}

// 🚀 选中的绘图元素信息
#[derive(Debug, Clone)]
struct SelectedElement {
    index: usize,
    handles: Vec<Handle>,
    is_moving: bool,
    move_offset: (f32, f32),
}

struct ToolbarButton {
    tool: Tool,
    rect: (f32, f32, f32, f32),
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
    current_box_coords: Option<(f32, f32, f32, f32)>,
    mouse_position: Option<(f32, f32)>,
    hovered_button: Option<usize>,
    toolbar_active: bool,

    // 图标相关
    icon_textures: std::collections::HashMap<Tool, wgpu::Texture>,
    icon_bind_groups: std::collections::HashMap<Tool, wgpu::BindGroup>,
    icon_render_pipeline: wgpu::RenderPipeline,

    // � GPU优化：降低使用率 - 移除MSAA以减少GPU负载
    needs_redraw: bool,
    render_cache_valid: bool,
    // 🚀 背景缓存系统
    background_cache_texture: Option<wgpu::Texture>,
    background_cache_view: Option<wgpu::TextureView>,
    background_cache_bind_group: Option<wgpu::BindGroup>,
    background_cache_valid: bool,
    force_background_update: bool,
    background_cache_pipeline: wgpu::RenderPipeline,

    drawing_elements: Vec<DrawingElement>,
    current_drawing: Option<DrawingElement>,
    drawing_state: DrawingState,
    drawing_start_pos: Option<(f32, f32)>,
    pen_points: Vec<(f32, f32)>,

    // 🚀 绘图元素选择和编辑系统
    selected_element: Option<SelectedElement>,
    hovered_handle: Option<Handle>,
    dragging_handle: Option<Handle>,

    // 🚀 鼠标指针状态
    current_cursor: winit::window::CursorIcon,

    // 🚀 撤销系统
    undo_stack: Vec<Vec<DrawingElement>>, // 撤销栈，存储历史状态
    redo_stack: Vec<Vec<DrawingElement>>, // 重做栈

    // 🚀 修饰键状态
    modifiers: winit::event::Modifiers,

    // 🚀 文本输入状态
    text_input_active: bool,
    current_text_input: String,
    text_cursor_position: usize,

    // 绘图渲染相关
    drawing_render_pipeline: wgpu::RenderPipeline,
    drawing_vertex_buffer: Option<wgpu::Buffer>,

    // 🚀 绘图元素缓存系统
    cached_drawing_vertices: std::collections::HashMap<String, Vec<f32>>,
    drawing_cache_valid: std::collections::HashMap<String, bool>,

    // 🚀 文本渲染器
    text_renderer: TextRenderer,

    // 🚀 文本缓存
    text_buffer_cache: Option<glyphon::Buffer>,
    cached_text_content: String,

    // 🚀 双击检测
    last_click_time: std::time::Instant,
    last_click_position: Option<(f32, f32)>,
    double_click_threshold: std::time::Duration,
}

impl State {
    async fn new(window: Arc<Window>) -> State {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();
        // 检查设备是否支持管道缓存功能
        let features = if adapter.features().contains(wgpu::Features::PIPELINE_CACHE) {
            wgpu::Features::PIPELINE_CACHE
        } else {
            wgpu::Features::empty()
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: features,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();
        let surface = instance.create_surface(window.clone()).unwrap();
        let cap = surface.get_capabilities(&adapter);
        let surface_format = cap.formats[0];

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        // 主绑定组布局 (group 0)
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
            label: Some("Main Bind Group Layout"),
        });

        // 🚀 背景缓存绑定组布局 (group 1)
        let background_cache_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                ],
                label: Some("Background Cache Bind Group Layout"),
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Main Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout, &background_cache_bind_group_layout],
            push_constant_ranges: &[],
        });

        // 创建管道缓存以提高性能（如果支持的话）
        let pipeline_cache = if device.features().contains(wgpu::Features::PIPELINE_CACHE) {
            Some(unsafe {
                device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                    label: Some("Main Pipeline Cache"),
                    data: None,
                    fallback: true,
                })
            })
        } else {
            None
        };

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Main Render Pipeline"),
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
            cache: pipeline_cache.as_ref(),
        });
        let drawing_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Drawing Render Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_drawing"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 28, // 7 floats: x, y, r, g, b, a, thickness
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x2, // position
                            },
                            wgpu::VertexAttribute {
                                offset: 8,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x4, // color
                            },
                            wgpu::VertexAttribute {
                                offset: 24,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32, // thickness
                            },
                        ],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_drawing"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::LineList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: pipeline_cache.as_ref(),
            });
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
            -1.0f32,
            -1.0f32,
            -1.0f32,
            -1.0f32, // box coordinates
            size.width as f32,
            size.height as f32, // screen size
            DEFAULT_BORDER_WIDTH,
            DEFAULT_HANDLE_SIZE,
            DEFAULT_HANDLE_BORDER_WIDTH, // border/handle params
            0.0,
            TOOLBAR_HEIGHT,
            -1.0,
            0.0,
            -1.0, // toolbar params
            TOOLBAR_BUTTON_SIZE,
            TOOLBAR_BUTTON_MARGIN, // button params
            DEFAULT_BORDER_COLOR[0],
            DEFAULT_BORDER_COLOR[1],
            DEFAULT_BORDER_COLOR[2],
            1.0, // border color
            DEFAULT_HANDLE_COLOR[0],
            DEFAULT_HANDLE_COLOR[1],
            DEFAULT_HANDLE_COLOR[2],
            1.0, // handle color
            9.0,
            0.0,
            0.0,
            0.0, // button count + padding
            0.0,
            0.0,
            0.0,
            0.0, // extra padding
        ];

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&box_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let icon_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Icon Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_icon"),
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
                entry_point: Some("fs_icon"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: pipeline_cache.as_ref(),
        });

        // 🚀 创建背景缓存渲染管道 - 只使用主绑定组布局
        let background_cache_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Background Cache Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout], // 只使用主绑定组
                push_constant_ranges: &[],
            });

        let background_cache_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Background Cache Pipeline"),
                layout: Some(&background_cache_pipeline_layout),
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
                    entry_point: Some("fs_background_cache"),
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
                cache: pipeline_cache.as_ref(),
            });

        // 🚀 初始化文本渲染器
        let text_renderer =
            TextRenderer::new(&device, &queue, size.width, size.height, surface_format)
                .expect("Failed to create text renderer");

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
            current_tool: Tool::None, // 🚀 初始状态无工具选中
            show_toolbar: false,
            current_box_coords: None,
            mouse_position: None,
            hovered_button: None,
            toolbar_active: false,
            icon_textures: std::collections::HashMap::new(),
            icon_bind_groups: std::collections::HashMap::new(),
            icon_render_pipeline,
            // 新增绘图相关字段
            drawing_elements: Vec::new(),
            current_drawing: None,
            drawing_state: DrawingState::Idle,
            drawing_start_pos: None,
            pen_points: Vec::new(),
            // 🚀 绘图元素选择和编辑系统初始化
            selected_element: None,
            hovered_handle: None,
            dragging_handle: None,
            // 🚀 鼠标指针状态初始化
            current_cursor: winit::window::CursorIcon::Default,

            // 🚀 撤销系统初始化
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),

            // 🚀 修饰键状态初始化
            modifiers: winit::event::Modifiers::default(),

            // 🚀 文本输入状态初始化
            text_input_active: false,
            current_text_input: String::new(),
            text_cursor_position: 0,
            drawing_render_pipeline,
            drawing_vertex_buffer: None,
            // 🚀 绘图元素缓存系统初始化
            cached_drawing_vertices: std::collections::HashMap::new(),
            drawing_cache_valid: std::collections::HashMap::new(),

            needs_redraw: true,
            render_cache_valid: false,
            // 🚀 背景缓存系统初始化
            background_cache_texture: None,
            background_cache_view: None,
            background_cache_bind_group: None,
            background_cache_valid: false,
            force_background_update: false,
            background_cache_pipeline,
            // 🚀 文本渲染器
            text_renderer,

            // 🚀 文本缓存初始化
            text_buffer_cache: None,
            cached_text_content: String::new(),

            // 🚀 双击检测初始化
            last_click_time: std::time::Instant::now(),
            last_click_position: None,
            double_click_threshold: std::time::Duration::from_millis(500),
        };

        state.configure_surface();

        state.initialize_toolbar();
        state.initialize_svg_icons();

        // 🚀 初始化撤销/重做按钮状态
        state.update_undo_redo_button_states();

        state
    }
    // 新增：开始绘图
    fn start_drawing(&mut self, x: f32, y: f32) {
        // 只在截图框内绘图
        if !self.is_point_in_screenshot_area(x, y) {
            return;
        }

        self.drawing_state = DrawingState::Drawing;
        self.drawing_start_pos = Some((x, y));

        match self.current_tool {
            Tool::None => {
                // 🚀 无工具选择时不能绘制
                return;
            }
            Tool::Rectangle => {
                self.current_drawing = Some(DrawingElement::Rectangle {
                    start: (x, y),
                    end: (x, y),
                    color: RED,
                    thickness: 2.0,
                });
            }
            Tool::Circle => {
                self.current_drawing = Some(DrawingElement::Circle {
                    center: (x, y),
                    radius_x: 0.0,
                    radius_y: 0.0,
                    color: RED,
                    thickness: 2.0,
                });
            }
            Tool::Arrow => {
                self.current_drawing = Some(DrawingElement::Arrow {
                    start: (x, y),
                    end: (x, y),
                    color: RED,
                    thickness: 2.0,
                });
            }
            Tool::Pen => {
                self.pen_points.clear();
                self.pen_points.push((x, y));
                self.current_drawing = Some(DrawingElement::Pen {
                    points: vec![(x, y)],
                    color: RED,
                    thickness: 2.0, // 恢复正常厚度
                });
            }
            Tool::Text => {
                // 🚀 文本工具：创建文本元素并开始文本输入
                self.current_drawing = Some(DrawingElement::Text {
                    position: (x, y),
                    content: String::new(),
                    color: RED,
                    font_size: 24.0, // 增大字体
                    is_editing: true,
                    rotation: None, // 🚀 初始无旋转
                });

                // 激活文本输入模式
                self.text_input_active = true;
                self.current_text_input.clear();
                self.text_cursor_position = 0;

                // 🚀 确保进入正确的绘图状态
                self.drawing_state = DrawingState::Drawing;

                println!("🚀 开始文本输入模式，位置: ({}, {})", x, y);
            }
            _ => {}
        }

        // 🔧 修复：开始绘图时标记需要重绘
        self.needs_redraw = true;
        self.render_cache_valid = false;
        // 🚀 开始绘图时失效相关缓存
        self.invalidate_drawing_cache();
    }

    fn update_drawing(&mut self, x: f32, y: f32) {
        if self.drawing_state != DrawingState::Drawing {
            return;
        }

        if !self.is_point_in_screenshot_area(x, y) {
            return;
        }

        if let Some(ref mut drawing) = self.current_drawing {
            match drawing {
                DrawingElement::Rectangle { end, .. } => {
                    *end = (x, y);
                }
                DrawingElement::Circle {
                    center,
                    radius_x,
                    radius_y,
                    ..
                } => {
                    let dx = (x - center.0).abs();
                    let dy = (y - center.1).abs();
                    *radius_x = dx;
                    *radius_y = dy;
                }
                DrawingElement::Arrow { end, .. } => {
                    *end = (x, y);
                }
                DrawingElement::Pen { .. } => {
                    // 🔧 修复：画笔实时渲染，立即添加点并重绘
                    self.add_pen_point(x, y);
                }
                DrawingElement::Text { .. } => {
                    // 🚀 文本元素不需要在拖拽时更新
                }
            }
        }

        // 🔧 修复：其他绘图工具更新时标记需要重绘
        self.needs_redraw = true;
        self.render_cache_valid = false;
        // 🚀 更新绘图时失效当前元素缓存
        if let Some(current) = self.current_drawing.clone() {
            self.invalidate_element_cache(&current);
        }
    }
    // 新增：完成当前绘图
    fn finish_current_drawing(&mut self) {
        if let Some(drawing) = self.current_drawing.take() {
            println!(
                "🚀 完成绘图，元素类型: {:?}",
                match &drawing {
                    DrawingElement::Text { content, .. } => format!("Text('{}')", content),
                    DrawingElement::Rectangle { .. } => "Rectangle".to_string(),
                    DrawingElement::Circle { .. } => "Circle".to_string(),
                    DrawingElement::Arrow { .. } => "Arrow".to_string(),
                    DrawingElement::Pen { .. } => "Pen".to_string(),
                }
            );

            // 🚀 新增：检查元素是否满足最小尺寸要求
            if !self.is_element_large_enough(&drawing) {
                println!(
                    "🚀 元素太小，不保存: {:?}",
                    match &drawing {
                        DrawingElement::Rectangle { .. } => "Rectangle",
                        DrawingElement::Circle { .. } => "Circle",
                        DrawingElement::Arrow { .. } => "Arrow",
                        _ => "Other",
                    }
                );
                self.drawing_state = DrawingState::Idle;
                self.drawing_start_pos = None;
                self.pen_points.clear();
                self.needs_redraw = true;
                return;
            }

            // 🚀 保存状态到撤销栈（在添加新元素之前）
            self.save_state_for_undo();

            let new_index = self.drawing_elements.len();
            self.drawing_elements.push(drawing);

            println!(
                "🚀 绘图元素已添加到列表，索引: {}, 总数: {}",
                new_index,
                self.drawing_elements.len()
            );

            // 🚀 绘制完成后立即选择并显示手柄
            self.select_element(new_index);
        } else {
            println!("🚀 没有当前绘图元素需要完成");
        }
        self.drawing_state = DrawingState::Idle;
        self.drawing_start_pos = None;
        self.pen_points.clear();

        // 🔧 修复：完成绘图时标记需要重绘
        self.needs_redraw = true;
        self.render_cache_valid = false;
        // 🚀 完成绘图时失效绘图缓存
        self.invalidate_drawing_cache();
    }

    // 🚀 新增：检查元素是否满足最小尺寸要求
    fn is_element_large_enough(&self, element: &DrawingElement) -> bool {
        match element {
            DrawingElement::Rectangle { start, end, .. } => {
                let width = (end.0 - start.0).abs();
                let height = (end.1 - start.1).abs();
                width >= MIN_SAVE_SIZE && height >= MIN_SAVE_SIZE
            }
            DrawingElement::Circle {
                radius_x, radius_y, ..
            } => *radius_x >= MIN_SAVE_RADIUS && *radius_y >= MIN_SAVE_RADIUS,
            DrawingElement::Arrow { start, end, .. } => {
                let dx = end.0 - start.0;
                let dy = end.1 - start.1;
                let length = (dx * dx + dy * dy).sqrt();
                length >= MIN_ARROW_LENGTH
            }
            DrawingElement::Text { .. } => {
                // 文本元素总是保存，因为即使很小也有意义
                true
            }
            DrawingElement::Pen { points, .. } => {
                // 笔画元素总是保存，因为用户手绘的内容都有意义
                !points.is_empty()
            }
        }
    }

    // 新增：撤销操作
    fn undo_drawing(&mut self) {
        if !self.drawing_elements.is_empty() {
            self.drawing_elements.pop();
            println!("撤销了一个绘图元素，剩余: {}", self.drawing_elements.len());
        } else {
            println!("没有可撤销的绘图元素");
        }
    }

    // 新增：检查点是否在截图区域内
    fn is_point_in_screenshot_area(&self, x: f32, y: f32) -> bool {
        if let Some((min_x, min_y, max_x, max_y)) = self.current_box_coords {
            x >= min_x && x <= max_x && y >= min_y && y <= max_y
        } else {
            false
        }
    }

    // 🔧 实时画笔：确保每次添加点都立即渲染
    fn add_pen_point(&mut self, x: f32, y: f32) {
        if let Some(DrawingElement::Pen { points, .. }) = &mut self.current_drawing {
            // 直接添加所有点，不做任何过滤
            points.push((x, y));
            self.pen_points.push((x, y));

            // 🔧 实时渲染：立即标记重绘
            self.needs_redraw = true; // 立即重绘
            self.render_cache_valid = false; // 清除缓存
        }
    }

    // 🚀 生成绘图元素的缓存键
    fn generate_element_cache_key(&self, element: &DrawingElement) -> String {
        match element {
            DrawingElement::Rectangle {
                start,
                end,
                color,
                thickness,
            } => {
                format!(
                    "rect_{}_{}_{}_{}_{}_{}_{}_{}",
                    start.0, start.1, end.0, end.1, color[0], color[1], color[2], thickness
                )
            }
            DrawingElement::Circle {
                center,
                radius_x,
                radius_y,
                color,
                thickness,
            } => {
                format!(
                    "circle_{}_{}_{}_{}_{}_{}_{}_{}",
                    center.0, center.1, radius_x, radius_y, color[0], color[1], color[2], thickness
                )
            }
            DrawingElement::Arrow {
                start,
                end,
                color,
                thickness,
            } => {
                format!(
                    "arrow_{}_{}_{}_{}_{}_{}_{}_{}",
                    start.0, start.1, end.0, end.1, color[0], color[1], color[2], thickness
                )
            }
            DrawingElement::Pen {
                points,
                color,
                thickness,
            } => {
                // 对于画笔，使用点的哈希值
                let points_hash = points
                    .iter()
                    .map(|(x, y)| format!("{}_{}", x, y))
                    .collect::<Vec<_>>()
                    .join("_");
                format!(
                    "pen_{}_{}_{}_{}_{}",
                    points_hash, color[0], color[1], color[2], thickness
                )
            }
            DrawingElement::Text {
                position,
                content,
                color,
                font_size,
                ..
            } => {
                format!(
                    "text_{}_{}_{}_{}_{}_{}_{}",
                    position.0, position.1, content, color[0], color[1], color[2], font_size
                )
            }
        }
    }

    // 🚀 缓存的绘图元素顶点生成
    fn get_cached_element_vertices(&mut self, element: &DrawingElement) -> Vec<f32> {
        let cache_key = self.generate_element_cache_key(element);

        // 检查缓存是否有效
        if let Some(cached_vertices) = self.cached_drawing_vertices.get(&cache_key) {
            if *self.drawing_cache_valid.get(&cache_key).unwrap_or(&false) {
                return cached_vertices.clone();
            }
        }

        // 缓存无效或不存在，重新计算
        let mut vertices = Vec::new();
        self.add_element_vertices_uncached(element, &mut vertices);

        // 更新缓存
        self.cached_drawing_vertices
            .insert(cache_key.clone(), vertices.clone());
        self.drawing_cache_valid.insert(cache_key, true);

        vertices
    }

    // 🚀 失效绘图元素缓存
    fn invalidate_drawing_cache(&mut self) {
        self.drawing_cache_valid.clear();
    }

    // 🚀 失效特定元素的缓存
    fn invalidate_element_cache(&mut self, element: &DrawingElement) {
        let cache_key = self.generate_element_cache_key(element);
        self.drawing_cache_valid.insert(cache_key, false);
    }

    // 🚀 为绘图元素生成手柄
    fn generate_handles_for_element(
        &self,
        element: &DrawingElement,
        element_index: usize,
    ) -> Vec<Handle> {
        let mut handles = Vec::new();

        match element {
            DrawingElement::Rectangle { start, end, .. } => {
                // 矩形的8个调整手柄
                let min_x = start.0.min(end.0);
                let max_x = start.0.max(end.0);
                let min_y = start.1.min(end.1);
                let max_y = start.1.max(end.1);
                let center_x = (min_x + max_x) / 2.0;
                let center_y = (min_y + max_y) / 2.0;

                handles.push(Handle {
                    handle_type: HandleType::TopLeft,
                    position: (min_x, min_y),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::TopCenter,
                    position: (center_x, min_y),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::TopRight,
                    position: (max_x, min_y),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::MiddleLeft,
                    position: (min_x, center_y),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::MiddleRight,
                    position: (max_x, center_y),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::BottomLeft,
                    position: (min_x, max_y),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::BottomCenter,
                    position: (center_x, max_y),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::BottomRight,
                    position: (max_x, max_y),
                    size: self.handle_size,
                    element_index,
                });
                // 不再需要专门的移动手柄，点击元素内部即可拖动
            }
            DrawingElement::Circle {
                center,
                radius_x,
                radius_y,
                ..
            } => {
                // 🚀 椭圆的手柄放在包围矩形的边框上
                // 计算包围椭圆的矩形边界
                let left = center.0 - *radius_x;
                let right = center.0 + *radius_x;
                let top = center.1 - *radius_y;
                let bottom = center.1 + *radius_y;
                let center_x = (left + right) / 2.0;
                let center_y = (top + bottom) / 2.0;

                // 8个手柄放在矩形边框上（与矩形手柄相同的布局）
                handles.push(Handle {
                    handle_type: HandleType::TopLeft,
                    position: (left, top),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::TopCenter,
                    position: (center_x, top),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::TopRight,
                    position: (right, top),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::MiddleLeft,
                    position: (left, center_y),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::MiddleRight,
                    position: (right, center_y),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::BottomLeft,
                    position: (left, bottom),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::BottomCenter,
                    position: (center_x, bottom),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::BottomRight,
                    position: (right, bottom),
                    size: self.handle_size,
                    element_index,
                });
                // 不再需要专门的移动手柄，点击元素内部即可拖动
            }
            DrawingElement::Arrow { start, end, .. } => {
                // 箭头的2个调整手柄
                handles.push(Handle {
                    handle_type: HandleType::ArrowStart,
                    position: *start,
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::ArrowEnd,
                    position: *end,
                    size: self.handle_size,
                    element_index,
                });
                // 不再需要专门的移动手柄，点击元素内部即可拖动
            }
            DrawingElement::Pen { .. } => {
                // 🚀 画笔不生成手柄，画完后直接固化，不可编辑
                // 这符合画笔工具的使用习惯：一次性绘制，不可修改
            }
            DrawingElement::Text {
                position,
                is_editing,
                content,
                font_size,
                ..
            } => {
                // 🚀 修复：输入文字时也显示手柄
                // 🚀 为文本添加四个角的调整手柄（类似矩形）
                let lines: Vec<&str> = content.split('\n').collect();
                let line_count = lines.len() as f32;
                let max_line_width = lines
                    .iter()
                    .map(|line| line.len() as f32 * font_size * 0.6)
                    .fold(0.0, f32::max);

                let text_width = max_line_width.max(100.0);
                let text_height = font_size * 1.2 * line_count;

                // 🚀 添加padding到手柄位置计算
                let padding = 8.0; // 与边框padding保持一致
                let left = position.0 - padding;
                let top = position.1 - padding;
                let right = left + text_width + padding * 2.0;
                let bottom = top + text_height + padding * 2.0;
                let center_x = (left + right) / 2.0;
                let center_y = (top + bottom) / 2.0;

                // 四个角的调整手柄
                handles.push(Handle {
                    handle_type: HandleType::TopLeft,
                    position: (left, top),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::TopRight,
                    position: (right, top),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::BottomLeft,
                    position: (left, bottom),
                    size: self.handle_size,
                    element_index,
                });
                handles.push(Handle {
                    handle_type: HandleType::BottomRight,
                    position: (right, bottom),
                    size: self.handle_size,
                    element_index,
                });

                // 🚀 移除旋转手柄
            }
        }

        handles
    }

    // 🚀 检测点击是否在绘图元素上
    fn hit_test_element(&self, pos: (f32, f32), element: &DrawingElement) -> bool {
        match element {
            DrawingElement::Rectangle { start, end, .. } => {
                let min_x = start.0.min(end.0);
                let max_x = start.0.max(end.0);
                let min_y = start.1.min(end.1);
                let max_y = start.1.max(end.1);
                pos.0 >= min_x && pos.0 <= max_x && pos.1 >= min_y && pos.1 <= max_y
            }
            DrawingElement::Circle {
                center,
                radius_x,
                radius_y,
                ..
            } => {
                // 椭圆碰撞检测：使用椭圆方程，防止除零
                if *radius_x <= 0.0 || *radius_y <= 0.0 {
                    return false; // 无效椭圆
                }
                let dx = pos.0 - center.0;
                let dy = pos.1 - center.1;
                let normalized_x = dx / radius_x;
                let normalized_y = dy / radius_y;
                (normalized_x * normalized_x + normalized_y * normalized_y) <= 1.0
            }
            DrawingElement::Arrow { start, end, .. } => {
                // 简化的线段碰撞检测
                let threshold = 10.0;
                let dx = end.0 - start.0;
                let dy = end.1 - start.1;
                let length = (dx * dx + dy * dy).sqrt();
                if length == 0.0 {
                    return false;
                }

                let t = ((pos.0 - start.0) * dx + (pos.1 - start.1) * dy) / (length * length);
                let t = t.clamp(0.0, 1.0);
                let closest_x = start.0 + t * dx;
                let closest_y = start.1 + t * dy;
                let dist = ((pos.0 - closest_x).powi(2) + (pos.1 - closest_y).powi(2)).sqrt();
                dist <= threshold
            }
            DrawingElement::Pen { .. } => {
                // 🚀 画笔不参与交互，画完后固化，不可选中或移动
                false
            }
            DrawingElement::Text {
                position,
                content,
                font_size,
                ..
            } => {
                // 🚀 文本碰撞检测：只有非空文本才能被点击
                if content.is_empty() {
                    return false;
                }

                // 🚀 改进：支持多行文本的碰撞检测
                let lines: Vec<&str> = content.split('\n').collect();
                let line_count = lines.len() as f32;
                let max_line_width = lines
                    .iter()
                    .map(|line| line.len() as f32 * font_size * 0.6)
                    .fold(0.0, f32::max);

                let text_width = max_line_width.max(100.0);
                let text_height = font_size * 1.2 * line_count;

                pos.0 >= position.0
                    && pos.0 <= position.0 + text_width
                    && pos.1 >= position.1
                    && pos.1 <= position.1 + text_height
            }
        }
    }

    // 🚀 检测点击是否在手柄上
    fn hit_test_handle(&self, pos: (f32, f32), handle: &Handle) -> bool {
        let dx = pos.0 - handle.position.0;
        let dy = pos.1 - handle.position.1;
        let distance = (dx * dx + dy * dy).sqrt();
        distance <= handle.size / 2.0
    }

    // 🚀 选择绘图元素
    fn select_element(&mut self, element_index: usize) {
        if element_index < self.drawing_elements.len() {
            let element = self.drawing_elements[element_index].clone();
            let handles = self.generate_handles_for_element(&element, element_index);

            // 🚀 根据选中的元素类型更新工具栏状态
            self.update_tool_from_element(&element);

            self.selected_element = Some(SelectedElement {
                index: element_index,
                handles,
                is_moving: false,
                move_offset: (0.0, 0.0),
            });
            self.drawing_state = DrawingState::Editing;
            self.needs_redraw = true;
        }
    }

    // 🚀 取消选择
    fn deselect_element(&mut self) {
        self.selected_element = None;
        self.hovered_handle = None;
        self.dragging_handle = None;
        self.drawing_state = DrawingState::Idle;
        self.needs_redraw = true;
    }

    // 🚀 处理手柄拖拽
    fn handle_drag(&mut self, pos: (f32, f32)) {
        if let Some(mut dragging_handle) = self.dragging_handle.clone() {
            if let Some(selected_index) = self.selected_element.as_ref().map(|s| s.index) {
                if selected_index < self.drawing_elements.len() {
                    let element = &mut self.drawing_elements[selected_index];

                    // 🚀 对于矩形，检测是否需要动态切换手柄类型
                    let (new_handle_type, should_update_handles) =
                        if let DrawingElement::Rectangle { start, end, .. } = element {
                            let new_handle_type = Self::get_dynamic_handle_type_static(
                                &dragging_handle,
                                pos,
                                *start,
                                *end,
                            );
                            let should_update = new_handle_type != dragging_handle.handle_type;
                            (new_handle_type, should_update)
                        } else {
                            (dragging_handle.handle_type, false)
                        };

                    if should_update_handles {
                        dragging_handle.handle_type = new_handle_type;
                        // 更新当前拖拽的手柄类型
                        self.dragging_handle = Some(dragging_handle.clone());
                    }

                    match dragging_handle.handle_type {
                        HandleType::TopLeft => {
                            if let DrawingElement::Rectangle { start, end, .. } = element {
                                start.0 = pos.0;
                                start.1 = pos.1;
                                // 🚀 规范化矩形坐标，防止坐标混乱
                                Self::normalize_rectangle(start, end);
                            } else if let DrawingElement::Circle {
                                center,
                                radius_x,
                                radius_y,
                                ..
                            } = element
                            {
                                // 🚀 椭圆角手柄：同时调整水平和垂直半径，防止负值
                                let new_radius_x = (pos.0 - center.0).abs().max(MIN_ELLIPSE_RADIUS);
                                let new_radius_y = (pos.1 - center.1).abs().max(MIN_ELLIPSE_RADIUS);
                                *radius_x = new_radius_x;
                                *radius_y = new_radius_y;
                            } else if let DrawingElement::Text {
                                position,
                                font_size,
                                content,
                                ..
                            } = element
                            {
                                // 🚀 文本左上角手柄：缩放文本框和字体大小
                                Self::scale_text_element(
                                    position,
                                    font_size,
                                    content,
                                    pos,
                                    HandleType::TopLeft,
                                );
                            }
                        }
                        HandleType::TopRight => {
                            if let DrawingElement::Rectangle { start, end, .. } = element {
                                end.0 = pos.0;
                                start.1 = pos.1;
                                // 🚀 规范化矩形坐标，防止坐标混乱
                                Self::normalize_rectangle(start, end);
                            } else if let DrawingElement::Circle {
                                center,
                                radius_x,
                                radius_y,
                                ..
                            } = element
                            {
                                // 🚀 椭圆角手柄：同时调整水平和垂直半径，防止负值
                                let new_radius_x = (pos.0 - center.0).abs().max(MIN_ELLIPSE_RADIUS);
                                let new_radius_y = (pos.1 - center.1).abs().max(MIN_ELLIPSE_RADIUS);
                                *radius_x = new_radius_x;
                                *radius_y = new_radius_y;
                            } else if let DrawingElement::Text {
                                position,
                                font_size,
                                content,
                                ..
                            } = element
                            {
                                // 🚀 文本右上角手柄：缩放文本框和字体大小
                                Self::scale_text_element(
                                    position,
                                    font_size,
                                    content,
                                    pos,
                                    HandleType::TopRight,
                                );
                            }
                        }
                        HandleType::BottomLeft => {
                            if let DrawingElement::Rectangle { start, end, .. } = element {
                                start.0 = pos.0;
                                end.1 = pos.1;
                                // 🚀 规范化矩形坐标，防止坐标混乱
                                Self::normalize_rectangle(start, end);
                            } else if let DrawingElement::Circle {
                                center,
                                radius_x,
                                radius_y,
                                ..
                            } = element
                            {
                                // 🚀 椭圆角手柄：同时调整水平和垂直半径，防止负值
                                let new_radius_x = (pos.0 - center.0).abs().max(MIN_ELLIPSE_RADIUS);
                                let new_radius_y = (pos.1 - center.1).abs().max(MIN_ELLIPSE_RADIUS);
                                *radius_x = new_radius_x;
                                *radius_y = new_radius_y;
                            } else if let DrawingElement::Text {
                                position,
                                font_size,
                                content,
                                ..
                            } = element
                            {
                                // 🚀 文本左下角手柄：缩放文本框和字体大小
                                Self::scale_text_element(
                                    position,
                                    font_size,
                                    content,
                                    pos,
                                    HandleType::BottomLeft,
                                );
                            }
                        }
                        HandleType::BottomRight => {
                            if let DrawingElement::Rectangle { start, end, .. } = element {
                                end.0 = pos.0;
                                end.1 = pos.1;
                                // 🚀 规范化矩形坐标，防止坐标混乱
                                Self::normalize_rectangle(start, end);
                            } else if let DrawingElement::Circle {
                                center,
                                radius_x,
                                radius_y,
                                ..
                            } = element
                            {
                                // 🚀 椭圆角手柄：同时调整水平和垂直半径，防止负值
                                let new_radius_x = (pos.0 - center.0).abs().max(MIN_ELLIPSE_RADIUS);
                                let new_radius_y = (pos.1 - center.1).abs().max(MIN_ELLIPSE_RADIUS);
                                *radius_x = new_radius_x;
                                *radius_y = new_radius_y;
                            } else if let DrawingElement::Text {
                                position,
                                font_size,
                                content,
                                ..
                            } = element
                            {
                                // 🚀 文本右下角手柄：缩放文本框和字体大小
                                Self::scale_text_element(
                                    position,
                                    font_size,
                                    content,
                                    pos,
                                    HandleType::BottomRight,
                                );
                            }
                        }
                        HandleType::TopCenter => {
                            if let DrawingElement::Rectangle { start, end, .. } = element {
                                start.1 = pos.1;
                                // 🚀 规范化矩形坐标，防止坐标混乱
                                Self::normalize_rectangle(start, end);
                            } else if let DrawingElement::Circle {
                                center, radius_y, ..
                            } = element
                            {
                                // 🚀 椭圆上中点手柄：只调整垂直半径，形成椭圆，防止负值
                                *radius_y = (pos.1 - center.1).abs().max(MIN_ELLIPSE_RADIUS);
                            }
                        }
                        HandleType::BottomCenter => {
                            if let DrawingElement::Rectangle { start, end, .. } = element {
                                end.1 = pos.1;
                                // 🚀 规范化矩形坐标，防止坐标混乱
                                Self::normalize_rectangle(start, end);
                            } else if let DrawingElement::Circle {
                                center, radius_y, ..
                            } = element
                            {
                                // 🚀 椭圆下中点手柄：只调整垂直半径，形成椭圆，防止负值
                                *radius_y = (pos.1 - center.1).abs().max(MIN_ELLIPSE_RADIUS);
                            }
                        }
                        HandleType::MiddleLeft => {
                            if let DrawingElement::Rectangle { start, end, .. } = element {
                                start.0 = pos.0;
                                // 🚀 规范化矩形坐标，防止坐标混乱
                                Self::normalize_rectangle(start, end);
                            } else if let DrawingElement::Circle {
                                center, radius_x, ..
                            } = element
                            {
                                // 🚀 椭圆左中点手柄：只调整水平半径，形成椭圆，防止负值
                                *radius_x = (pos.0 - center.0).abs().max(MIN_ELLIPSE_RADIUS);
                            }
                        }
                        HandleType::MiddleRight => {
                            if let DrawingElement::Rectangle { start, end, .. } = element {
                                end.0 = pos.0;
                                // 🚀 规范化矩形坐标，防止坐标混乱
                                Self::normalize_rectangle(start, end);
                            } else if let DrawingElement::Circle {
                                center, radius_x, ..
                            } = element
                            {
                                // 🚀 椭圆右中点手柄：只调整水平半径，形成椭圆，防止负值
                                *radius_x = (pos.0 - center.0).abs().max(MIN_ELLIPSE_RADIUS);
                            }
                        }
                        HandleType::ArrowStart => {
                            if let DrawingElement::Arrow { start, .. } = element {
                                start.0 = pos.0;
                                start.1 = pos.1;
                            }
                        }
                        HandleType::ArrowEnd => {
                            if let DrawingElement::Arrow { end, .. } = element {
                                end.0 = pos.0;
                                end.1 = pos.1;
                            }
                        }
                        HandleType::Move => {
                            // Move手柄已移除，这个分支不应该被执行
                        }
                        HandleType::Rotate => {
                            // 🚀 旋转手柄已移除，这个分支不应该被执行
                        }
                    }

                    // 更新手柄位置和缓存
                    let element_clone = element.clone();
                    let selected_index = selected_index;
                    self.invalidate_element_cache(&element_clone);
                    let new_handles =
                        self.generate_handles_for_element(&element_clone, selected_index);
                    if let Some(ref mut selected) = self.selected_element {
                        selected.handles = new_handles;
                    }
                    self.needs_redraw = true;
                }
            }
        }
    }

    // 🚀 移动绘图元素（静态版本）
    fn move_element_static(element: &mut DrawingElement, offset: (f32, f32)) {
        match element {
            DrawingElement::Rectangle { start, end, .. } => {
                start.0 += offset.0;
                start.1 += offset.1;
                end.0 += offset.0;
                end.1 += offset.1;
            }
            DrawingElement::Circle { center, .. } => {
                center.0 += offset.0;
                center.1 += offset.1;
            }
            DrawingElement::Arrow { start, end, .. } => {
                start.0 += offset.0;
                start.1 += offset.1;
                end.0 += offset.0;
                end.1 += offset.1;
            }
            DrawingElement::Pen { points, .. } => {
                for point in points {
                    point.0 += offset.0;
                    point.1 += offset.1;
                }
            }
            DrawingElement::Text { position, .. } => {
                position.0 += offset.0;
                position.1 += offset.1;
            }
        }
    }

    // 新增：添加单个元素的顶点数据（无缓存版本）
    fn add_element_vertices_uncached(&self, element: &DrawingElement, vertices: &mut Vec<f32>) {
        // 🚀 使用缓存优化的几何图形计算
        let screen_width = self.size.width as f32;
        let screen_height = self.size.height as f32;

        match element {
            DrawingElement::Rectangle {
                start,
                end,
                color,
                thickness,
            } => {
                // 🚀 使用缓存的矩形顶点计算
                let x1 = (start.0 / screen_width) * 2.0 - 1.0;
                let y1 = 1.0 - (start.1 / screen_height) * 2.0;
                let x2 = (end.0 / screen_width) * 2.0 - 1.0;
                let y2 = 1.0 - (end.1 / screen_height) * 2.0;

                // 预计算的矩形边线
                let lines = [
                    (x1, y1, x2, y1), // 上边
                    (x2, y1, x2, y2), // 右边
                    (x2, y2, x1, y2), // 下边
                    (x1, y2, x1, y1), // 左边
                ];

                for (sx, sy, ex, ey) in lines.iter() {
                    vertices.extend_from_slice(&[
                        *sx, *sy, color[0], color[1], color[2], 1.0, *thickness, *ex, *ey,
                        color[0], color[1], color[2], 1.0, *thickness,
                    ]);
                }
            }
            DrawingElement::Circle {
                center,
                radius_x,
                radius_y,
                color,
                thickness,
            } => {
                // 🚀 使用缓存的椭圆顶点计算，减少三角函数调用
                const SEGMENTS: i32 = 32;
                let cx = (center.0 / screen_width) * 2.0 - 1.0;
                let cy = 1.0 - (center.1 / screen_height) * 2.0;
                let r_x = radius_x / screen_width * 2.0;
                let r_y = radius_y / screen_height * 2.0;

                // 预计算角度增量
                const ANGLE_STEP: f32 = 2.0 * std::f32::consts::PI / SEGMENTS as f32;

                for i in 0..SEGMENTS {
                    let angle1 = (i as f32) * ANGLE_STEP;
                    let angle2 = ((i + 1) as f32) * ANGLE_STEP;

                    let x1 = cx + r_x * angle1.cos();
                    let y1 = cy + r_y * angle1.sin();
                    let x2 = cx + r_x * angle2.cos();
                    let y2 = cy + r_y * angle2.sin();

                    vertices.extend_from_slice(&[
                        x1, y1, color[0], color[1], color[2], 1.0, *thickness, x2, y2, color[0],
                        color[1], color[2], 1.0, *thickness,
                    ]);
                }
            }
            DrawingElement::Arrow {
                start,
                end,
                color,
                thickness,
            } => {
                // 🚀 使用缓存的箭头顶点计算，减少重复的向量运算
                // 主线
                let x1 = (start.0 / screen_width) * 2.0 - 1.0;
                let y1 = 1.0 - (start.1 / screen_height) * 2.0;
                let x2 = (end.0 / screen_width) * 2.0 - 1.0;
                let y2 = 1.0 - (end.1 / screen_height) * 2.0;

                vertices.extend_from_slice(&[
                    x1, y1, color[0], color[1], color[2], 1.0, *thickness, x2, y2, color[0],
                    color[1], color[2], 1.0, *thickness,
                ]);

                // 🚀 优化的箭头计算
                let dx = end.0 - start.0;
                let dy = end.1 - start.1;
                let len_squared = dx * dx + dy * dy;

                if len_squared > 1.0 {
                    // 避免除零，使用平方长度比较
                    let len = len_squared.sqrt();
                    let ux = dx / len;
                    let uy = dy / len;

                    // 预定义箭头参数
                    const ARROW_LEN: f32 = 15.0;
                    const ARROW_WIDTH: f32 = 8.0;

                    let p1_x = end.0 - ARROW_LEN * ux + ARROW_WIDTH * uy;
                    let p1_y = end.1 - ARROW_LEN * uy - ARROW_WIDTH * ux;
                    let p2_x = end.0 - ARROW_LEN * ux - ARROW_WIDTH * uy;
                    let p2_y = end.1 - ARROW_LEN * uy + ARROW_WIDTH * ux;

                    let ap1_x = (p1_x / screen_width) * 2.0 - 1.0;
                    let ap1_y = 1.0 - (p1_y / screen_height) * 2.0;
                    let ap2_x = (p2_x / screen_width) * 2.0 - 1.0;
                    let ap2_y = 1.0 - (p2_y / screen_height) * 2.0;

                    vertices.extend_from_slice(&[
                        x2, y2, color[0], color[1], color[2], 1.0, *thickness, ap1_x, ap1_y,
                        color[0], color[1], color[2], 1.0, *thickness, x2, y2, color[0], color[1],
                        color[2], 1.0, *thickness, ap2_x, ap2_y, color[0], color[1], color[2], 1.0,
                        *thickness,
                    ]);
                }
            }
            DrawingElement::Pen {
                points,
                color,
                thickness,
            } => {
                // 去掉步长优化，直接渲染所有点以获得最佳质量
                for i in 0..points.len().saturating_sub(1) {
                    let x1 = (points[i].0 / screen_width) * 2.0 - 1.0;
                    let y1 = 1.0 - (points[i].1 / screen_height) * 2.0;
                    let x2 = (points[i + 1].0 / screen_width) * 2.0 - 1.0;
                    let y2 = 1.0 - (points[i + 1].1 / screen_height) * 2.0;

                    vertices.extend_from_slice(&[
                        x1, y1, color[0], color[1], color[2], 1.0, *thickness, x2, y2, color[0],
                        color[1], color[2], 1.0, *thickness,
                    ]);
                }
            }
            DrawingElement::Text { .. } => {
                // 🚀 文本渲染现在通过 wgpu-text 处理，不再添加到顶点缓冲区
                // 文本将在单独的渲染通道中处理
                // 🚀 修复：已完成的文本不需要边框，只有正在编辑的文本才需要边框
                // 边框渲染在其他地方处理
            }
        }
    }

    // 🚀 缓存优化的元素顶点添加函数
    fn add_element_vertices(&mut self, element: &DrawingElement, vertices: &mut Vec<f32>) {
        let cached_vertices = self.get_cached_element_vertices(element);
        vertices.extend_from_slice(&cached_vertices);
    }

    // 🚀 准备并渲染文本元素
    fn render_text_elements(&mut self, view: &wgpu::TextureView) {
        use glyphon::{Color, TextArea, TextBounds};

        let mut text_areas = Vec::new();
        let mut buffers = Vec::new(); // 存储所有 buffer 以保持生命周期

        // 只在调试时打印，减少日志噪音
        if self.text_input_active {
            println!(
                "🚀 渲染文本元素 - text_input_active: {}, current_drawing: {:?}",
                self.text_input_active,
                self.current_drawing.as_ref().map(|d| match d {
                    DrawingElement::Text { is_editing, .. } =>
                        format!("Text(editing: {})", is_editing),
                    _ => "Other".to_string(),
                })
            );
        }

        // 收集已完成的文本元素
        for element in &self.drawing_elements {
            if let DrawingElement::Text {
                position,
                content,
                color,
                font_size,
                ..
            } = element
            {
                if !content.is_empty() {
                    let buffer = self.text_renderer.create_buffer(
                        content,
                        *font_size,
                        content.len() as f32 * font_size * 0.6, // 估算宽度
                        *font_size * 1.2,                       // 行高
                    );

                    buffers.push(buffer);
                }
            }
        }

        // 创建已完成文本元素的 TextArea
        let mut buffer_index = 0;
        for element in &self.drawing_elements {
            if let DrawingElement::Text {
                position,
                content,
                color,
                font_size,
                ..
            } = element
            {
                if !content.is_empty() && buffer_index < buffers.len() {
                    let text_area = TextArea {
                        buffer: &buffers[buffer_index],
                        left: position.0,
                        top: position.1,
                        scale: 1.0,
                        bounds: TextBounds {
                            left: position.0 as i32,
                            top: position.1 as i32,
                            right: (position.0 + content.len() as f32 * font_size * 0.6) as i32,
                            bottom: (position.1 + font_size * 1.2) as i32,
                        },
                        default_color: Color::rgba(255, 255, 255, 255), // 强制使用白色
                        custom_glyphs: &[],
                    };
                    text_areas.push(text_area);
                    buffer_index += 1;
                }
            }
        }

        // 🚀 简单测试：总是在屏幕左上角显示固定文字
        if self.text_input_active {
            let display_text = "TEST TEXT 测试文字 123";
            let test_position = (100.0, 100.0); // 固定位置

            println!("🚀 测试文本渲染: '{}'", display_text);

            // 只在文本内容改变时重新创建缓冲区
            if self.cached_text_content != display_text {
                println!("🚀 创建测试文字缓冲区");

                let buffer = self.text_renderer.create_buffer(
                    display_text,
                    64.0,  // 使用很大的字体
                    800.0, // 足够的宽度
                    80.0,  // 足够的高度
                );

                self.text_buffer_cache = Some(buffer);
                self.cached_text_content = display_text.to_string();
            }

            // 使用缓存的缓冲区创建 TextArea
            if let Some(ref buffer) = self.text_buffer_cache {
                println!(
                    "🚀 创建测试 TextArea: 位置=({}, {})",
                    test_position.0, test_position.1
                );

                let text_area = TextArea {
                    buffer,
                    left: test_position.0,
                    top: test_position.1,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: test_position.0 as i32,
                        top: test_position.1 as i32,
                        right: (test_position.0 + 800.0) as i32,
                        bottom: (test_position.1 + 80.0) as i32,
                    },
                    default_color: Color::rgba(255, 0, 0, 255), // 使用红色
                    custom_glyphs: &[],
                };
                text_areas.push(text_area);
            }
        } else {
            // 清除缓存
            self.text_buffer_cache = None;
            self.cached_text_content.clear();
        }

        // 渲染文本
        if !text_areas.is_empty() {
            if let Err(e) =
                self.text_renderer
                    .prepare(&self.device, &self.queue, text_areas.into_iter())
            {
                eprintln!("Failed to prepare text: {:?}", e);
            } else {
                // 创建新的渲染通道用于文本渲染
                let mut text_encoder =
                    self.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Text Render Encoder"),
                        });

                {
                    let mut text_render_pass =
                        text_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Text Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load, // 保持之前的内容
                                    store: wgpu::StoreOp::Store,
                                },
                                depth_slice: None,
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });

                    if let Err(e) = self.text_renderer.render(&mut text_render_pass) {
                        eprintln!("Failed to render text: {:?}", e);
                    }
                }

                self.queue.submit(std::iter::once(text_encoder.finish()));
            }
        }
    }

    // 🚀 渲染已完成的文本
    fn render_completed_text<'a>(&'a mut self, render_pass: &mut wgpu::RenderPass<'a>) {
        use glyphon::{Color, TextArea, TextBounds};

        // 🚀 修复策略：只渲染已完成的文本，正在编辑的文本通过其他方式显示
        println!(
            "🚀 开始渲染文本，drawing_elements数量: {}",
            self.drawing_elements.len()
        );

        let mut completed_text_areas = Vec::new();
        let mut completed_buffers = Vec::new();

        // 收集已完成的文本元素
        for (index, element) in self.drawing_elements.iter().enumerate() {
            if let DrawingElement::Text {
                content, font_size, ..
            } = element
            {
                println!(
                    "🚀 检查文本元素 {}: '{}' (长度: {})",
                    index,
                    content,
                    content.len()
                );
                if !content.is_empty() {
                    let buffer = self.text_renderer.create_buffer(
                        content,
                        *font_size,
                        content.len() as f32 * font_size * 0.6,
                        *font_size * 1.2,
                    );
                    completed_buffers.push(buffer);
                    println!("🚀 为文本元素 {} 创建了buffer", index);
                }
            }
        }

        // 创建已完成文本的 TextArea
        let mut buffer_index = 0;
        for element in &self.drawing_elements {
            if let DrawingElement::Text {
                position,
                content,
                font_size,
                ..
            } = element
            {
                if !content.is_empty() && buffer_index < completed_buffers.len() {
                    // 🚀 修复：计算多行文本的边界
                    let lines: Vec<&str> = content.split('\n').collect();
                    let line_count = lines.len() as f32;
                    let max_line_width = lines
                        .iter()
                        .map(|line| line.len() as f32 * font_size * 0.6)
                        .fold(0.0, f32::max);

                    let text_width = max_line_width.max(100.0);
                    let text_height = font_size * 1.2 * line_count;

                    let text_area = TextArea {
                        buffer: &completed_buffers[buffer_index],
                        left: position.0,
                        top: position.1,
                        scale: 1.0,
                        bounds: TextBounds {
                            left: position.0 as i32,
                            top: position.1 as i32,
                            right: (position.0 + text_width) as i32,
                            bottom: (position.1 + text_height) as i32,
                        },
                        default_color: Color::rgba(255, 0, 0, 255), // 红色文字
                        custom_glyphs: &[],
                    };
                    completed_text_areas.push(text_area);
                    buffer_index += 1;
                }
            }
        }

        // 渲染已完成的文本
        if !completed_text_areas.is_empty() {
            if let Err(e) = self.text_renderer.prepare(
                &self.device,
                &self.queue,
                completed_text_areas.into_iter(),
            ) {
                eprintln!("Failed to prepare completed text: {:?}", e);
            } else if let Err(e) = self.text_renderer.render(render_pass) {
                eprintln!("Failed to render completed text: {:?}", e);
            } else {
                println!(
                    "🚀 成功渲染已完成的文本元素数量: {}",
                    completed_buffers.len()
                );
            }
        }

        // 🚀 这个函数只渲染已完成的文本
    }

    // 🚀 渲染所有文本（已完成的文本 + 正在编辑的文本）
    fn render_all_text_with_editing<'a>(&'a mut self, render_pass: &mut wgpu::RenderPass<'a>) {
        use glyphon::{Color, TextArea, TextBounds};

        println!("🚀 开始渲染所有文本（包括正在编辑的）");

        let mut all_text_areas = Vec::new();
        let mut all_buffers = Vec::new();

        // 1. 首先为已完成的文本创建 buffers
        for (index, element) in self.drawing_elements.iter().enumerate() {
            if let DrawingElement::Text {
                content, font_size, ..
            } = element
            {
                if !content.is_empty() {
                    println!("🚀 为已完成文本 {} 创建buffer: '{}'", index, content);
                    let buffer = self.text_renderer.create_buffer(
                        content,
                        *font_size,
                        content.len() as f32 * font_size * 0.6,
                        *font_size * 1.2,
                    );
                    all_buffers.push(buffer);
                }
            }
        }

        // 2. 为正在编辑的文本创建 buffer
        if let Some(DrawingElement::Text {
            is_editing,
            font_size,
            ..
        }) = &self.current_drawing
        {
            if *is_editing {
                let display_text = if self.current_text_input.is_empty() {
                    "输入文字...".to_string()
                } else {
                    // 🚀 修复：使用正确的光标位置显示（与其他地方保持一致）
                    let mut chars: Vec<char> = self.current_text_input.chars().collect();
                    let cursor_pos = self.text_cursor_position.min(chars.len());
                    chars.insert(cursor_pos, '|');
                    let result = chars.into_iter().collect::<String>();
                    println!(
                        "🚀 Buffer创建文本（带光标）: {:?}, 光标位置: {}",
                        result, cursor_pos
                    );
                    result
                };

                println!("🚀 为正在编辑的文本创建buffer: '{}'", display_text);

                let editing_buffer = self.text_renderer.create_buffer(
                    &display_text,
                    *font_size,
                    display_text.len() as f32 * font_size * 0.6,
                    font_size * 1.2,
                );
                all_buffers.push(editing_buffer);
            }
        }

        // 3. 创建 TextAreas
        let mut buffer_index = 0;

        // 为已完成的文本创建 TextAreas
        for element in &self.drawing_elements {
            if let DrawingElement::Text {
                position,
                content,
                font_size,
                ..
            } = element
            {
                if !content.is_empty() && buffer_index < all_buffers.len() {
                    // 🚀 修复：计算多行文本的边界
                    let lines: Vec<&str> = content.split('\n').collect();
                    let line_count = lines.len() as f32;
                    let max_line_width = lines
                        .iter()
                        .map(|line| line.len() as f32 * font_size * 0.6)
                        .fold(0.0, f32::max);

                    let text_width = max_line_width.max(100.0);
                    let text_height = font_size * 1.2 * line_count;

                    let text_area = TextArea {
                        buffer: &all_buffers[buffer_index],
                        left: position.0,
                        top: position.1,
                        scale: 1.0,
                        bounds: TextBounds {
                            left: position.0 as i32,
                            top: position.1 as i32,
                            right: (position.0 + text_width) as i32,
                            bottom: (position.1 + text_height) as i32,
                        },
                        default_color: Color::rgba(255, 0, 0, 255), // 红色文字
                        custom_glyphs: &[],
                    };
                    all_text_areas.push(text_area);
                    buffer_index += 1;
                }
            }
        }

        // 为正在编辑的文本创建 TextArea
        if let Some(DrawingElement::Text {
            position,
            is_editing,
            font_size,
            ..
        }) = &self.current_drawing
        {
            if *is_editing && buffer_index < all_buffers.len() {
                let display_text = if self.current_text_input.is_empty() {
                    "输入文字...".to_string()
                } else {
                    // 🚀 修复：正确处理光标位置和字符索引
                    let mut chars: Vec<char> = self.current_text_input.chars().collect();
                    let cursor_pos = self.text_cursor_position.min(chars.len());
                    chars.insert(cursor_pos, '|');
                    let result = chars.into_iter().collect::<String>();
                    println!(
                        "🚀 显示文本（带光标）: {:?}, 光标位置: {}",
                        result, cursor_pos
                    );
                    result
                };

                // 🚀 修复：计算多行文本的边界
                let lines: Vec<&str> = display_text.split('\n').collect();
                let line_count = lines.len() as f32;
                let max_line_width = lines
                    .iter()
                    .map(|line| line.len() as f32 * font_size * 0.6)
                    .fold(0.0, f32::max);

                let text_width = max_line_width.max(100.0);
                let text_height = font_size * 1.2 * line_count;

                let editing_text_area = TextArea {
                    buffer: &all_buffers[buffer_index],
                    left: position.0,
                    top: position.1,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: position.0 as i32,
                        top: position.1 as i32,
                        right: (position.0 + text_width) as i32,
                        bottom: (position.1 + text_height) as i32,
                    },
                    default_color: Color::rgba(255, 255, 0, 255), // 黄色文字表示正在编辑
                    custom_glyphs: &[],
                };
                all_text_areas.push(editing_text_area);
            }
        }

        // 3. 一次性渲染所有文本
        if !all_text_areas.is_empty() {
            if let Err(e) =
                self.text_renderer
                    .prepare(&self.device, &self.queue, all_text_areas.into_iter())
            {
                eprintln!("Failed to prepare all text: {:?}", e);
            } else if let Err(e) = self.text_renderer.render(render_pass) {
                eprintln!("Failed to render all text: {:?}", e);
            } else {
                println!("🚀 成功渲染所有文本元素数量: {}", all_buffers.len());
            }
        }
    }

    // 🚀 渲染文本外框
    fn render_text_border(&self, vertices: &mut Vec<f32>) {
        if let Some(DrawingElement::Text {
            position,
            is_editing,
            font_size,
            ..
        }) = &self.current_drawing
        {
            if *is_editing {
                // 🚀 使用实际的用户输入内容和字体大小来计算边框
                let display_text = if self.current_text_input.is_empty() {
                    "输入文字...".to_string()
                } else {
                    // 🚀 修复：正确处理光标位置和字符索引
                    let mut chars: Vec<char> = self.current_text_input.chars().collect();
                    let cursor_pos = self.text_cursor_position.min(chars.len());
                    chars.insert(cursor_pos, '|');
                    let result = chars.into_iter().collect::<String>();
                    println!(
                        "🚀 边框文本（带光标）: {:?}, 光标位置: {}",
                        result, cursor_pos
                    );
                    result
                };

                // 🚀 修复：动态计算多行文本的宽度和高度
                let lines: Vec<&str> = display_text.split('\n').collect();
                let line_count = lines.len() as f32;
                let max_line_width = lines
                    .iter()
                    .map(|line| line.len() as f32 * font_size * 0.6)
                    .fold(0.0, f32::max);

                let text_width = max_line_width.max(100.0); // 最小宽度100像素
                let text_height = font_size * 1.2 * line_count; // 高度 = 行高 × 行数

                // 🚀 修复：使用黑色虚线边框代替青色实线边框
                self.add_dashed_text_border(*position, text_width, text_height, vertices);

                println!(
                    "🚀 添加文本边框: 位置=({}, {}) 大小={}x{}",
                    position.0, position.1, text_width, text_height
                );
            }
        }
    }

    // 🔧 GPU优化：智能鼠标位置更新，减少不必要的重绘
    fn update_mouse_position(&mut self, x: f32, y: f32) {
        self.mouse_position = Some((x, y));
        let old_hovered = self.hovered_button;
        self.hovered_button = None;

        // 🔧 移除不必要的频率限制，保持实时响应

        if self.show_toolbar {
            for (i, button) in self.toolbar_buttons.iter().enumerate() {
                let (btn_x, btn_y, btn_w, btn_h) = button.rect;
                if x >= btn_x && x <= btn_x + btn_w && y >= btn_y && y <= btn_y + btn_h {
                    self.hovered_button = Some(i);
                    break;
                }
            }
        }

        // 🔧 GPU优化：只在悬停状态真正改变时标记需要重绘
        if old_hovered != self.hovered_button {
            self.needs_redraw = true;
            self.render_cache_valid = false;
            self.update_uniforms();
        }
    }

    // 使用wgpu 26最新的纹理创建和写入方法，支持现代GPU优化
    fn load_svg_texture(&self, svg_data: &str, size: u32) -> wgpu::Texture {
        // 使用最新的usvg选项配置，启用现代渲染特性
        let mut opt = usvg::Options::default();
        opt.fontdb_mut().load_system_fonts(); // 加载系统字体以获得更好的文本渲染

        let tree = usvg::Tree::from_str(svg_data, &opt).unwrap();
        let mut pixmap = Pixmap::new(size, size).unwrap();

        // 计算缩放和偏移以保持纵横比
        let tree_size = tree.size();
        let scale_x = size as f32 / tree_size.width();
        let scale_y = size as f32 / tree_size.height();
        let scale = scale_x.min(scale_y);

        let offset_x = (size as f32 - tree_size.width() * scale) * 0.5;
        let offset_y = (size as f32 - tree_size.height() * scale) * 0.5;

        // 使用现代的变换API，支持高质量渲染
        let transform =
            usvg::Transform::from_translate(offset_x, offset_y).post_scale(scale, scale);

        // 使用高质量渲染设置
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let rgba_data = pixmap.take();

        // 使用wgpu 26的现代纹理描述符，优化内存布局
        let texture_size = wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        };

        let texture_descriptor = wgpu::TextureDescriptor {
            label: Some("SVG Icon Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[], // 现代wgpu 26支持的视图格式
        };

        let texture = self.device.create_texture(&texture_descriptor);

        // 使用wgpu 26的现代write_texture API，性能优化
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size), // RGBA = 4 bytes per pixel
                rows_per_image: Some(size),
            },
            texture_size,
        );

        texture
    }

    fn initialize_svg_icons(&mut self) {
        let icons = [
            (
                Tool::Rectangle,
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" class=\"lucide lucide-square-icon lucide-square\"><rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\"/></svg>",
            ),
            (
                Tool::Circle,
                "<svg viewBox=\"0 0 24 24\" xmlns=\"http://www.w3.org/2000/svg\"><circle cx=\"12\" cy=\"12\" r=\"9\" fill=\"none\" stroke=\"#000000\" stroke-width=\"2\"/></svg>",
            ),
            (
                Tool::Arrow,
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" class=\"lucide lucide-move-up-right-icon lucide-move-up-right\"><path d=\"M13 5H19V11\"/><path d=\"M19 5L5 19\"/></svg>",
            ),
            (
                Tool::Pen,
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" class=\"lucide lucide-pen-line-icon lucide-pen-line\"><path d=\"M12 20h9\"/><path d=\"M16.376 3.622a1 1 0 0 1 3.002 3.002L7.368 18.635a2 2 0 0 1-.855.506l-2.872.838a.5.5 0 0 1-.62-.62l.838-2.872a2 2 0 0 1 .506-.854z\"/></svg>",
            ),
            (
                Tool::Text,
                "<svg viewBox=\"0 0 24 24\" xmlns=\"http://www.w3.org/2000/svg\"><polyline points=\"4,7 4,4 20,4 20,7\" stroke=\"#000000\" stroke-width=\"2\" fill=\"none\"/><line x1=\"9\" y1=\"20\" x2=\"15\" y2=\"20\" stroke=\"#000000\" stroke-width=\"2\"/><line x1=\"12\" y1=\"4\" x2=\"12\" y2=\"20\" stroke=\"#000000\" stroke-width=\"2\"/></svg>",
            ),
            (
                Tool::Undo,
                "<svg viewBox=\"0 0 24 24\" xmlns=\"http://www.w3.org/2000/svg\"><path d=\"M1 4v6h6\" stroke=\"#000000\" stroke-width=\"2\" fill=\"none\"/><path d=\"M3.51 15a9 9 0 1 0 2.13-9.36L1 10\" stroke=\"#000000\" stroke-width=\"2\" fill=\"none\"/></svg>",
            ),
            (
                Tool::Save,
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" class=\"lucide lucide-download-icon lucide-download\"><path d=\"M12 15V3\"/><path d=\"M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4\"/><path d=\"m7 10 5 5 5-5\"/></svg>",
            ),
            (
                Tool::Exit,
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"#f50000\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" class=\"lucide lucide-x-icon lucide-x\"><path d=\"M18 6 6 18\"/><path d=\"m6 6 12 12\"/></svg>",
            ),
            (
                Tool::Complete,
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"#00f53d\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" class=\"lucide lucide-check-icon lucide-check\"><path d=\"M20 6 9 17l-5-5\"/></svg>",
            ),
        ];

        let icon_size = 32;

        for (tool, svg_data) in icons.iter() {
            let texture = self.load_svg_texture(svg_data, icon_size);
            // 使用wgpu 26的现代纹理视图配置
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("SVG Icon Texture View"),
                format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: Some(1),
                usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
            });
            // 使用wgpu 26的现代采样器配置，优化SVG图标渲染质量
            let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("SVG Icon Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear, // 放大时使用线性过滤获得更好质量
                min_filter: wgpu::FilterMode::Linear, // 缩小时也使用线性过滤
                mipmap_filter: wgpu::FilterMode::Linear, // mipmap过滤也使用线性
                lod_min_clamp: 0.0,
                lod_max_clamp: 32.0,
                compare: None,
                anisotropy_clamp: 1, // wgpu 26支持的各向异性过滤
                border_color: None,
            });

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
                label: Some("SVG Icon Bind Group"),
            });

            self.icon_textures.insert(*tool, texture);
            self.icon_bind_groups.insert(*tool, bind_group);
        }
    }

    fn get_icon_bind_group(&self, tool: Tool) -> Option<&wgpu::BindGroup> {
        self.icon_bind_groups.get(&tool)
    }

    fn initialize_toolbar(&mut self) {
        self.toolbar_buttons = vec![
            ToolbarButton {
                tool: Tool::Rectangle,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Circle,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Arrow,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Pen,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Text,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Undo,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Save,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Exit,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                is_selected: false,
            },
            ToolbarButton {
                tool: Tool::Complete,
                rect: (0.0, 0.0, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE),
                is_selected: false,
            },
        ];
        self.update_toolbar_layout();
    }

    fn update_toolbar_layout(&mut self) {
        if let Some((box_min_x, box_min_y, _box_max_x, box_max_y)) = self.current_box_coords {
            // 计算工具栏宽度（与shader中的计算保持一致）
            let total_width = (self.toolbar_buttons.len() as f32) * TOOLBAR_BUTTON_SIZE
                + ((self.toolbar_buttons.len() - 1) as f32) * TOOLBAR_BUTTON_MARGIN;

            // 首先尝试在框的下方
            let mut toolbar_y = box_max_y + 5.0;
            let toolbar_bottom = toolbar_y + TOOLBAR_HEIGHT;

            // 如果超出屏幕下边界，移到框的上方
            if toolbar_bottom > self.size.height as f32 {
                toolbar_y = box_min_y - TOOLBAR_HEIGHT - 10.0;

                // 如果移到上方还是超出屏幕，则放在屏幕顶部
                if toolbar_y < 0.0 {
                    toolbar_y = 10.0;
                }
            }

            // 计算X坐标
            let mut toolbar_start_x = box_min_x;
            if toolbar_start_x + total_width > self.size.width as f32 {
                toolbar_start_x = (self.size.width as f32 - total_width).max(0.0);
            } else {
                toolbar_start_x = toolbar_start_x.max(0.0);
            }

            // 更新每个按钮的位置（考虑垂直居中）
            for (i, button) in self.toolbar_buttons.iter_mut().enumerate() {
                let x =
                    toolbar_start_x + (i as f32) * (TOOLBAR_BUTTON_SIZE + TOOLBAR_BUTTON_MARGIN);

                // 按钮在工具栏内垂直居中
                let button_y_offset = (TOOLBAR_HEIGHT - TOOLBAR_BUTTON_SIZE) * 0.5;
                let y = toolbar_y + button_y_offset;

                button.rect = (x, y, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SIZE);
            }
        }
    }

    fn show_toolbar(&mut self) {
        self.show_toolbar = true;
        if self.current_box_coords.is_some() {
            self.update_toolbar_layout();
        }
        // 🔧 修复：显示工具栏时标记需要重绘
        self.needs_redraw = true;
        self.render_cache_valid = false;
        // 🚀 工具栏状态改变时，背景缓存失效
        self.invalidate_background_cache();
    }

    fn hide_toolbar(&mut self) {
        self.show_toolbar = false;
        // 🔧 修复：隐藏工具栏时标记需要重绘
        self.needs_redraw = true;
        self.render_cache_valid = false;
        // 🚀 工具栏状态改变时，背景缓存失效
        self.invalidate_background_cache();
    }

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

    // 🚀 检查工具栏按钮是否禁用
    fn is_toolbar_button_disabled(&self, tool: Tool) -> bool {
        match tool {
            Tool::Undo => {
                // 撤销按钮：无撤销历史时禁用
                self.undo_stack.is_empty()
            }
            _ => false, // 其他按钮默认启用
        }
    }

    fn set_current_tool(&mut self, tool: Tool) {
        self.current_tool = tool;
    }

    // 🚀 根据绘图元素类型更新当前工具状态
    fn update_tool_from_element(&mut self, element: &DrawingElement) {
        let tool = match element {
            DrawingElement::Rectangle { .. } => Tool::Rectangle,
            DrawingElement::Circle { .. } => Tool::Circle,
            DrawingElement::Arrow { .. } => Tool::Arrow,
            DrawingElement::Pen { .. } => Tool::Pen,
            DrawingElement::Text { .. } => Tool::Text,
        };

        // 更新当前工具
        self.current_tool = tool;

        // 更新工具栏按钮选择状态
        for button in &mut self.toolbar_buttons {
            button.is_selected = button.tool == tool;
        }

        // 更新uniforms以反映状态变化
        self.update_uniforms();
    }

    fn handle_toolbar_click(&mut self, tool: Tool) -> bool {
        self.toolbar_active = true;

        for button in &mut self.toolbar_buttons {
            button.is_selected = false;
        }

        for button in self.toolbar_buttons.iter_mut() {
            if button.tool == tool {
                button.is_selected = true;
                break;
            }
        }

        match tool {
            Tool::None => {
                // 🚀 无工具选择：取消所有选择，进入空闲状态
                self.current_tool = Tool::None;
                self.deselect_element();
                self.drawing_state = DrawingState::Idle;
                false
            }
            Tool::Rectangle | Tool::Circle | Tool::Arrow | Tool::Pen | Tool::Text => {
                self.set_current_tool(tool);
                self.update_uniforms();
                false
            }
            Tool::Undo => {
                // 🚀 只有在有撤销历史时才执行撤销
                if !self.undo_stack.is_empty() {
                    self.undo();
                } else {
                    println!("⚠️ 没有可撤销的操作");
                }
                self.update_uniforms();
                false
            }
            Tool::Save => {
                println!("保存截图");
                self.update_uniforms();
                false
            }
            Tool::Exit => true,
            Tool::Complete => {
                println!("完成截图");
                self.update_uniforms();
                false
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

        // 使用wgpu 26的现代纹理视图配置
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Screenshot Texture View"),
            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(1),
            usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
        });
        // 使用wgpu 26的现代采样器配置，优化截图纹理渲染
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Screenshot Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

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

    // 🚀 创建背景缓存纹理
    fn create_background_cache_texture(&mut self) {
        if self.size.width == 0 || self.size.height == 0 {
            return;
        }

        // 创建背景缓存纹理
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Background Cache Texture"),
            size: wgpu::Extent3d {
                width: self.size.width,
                height: self.size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.surface_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 创建采样器
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Background Cache Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

        // 创建绑定组
        let bind_group_layout = &self.render_pipeline.get_bind_group_layout(1);
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
            ],
            label: Some("Background Cache Bind Group"),
        });

        self.background_cache_texture = Some(texture);
        self.background_cache_view = Some(view);
        self.background_cache_bind_group = Some(bind_group);
        self.background_cache_valid = false; // 需要重新渲染
    }

    // 🚀 渲染背景到缓存纹理
    fn render_background_to_cache(&mut self) {
        if let (Some(cache_view), Some(bind_group)) =
            (&self.background_cache_view, &self.bind_group)
        {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Background Cache Encoder"),
                });

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Background Cache Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: cache_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                // 使用背景缓存管道渲染背景
                render_pass.set_pipeline(&self.background_cache_pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..6, 0..1);
            }

            self.queue.submit(std::iter::once(encoder.finish()));
            self.background_cache_valid = true;
            self.force_background_update = false;
        }
    }

    // 🚀 标记背景缓存无效
    fn invalidate_background_cache(&mut self) {
        self.background_cache_valid = false;
        self.force_background_update = true;
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
                    // 恢复原来的设置，保持最佳响应性
                    present_mode: wgpu::PresentMode::Immediate,
                    alpha_mode: wgpu::CompositeAlphaMode::Auto,
                    view_formats: vec![],
                    // 恢复最低延迟
                    desired_maximum_frame_latency: 1,
                },
            );
        }
    }

    fn update_uniforms(&mut self) {
        let hovered_index = self.hovered_button.map(|i| i as f32).unwrap_or(-1.0);
        let selected_index = self
            .toolbar_buttons
            .iter()
            .position(|btn| btn.is_selected && btn.tool == self.current_tool)
            .map(|i| i as f32)
            .unwrap_or(-1.0);

        // 🚀 获取撤销按钮状态
        let undo_button_enabled = !self.undo_stack.is_empty();

        let uniform_data = [
            self.current_box_coords
                .map_or(-1.0, |(min_x, _, _, _)| min_x),
            self.current_box_coords
                .map_or(-1.0, |(_, min_y, _, _)| min_y),
            self.current_box_coords
                .map_or(-1.0, |(_, _, max_x, _)| max_x),
            self.current_box_coords
                .map_or(-1.0, |(_, _, _, max_y)| max_y),
            self.size.width as f32,
            self.size.height as f32,
            self.border_width,
            self.handle_size,
            self.handle_border_width,
            if self.show_toolbar { 1.0 } else { 0.0 },
            TOOLBAR_HEIGHT,
            hovered_index,
            if self.toolbar_active { 1.0 } else { 0.0 },
            selected_index,
            TOOLBAR_BUTTON_SIZE,
            TOOLBAR_BUTTON_MARGIN,
            self.border_color[0],
            self.border_color[1],
            self.border_color[2],
            1.0,
            self.handle_color[0],
            self.handle_color[1],
            self.handle_color[2],
            1.0,
            self.toolbar_buttons.len() as f32,
            // 🚀 背景缓存控制参数
            if self.background_cache_valid {
                1.0
            } else {
                0.0
            },
            if self.force_background_update {
                1.0
            } else {
                0.0
            },
            // 🚀 绘图元素手柄参数
            if self.selected_element.is_some() {
                1.0
            } else {
                0.0
            }, // 是否显示手柄
            // 🚀 撤销按钮状态
            if undo_button_enabled { 1.0 } else { 0.0 },
        ];

        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&uniform_data));
    }

    fn update_box(&mut self, min_x: f32, min_y: f32, max_x: f32, max_y: f32) {
        self.current_box_coords = Some((min_x, min_y, max_x, max_y));
        self.update_uniforms();
        if self.show_toolbar {
            self.update_toolbar_layout();
        }
        // 🔧 修复：确保框更新时标记需要重绘
        self.needs_redraw = true;
        self.render_cache_valid = false;
        // 🚀 框位置改变时，背景缓存失效
        self.invalidate_background_cache();
    }

    // 🚀 智能背景缓存渲染系统
    fn render(&mut self) {
        if self.size.width == 0 || self.size.height == 0 {
            return;
        }

        // 🚀 检查是否需要创建背景缓存纹理
        if self.background_cache_texture.is_none() {
            self.create_background_cache_texture();
        }

        // 🚀 检查是否需要更新背景缓存
        if !self.background_cache_valid || self.force_background_update {
            self.render_background_to_cache();
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
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // 🚀 使用智能缓存渲染：暂时简化，只使用主绑定组
            if let Some(bind_group) = &self.bind_group {
                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                // 如果有背景缓存绑定组，也设置它
                if let Some(cache_bind_group) = &self.background_cache_bind_group {
                    render_pass.set_bind_group(1, cache_bind_group, &[]);
                }
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..6, 0..1);
            }

            // 渲染绘图元素
            self.render_drawings_batched(&mut render_pass);

            // 🚀 渲染选中元素的手柄
            if self.selected_element.is_some() {
                self.render_element_handles(&mut render_pass);
            }

            // 🚀 渲染当前正在绘制元素的临时手柄
            if self.drawing_state == DrawingState::Drawing && self.current_drawing.is_some() {
                self.render_current_drawing_handles(&mut render_pass);
            }

            if self.show_toolbar {
                self.render_svg_toolbar_icons(&mut render_pass);
            }

            // 🚀 在主渲染通道中渲染文本 - 检查状态后调用合适的函数
            let is_editing = self.text_input_active;
            if is_editing {
                self.render_all_text_with_editing(&mut render_pass);
            } else {
                self.render_completed_text(&mut render_pass);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // 🔧 GPU优化：重置重绘标志，避免不必要的渲染
        self.needs_redraw = false;
        self.render_cache_valid = true;
    }

    // 🔧 GPU优化：智能重绘请求，只在真正需要时请求重绘
    fn request_redraw_if_needed(&mut self) {
        if self.needs_redraw {
            self.window.request_redraw();
        }
    }

    // 🔧 GPU优化：标记需要重绘
    fn mark_needs_redraw(&mut self) {
        self.needs_redraw = true;
        self.render_cache_valid = false;
    }

    // 🚀 撤销系统：保存当前状态到撤销栈
    fn save_state_for_undo(&mut self) {
        // 限制撤销栈大小，避免内存过度使用
        const MAX_UNDO_STEPS: usize = 50;

        if self.undo_stack.len() >= MAX_UNDO_STEPS {
            self.undo_stack.remove(0); // 移除最旧的状态
        }

        // 保存当前绘图元素状态
        self.undo_stack.push(self.drawing_elements.clone());

        // 清空重做栈（新操作后不能重做之前的撤销）
        self.redo_stack.clear();

        // 🚀 更新工具栏按钮状态
        self.update_undo_redo_button_states();
    }

    // 🚀 撤销操作 (Ctrl+Z)
    fn undo(&mut self) {
        if let Some(previous_state) = self.undo_stack.pop() {
            // 将当前状态保存到重做栈
            self.redo_stack.push(self.drawing_elements.clone());

            // 恢复到之前的状态
            self.drawing_elements = previous_state;

            // 取消当前选择
            self.deselect_element();

            // 标记需要重绘
            self.needs_redraw = true;
            self.render_cache_valid = false;

            // 🚀 清除所有缓存
            self.cached_drawing_vertices.clear();
            self.drawing_cache_valid.clear();

            // 🚀 强制请求重绘
            self.window.request_redraw();

            // 🚀 更新工具栏按钮状态
            self.update_undo_redo_button_states();

            println!("🔄 撤销操作，剩余撤销步数: {}", self.undo_stack.len());
        } else {
            println!("⚠️ 没有可撤销的操作");
        }
    }

    // 🚀 重做操作 (Ctrl+Y 或 Ctrl+Shift+Z)
    fn redo(&mut self) {
        if let Some(next_state) = self.redo_stack.pop() {
            // 将当前状态保存到撤销栈
            self.undo_stack.push(self.drawing_elements.clone());

            // 恢复到重做状态
            self.drawing_elements = next_state;

            // 取消当前选择
            self.deselect_element();

            // 标记需要重绘
            self.needs_redraw = true;
            self.render_cache_valid = false;

            // 🚀 清除所有缓存
            self.cached_drawing_vertices.clear();
            self.drawing_cache_valid.clear();

            // 🚀 强制请求重绘
            self.window.request_redraw();

            // 🚀 更新工具栏按钮状态
            self.update_undo_redo_button_states();

            println!("🔄 重做操作，剩余重做步数: {}", self.redo_stack.len());
        } else {
            println!("⚠️ 没有可重做的操作");
        }
    }

    // 🚀 更新撤销/重做按钮的启用/禁用状态
    fn update_undo_redo_button_states(&mut self) {
        let has_undo_history = !self.undo_stack.is_empty();

        // 🚀 更新每个按钮的状态
        for (index, button) in self.toolbar_buttons.iter_mut().enumerate() {
            match button.tool {
                Tool::Undo => {
                    // 🚀 撤销按钮：根据撤销历史设置状态
                    button.is_selected = has_undo_history;
                    println!(
                        "🔄 撤销按钮状态: 索引={}, 撤销历史={}, is_selected={}",
                        index, has_undo_history, button.is_selected
                    );
                }
                _ => {
                    // 🚀 其他按钮：根据当前工具设置状态
                    button.is_selected = button.tool == self.current_tool;
                    if button.is_selected {
                        println!(
                            "🔄 当前工具按钮: 索引={}, 工具={:?}, is_selected={}",
                            index, button.tool, button.is_selected
                        );
                    }
                }
            }
        }

        // 更新uniforms以反映按钮状态变化
        self.update_uniforms();
    }

    // 🚀 处理文本输入
    fn handle_text_input(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::{KeyCode, PhysicalKey};

        println!("🚀 处理文本输入: {:?}", event.physical_key);

        match event.physical_key {
            PhysicalKey::Code(KeyCode::Enter) => {
                // 🚀 修复：Ctrl+Enter完成输入，单独Enter添加换行
                if self.modifiers.state().control_key() {
                    // Ctrl+Enter：完成文本输入
                    self.finish_text_input();
                    println!("🚀 Ctrl+Enter：完成文本输入");
                } else {
                    // 单独Enter：添加换行
                    println!(
                        "🚀 Enter前: 文本='{:?}', 光标位置={}, 文本长度={}",
                        self.current_text_input,
                        self.text_cursor_position,
                        self.current_text_input.len()
                    );

                    self.current_text_input
                        .insert(self.text_cursor_position, '\n');
                    self.text_cursor_position += 1;

                    println!(
                        "🚀 Enter后: 文本='{:?}', 光标位置={}, 文本长度={}",
                        self.current_text_input,
                        self.text_cursor_position,
                        self.current_text_input.len()
                    );

                    self.update_current_text_element();
                    println!("🚀 Enter：添加换行符");
                }
            }
            PhysicalKey::Code(KeyCode::Escape) => {
                // ESC键：取消文本输入
                self.cancel_text_input();
            }
            PhysicalKey::Code(KeyCode::Backspace) => {
                // 退格键：删除字符
                if !self.current_text_input.is_empty() && self.text_cursor_position > 0 {
                    self.text_cursor_position -= 1;
                    self.current_text_input.remove(self.text_cursor_position);
                    self.update_current_text_element();
                }
            }
            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                // 左箭头：移动光标
                if self.text_cursor_position > 0 {
                    self.text_cursor_position -= 1;
                    println!(
                        "🚀 光标左移到位置: {} (文本长度: {})",
                        self.text_cursor_position,
                        self.current_text_input.len()
                    );
                    // 🚀 修复：光标移动后触发重绘和文本更新
                    self.update_current_text_element(); // 强制更新文本元素
                    self.needs_redraw = true;
                    self.window.request_redraw();
                    println!("🚀 触发重绘请求和文本更新");
                } else {
                    println!("🚀 光标已在最左边，无法继续左移");
                }
            }
            PhysicalKey::Code(KeyCode::ArrowRight) => {
                // 右箭头：移动光标
                if self.text_cursor_position < self.current_text_input.len() {
                    self.text_cursor_position += 1;
                    println!("🚀 光标右移到位置: {}", self.text_cursor_position);
                    // 🚀 修复：光标移动后触发重绘和文本更新
                    self.update_current_text_element(); // 强制更新文本元素
                    self.needs_redraw = true;
                    self.window.request_redraw();
                    println!("🚀 触发重绘请求和文本更新");
                } else {
                    println!("🚀 光标已在最右边，无法继续右移");
                }
            }
            PhysicalKey::Code(KeyCode::ArrowUp) => {
                // 🚀 新增：上箭头移动到上一行
                self.move_cursor_up();
                // 🚀 修复：光标移动后触发重绘和文本更新
                self.update_current_text_element(); // 强制更新文本元素
                self.needs_redraw = true;
                self.window.request_redraw();
                println!("🚀 上箭头：触发重绘请求和文本更新");
            }
            PhysicalKey::Code(KeyCode::ArrowDown) => {
                // 🚀 新增：下箭头移动到下一行
                self.move_cursor_down();
                // 🚀 修复：光标移动后触发重绘和文本更新
                self.update_current_text_element(); // 强制更新文本元素
                self.needs_redraw = true;
                self.window.request_redraw();
                println!("🚀 下箭头：触发重绘请求和文本更新");
            }
            _ => {
                // 其他键：尝试作为字符输入
                if let Some(text) = self.key_to_char(event) {
                    println!("🚀 输入字符: '{}'", text);
                    self.current_text_input
                        .insert_str(self.text_cursor_position, &text);
                    self.text_cursor_position += text.len();
                    self.update_current_text_element();
                } else {
                    println!("🚀 未识别的按键: {:?}", event.physical_key);
                }
            }
        }

        self.window.request_redraw();
    }

    // 🚀 将按键转换为字符（简化版本）
    fn key_to_char(&self, event: &winit::event::KeyEvent) -> Option<String> {
        use winit::keyboard::{KeyCode, PhysicalKey};

        println!("🚀 尝试转换按键: {:?}", event.physical_key);

        let result = match event.physical_key {
            PhysicalKey::Code(KeyCode::Space) => Some(" ".to_string()),
            // 字母键
            PhysicalKey::Code(KeyCode::KeyA) => Some("a".to_string()),
            PhysicalKey::Code(KeyCode::KeyB) => Some("b".to_string()),
            PhysicalKey::Code(KeyCode::KeyC) => Some("c".to_string()),
            PhysicalKey::Code(KeyCode::KeyD) => Some("d".to_string()),
            PhysicalKey::Code(KeyCode::KeyE) => Some("e".to_string()),
            PhysicalKey::Code(KeyCode::KeyF) => Some("f".to_string()),
            PhysicalKey::Code(KeyCode::KeyG) => Some("g".to_string()),
            PhysicalKey::Code(KeyCode::KeyH) => Some("h".to_string()),
            PhysicalKey::Code(KeyCode::KeyI) => Some("i".to_string()),
            PhysicalKey::Code(KeyCode::KeyJ) => Some("j".to_string()),
            PhysicalKey::Code(KeyCode::KeyK) => Some("k".to_string()),
            PhysicalKey::Code(KeyCode::KeyL) => Some("l".to_string()),
            PhysicalKey::Code(KeyCode::KeyM) => Some("m".to_string()),
            PhysicalKey::Code(KeyCode::KeyN) => Some("n".to_string()),
            PhysicalKey::Code(KeyCode::KeyO) => Some("o".to_string()),
            PhysicalKey::Code(KeyCode::KeyP) => Some("p".to_string()),
            PhysicalKey::Code(KeyCode::KeyQ) => Some("q".to_string()),
            PhysicalKey::Code(KeyCode::KeyR) => Some("r".to_string()),
            PhysicalKey::Code(KeyCode::KeyS) => Some("s".to_string()),
            PhysicalKey::Code(KeyCode::KeyT) => Some("t".to_string()),
            PhysicalKey::Code(KeyCode::KeyU) => Some("u".to_string()),
            PhysicalKey::Code(KeyCode::KeyV) => Some("v".to_string()),
            PhysicalKey::Code(KeyCode::KeyW) => Some("w".to_string()),
            PhysicalKey::Code(KeyCode::KeyX) => Some("x".to_string()),
            PhysicalKey::Code(KeyCode::KeyY) => Some("y".to_string()),
            PhysicalKey::Code(KeyCode::KeyZ) => Some("z".to_string()),
            // 数字键
            PhysicalKey::Code(KeyCode::Digit0) => Some("0".to_string()),
            PhysicalKey::Code(KeyCode::Digit1) => Some("1".to_string()),
            PhysicalKey::Code(KeyCode::Digit2) => Some("2".to_string()),
            PhysicalKey::Code(KeyCode::Digit3) => Some("3".to_string()),
            PhysicalKey::Code(KeyCode::Digit4) => Some("4".to_string()),
            PhysicalKey::Code(KeyCode::Digit5) => Some("5".to_string()),
            PhysicalKey::Code(KeyCode::Digit6) => Some("6".to_string()),
            PhysicalKey::Code(KeyCode::Digit7) => Some("7".to_string()),
            PhysicalKey::Code(KeyCode::Digit8) => Some("8".to_string()),
            PhysicalKey::Code(KeyCode::Digit9) => Some("9".to_string()),
            _ => None,
        };

        if let Some(ref char) = result {
            println!("🚀 成功转换为字符: '{}'", char);
        } else {
            println!("🚀 无法转换按键: {:?}", event.physical_key);
        }

        result
    }

    // 🚀 更新当前文本元素的内容
    fn update_current_text_element(&mut self) {
        println!(
            "🚀 尝试更新文本元素，current_text_input: '{}'",
            self.current_text_input
        );

        if let Some(DrawingElement::Text { content, .. }) = &mut self.current_drawing {
            let old_content = content.clone();
            *content = self.current_text_input.clone();
            println!("🚀 文本内容更新: '{}' -> '{}'", old_content, content);

            // 标记需要重绘
            self.needs_redraw = true;
            self.render_cache_valid = false;
        } else {
            println!("🚀 警告：current_drawing不是Text类型或为None");
        }
    }

    // 🚀 完成文本输入
    fn finish_text_input(&mut self) {
        println!(
            "🚀 开始完成文本输入，current_text_input: '{}'",
            self.current_text_input
        );

        // 🚀 确保文本内容被保存到当前绘图元素中
        if let Some(DrawingElement::Text {
            content,
            is_editing,
            ..
        }) = &mut self.current_drawing
        {
            // 保存用户输入的文本内容
            *content = self.current_text_input.clone();
            *is_editing = false;

            println!("🚀 保存文本内容: '{}' (长度: {})", content, content.len());
        } else {
            println!("🚀 警告：current_drawing不是Text类型或为None");
        }

        // 🚀 改进：只有在文本完全为空（去除空白字符后）时才取消保存
        let trimmed_text = self.current_text_input.trim();
        if !trimmed_text.is_empty() {
            println!(
                "🚀 文本不为空，完成绘图并保存: '{}'",
                self.current_text_input
            );
            self.finish_current_drawing();
        } else {
            println!("🚀 文本为空（去除空白字符后），取消绘图，不保存");
            self.current_drawing = None;
            self.drawing_state = DrawingState::Idle;
        }

        // 退出文本输入模式
        self.text_input_active = false;
        self.current_text_input.clear();
        self.text_cursor_position = 0;

        println!("🚀 完成文本输入");
    }

    // 🚀 取消文本输入
    fn cancel_text_input(&mut self) {
        // 取消当前绘图
        self.current_drawing = None;

        // 退出文本输入模式
        self.text_input_active = false;
        self.current_text_input.clear();
        self.text_cursor_position = 0;
        self.drawing_state = DrawingState::Idle;

        println!("🚀 取消文本输入");
    }

    // 🚀 新增：上下箭头光标移动函数
    fn move_cursor_up(&mut self) {
        let lines: Vec<&str> = self.current_text_input.split('\n').collect();
        if lines.len() <= 1 {
            // 只有一行，移动到行首
            self.text_cursor_position = 0;
            println!("🚀 光标移动到行首: {}", self.text_cursor_position);
            return;
        }

        // 找到当前光标所在的行和列
        let mut current_pos = 0;
        let mut current_line = 0;
        let mut current_col = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            let line_end = current_pos + line.len();
            if self.text_cursor_position <= line_end {
                current_line = line_idx;
                current_col = self.text_cursor_position - current_pos;
                break;
            }
            current_pos = line_end + 1; // +1 for the '\n' character
        }

        if current_line > 0 {
            // 移动到上一行
            let prev_line = lines[current_line - 1];
            let prev_line_start = lines[..current_line - 1]
                .iter()
                .map(|l| l.len() + 1)
                .sum::<usize>();

            // 保持列位置，但不超过上一行的长度
            let new_col = current_col.min(prev_line.len());
            self.text_cursor_position = prev_line_start + new_col;
            println!(
                "🚀 光标上移到位置: {} (行: {}, 列: {})",
                self.text_cursor_position,
                current_line - 1,
                new_col
            );
        } else {
            // 已经在第一行，移动到行首
            self.text_cursor_position = 0;
            println!("🚀 光标移动到第一行行首: {}", self.text_cursor_position);
        }
    }

    fn move_cursor_down(&mut self) {
        let lines: Vec<&str> = self.current_text_input.split('\n').collect();
        if lines.len() <= 1 {
            // 只有一行，移动到行尾
            self.text_cursor_position = self.current_text_input.len();
            println!("🚀 光标移动到行尾: {}", self.text_cursor_position);
            return;
        }

        // 找到当前光标所在的行和列
        let mut current_pos = 0;
        let mut current_line = 0;
        let mut current_col = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            let line_end = current_pos + line.len();
            if self.text_cursor_position <= line_end {
                current_line = line_idx;
                current_col = self.text_cursor_position - current_pos;
                break;
            }
            current_pos = line_end + 1; // +1 for the '\n' character
        }

        if current_line < lines.len() - 1 {
            // 移动到下一行
            let next_line = lines[current_line + 1];
            let next_line_start = lines[..current_line + 1]
                .iter()
                .map(|l| l.len() + 1)
                .sum::<usize>();

            // 保持列位置，但不超过下一行的长度
            let new_col = current_col.min(next_line.len());
            self.text_cursor_position = next_line_start + new_col;
            println!(
                "🚀 光标下移到位置: {} (行: {}, 列: {})",
                self.text_cursor_position,
                current_line + 1,
                new_col
            );
        } else {
            // 已经在最后一行，移动到行尾
            self.text_cursor_position = self.current_text_input.len();
            println!("🚀 光标移动到最后一行行尾: {}", self.text_cursor_position);
        }
    }

    // 🚀 检测是否为双击
    fn is_double_click(&mut self, pos: (f32, f32)) -> bool {
        let now = std::time::Instant::now();
        let is_double = if let Some(last_pos) = self.last_click_position {
            let time_diff = now.duration_since(self.last_click_time);
            let distance = ((pos.0 - last_pos.0).powi(2) + (pos.1 - last_pos.1).powi(2)).sqrt();
            time_diff <= self.double_click_threshold && distance <= 10.0 // 10像素内算同一位置
        } else {
            false
        };

        // 更新点击记录
        self.last_click_time = now;
        self.last_click_position = Some(pos);

        is_double
    }

    // 🚀 开始编辑文本元素
    fn start_edit_text(&mut self, element_index: usize) {
        if element_index < self.drawing_elements.len() {
            // 先克隆元素以避免借用冲突
            let element = self.drawing_elements[element_index].clone();
            if let DrawingElement::Text {
                content,
                position,
                color,
                font_size,
                ..
            } = element
            {
                // 设置当前文本输入内容
                self.current_text_input = content.clone();
                self.text_cursor_position = content.len();

                // 创建编辑中的文本元素
                self.current_drawing = Some(DrawingElement::Text {
                    position,
                    content: content.clone(),
                    color,
                    font_size,
                    is_editing: true,
                    rotation: None, // 🚀 编辑时保持原有旋转
                });

                // 激活文本输入模式
                self.text_input_active = true;
                self.drawing_state = DrawingState::Drawing;

                // 🚀 修复：清除选中状态，避免显示多个手柄
                self.selected_element = None;

                // 从绘图元素列表中移除原文本（编辑完成后会重新添加）
                self.drawing_elements.remove(element_index);

                println!("🚀 开始编辑文本: '{}'", content);
            }
        }
    }

    // 🚀 根据当前拖拽位置动态确定矩形手柄类型
    fn get_dynamic_handle_type_static(
        original_handle: &Handle,
        pos: (f32, f32),
        start: (f32, f32),
        end: (f32, f32),
    ) -> HandleType {
        // 只对角手柄需要动态切换
        match original_handle.handle_type {
            HandleType::TopLeft
            | HandleType::TopRight
            | HandleType::BottomLeft
            | HandleType::BottomRight => {
                // 计算当前位置相对于矩形中心的象限
                let center_x = (start.0 + end.0) / 2.0;
                let center_y = (start.1 + end.1) / 2.0;

                let is_left = pos.0 < center_x;
                let is_top = pos.1 < center_y;

                match (is_left, is_top) {
                    (true, true) => HandleType::TopLeft,       // 左上象限
                    (false, true) => HandleType::TopRight,     // 右上象限
                    (true, false) => HandleType::BottomLeft,   // 左下象限
                    (false, false) => HandleType::BottomRight, // 右下象限
                }
            }
            // 边中点手柄保持不变
            _ => original_handle.handle_type,
        }
    }

    // 🚀 规范化矩形坐标，确保start是左上角，end是右下角
    fn normalize_rectangle(start: &mut (f32, f32), end: &mut (f32, f32)) {
        let left = start.0.min(end.0);
        let right = start.0.max(end.0);
        let top = start.1.min(end.1);
        let bottom = start.1.max(end.1);

        // 确保最小尺寸
        let width = (right - left).max(MIN_RECTANGLE_SIZE);
        let height = (bottom - top).max(MIN_RECTANGLE_SIZE);

        start.0 = left;
        start.1 = top;
        end.0 = left + width;
        end.1 = top + height;
    }

    // 🚀 渲染绘图元素的手柄
    fn render_element_handles(&mut self, render_pass: &mut wgpu::RenderPass) {
        if let Some(ref selected) = self.selected_element {
            let mut handle_vertices = Vec::new();

            for handle in &selected.handles {
                self.add_handle_vertices(handle, &mut handle_vertices);
            }

            // 🚀 为选中的元素添加虚线边框
            if selected.index < self.drawing_elements.len() {
                match &self.drawing_elements[selected.index] {
                    DrawingElement::Circle {
                        center,
                        radius_x,
                        radius_y,
                        ..
                    } => {
                        self.add_dashed_circle_border(
                            *center,
                            *radius_x,
                            *radius_y,
                            &mut handle_vertices,
                        );
                    }
                    DrawingElement::Text {
                        position,
                        content,
                        font_size,
                        is_editing,
                        ..
                    } => {
                        // 🚀 为选中的文本添加黑色虚线边框
                        if !*is_editing {
                            let lines: Vec<&str> = content.split('\n').collect();
                            let line_count = lines.len() as f32;
                            let max_line_width = lines
                                .iter()
                                .map(|line| line.len() as f32 * font_size * 0.6)
                                .fold(0.0, f32::max);

                            let text_width = max_line_width.max(100.0);
                            let text_height = font_size * 1.2 * line_count;

                            self.add_dashed_text_border(
                                *position,
                                text_width,
                                text_height,
                                &mut handle_vertices,
                            );
                        }
                    }
                    _ => {}
                }
            }

            if !handle_vertices.is_empty() {
                // 创建或更新手柄顶点缓冲区
                let handle_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Handle Vertex Buffer"),
                            contents: bytemuck::cast_slice(&handle_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });

                // 使用绘图渲染管道渲染手柄
                render_pass.set_pipeline(&self.drawing_render_pipeline);
                render_pass.set_vertex_buffer(0, handle_buffer.slice(..));
                render_pass.draw(0..(handle_vertices.len() / 7) as u32, 0..1);
            }
        }
    }

    // 🚀 渲染当前正在绘制元素的临时手柄
    fn render_current_drawing_handles(&mut self, render_pass: &mut wgpu::RenderPass) {
        if let Some(ref current_drawing) = self.current_drawing {
            // 🚀 修复：输入文字时也显示手柄
            let mut handle_vertices = Vec::new();
            let temp_handles = self.generate_handles_for_element(current_drawing, 9999); // 使用临时索引

            for handle in &temp_handles {
                self.add_handle_vertices(handle, &mut handle_vertices);
            }

            // 🚀 为当前绘制的圆形添加虚线边框
            if let DrawingElement::Circle {
                center,
                radius_x,
                radius_y,
                ..
            } = current_drawing
            {
                self.add_dashed_circle_border(*center, *radius_x, *radius_y, &mut handle_vertices);
            }

            if !handle_vertices.is_empty() {
                // 创建临时手柄顶点缓冲区
                let handle_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Current Drawing Handle Buffer"),
                            contents: bytemuck::cast_slice(&handle_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });

                // 使用绘图渲染管道渲染手柄
                render_pass.set_pipeline(&self.drawing_render_pipeline);
                render_pass.set_vertex_buffer(0, handle_buffer.slice(..));
                render_pass.draw(0..(handle_vertices.len() / 7) as u32, 0..1);
            }
        }
    }

    // 🚀 更新鼠标指针状态
    fn update_cursor(&mut self, mouse_pos: (f32, f32)) {
        let new_cursor = if self.dragging_handle.is_some() {
            // 正在拖拽手柄
            match self.dragging_handle.as_ref().unwrap().handle_type {
                HandleType::TopLeft | HandleType::BottomRight => {
                    winit::window::CursorIcon::NwResize
                }
                HandleType::TopRight | HandleType::BottomLeft => {
                    winit::window::CursorIcon::NeResize
                }
                HandleType::TopCenter | HandleType::BottomCenter => {
                    winit::window::CursorIcon::NsResize
                }
                HandleType::MiddleLeft | HandleType::MiddleRight => {
                    winit::window::CursorIcon::EwResize
                }
                // 圆形现在使用矩形手柄类型，不需要特殊处理
                HandleType::ArrowStart | HandleType::ArrowEnd => {
                    winit::window::CursorIcon::Crosshair
                }
                HandleType::Move => winit::window::CursorIcon::Move,
                HandleType::Rotate => winit::window::CursorIcon::Grab, // 🚀 旋转手柄光标
            }
        } else if let Some(ref selected) = self.selected_element {
            // 检查是否悬停在手柄上
            if let Some(ref hovered) = self.hovered_handle {
                match hovered.handle_type {
                    HandleType::TopLeft | HandleType::BottomRight => {
                        winit::window::CursorIcon::NwResize
                    }
                    HandleType::TopRight | HandleType::BottomLeft => {
                        winit::window::CursorIcon::NeResize
                    }
                    HandleType::TopCenter | HandleType::BottomCenter => {
                        winit::window::CursorIcon::NsResize
                    }
                    HandleType::MiddleLeft | HandleType::MiddleRight => {
                        winit::window::CursorIcon::EwResize
                    }
                    // 圆形现在使用矩形手柄类型，不需要特殊处理
                    HandleType::ArrowStart | HandleType::ArrowEnd => {
                        winit::window::CursorIcon::Crosshair
                    }
                    HandleType::Move => winit::window::CursorIcon::Move,
                    HandleType::Rotate => winit::window::CursorIcon::Grab, // 🚀 旋转手柄光标
                }
            } else if selected.is_moving {
                // 正在移动元素
                winit::window::CursorIcon::Move
            } else if selected.index < self.drawing_elements.len()
                && self.hit_test_element(mouse_pos, &self.drawing_elements[selected.index])
            {
                // 悬停在选中的元素上
                winit::window::CursorIcon::Move
            } else {
                winit::window::CursorIcon::Default
            }
        } else if self.drawing_state == DrawingState::Drawing {
            // 正在绘图
            winit::window::CursorIcon::Crosshair
        } else if self.toolbar_active {
            // 工具栏激活，准备绘图
            winit::window::CursorIcon::Crosshair
        } else {
            // 检查是否悬停在任何绘图元素上
            let mut hovering_element = false;
            for element in &self.drawing_elements {
                if self.hit_test_element(mouse_pos, element) {
                    hovering_element = true;
                    break;
                }
            }
            if hovering_element {
                winit::window::CursorIcon::Pointer
            } else {
                winit::window::CursorIcon::Default
            }
        };

        // 只在指针状态改变时更新
        if new_cursor != self.current_cursor {
            self.current_cursor = new_cursor;
            self.window.set_cursor(new_cursor);
        }
    }

    // 🚀 添加手柄顶点数据 - 渲染为白色圆圈
    fn add_handle_vertices(&self, handle: &Handle, vertices: &mut Vec<f32>) {
        let screen_width = self.size.width as f32;
        let screen_height = self.size.height as f32;

        // 转换到NDC坐标
        let center_x = (handle.position.0 / screen_width) * 2.0 - 1.0;
        let center_y = 1.0 - (handle.position.1 / screen_height) * 2.0;
        let radius = handle.size / 2.0;
        let r_x = radius / screen_width * 2.0;
        let r_y = radius / screen_height * 2.0;

        // 手柄颜色：白色圆圈
        let outer_color = [1.0, 1.0, 1.0]; // 白色外圈
        let inner_color = [0.0, 0.0, 0.0]; // 黑色内圈

        // 如果是悬停状态，使用高亮颜色
        let final_outer_color =
            if self.hovered_handle.as_ref().map(|h| h.handle_type) == Some(handle.handle_type) {
                [1.0, 0.8, 0.0] // 橙色高亮
            } else {
                outer_color
            };

        let thickness = 4.0;

        // 🚀 绘制圆形手柄（使用多边形近似）
        const SEGMENTS: i32 = 12; // 减少段数，提高性能
        const ANGLE_STEP: f32 = 2.0 * std::f32::consts::PI / SEGMENTS as f32;

        // 外圈白色圆圈
        for i in 0..SEGMENTS {
            let angle1 = (i as f32) * ANGLE_STEP;
            let angle2 = ((i + 1) as f32) * ANGLE_STEP;

            let x1 = center_x + r_x * angle1.cos();
            let y1 = center_y + r_y * angle1.sin();
            let x2 = center_x + r_x * angle2.cos();
            let y2 = center_y + r_y * angle2.sin();

            vertices.extend_from_slice(&[
                x1,
                y1,
                final_outer_color[0],
                final_outer_color[1],
                final_outer_color[2],
                1.0,
                thickness,
                x2,
                y2,
                final_outer_color[0],
                final_outer_color[1],
                final_outer_color[2],
                1.0,
                thickness,
            ]);
        }

        // 内圈黑色填充（较小的圆）
        let inner_r_x = r_x * 0.5; // 调整内圈大小
        let inner_r_y = r_y * 0.5;

        for i in 0..SEGMENTS {
            let angle1 = (i as f32) * ANGLE_STEP;
            let angle2 = ((i + 1) as f32) * ANGLE_STEP;

            let x1 = center_x + inner_r_x * angle1.cos();
            let y1 = center_y + inner_r_y * angle1.sin();
            let x2 = center_x + inner_r_x * angle2.cos();
            let y2 = center_y + inner_r_y * angle2.sin();

            vertices.extend_from_slice(&[
                x1,
                y1,
                inner_color[0],
                inner_color[1],
                inner_color[2],
                1.0,
                thickness,
                x2,
                y2,
                inner_color[0],
                inner_color[1],
                inner_color[2],
                1.0,
                thickness,
            ]);
        }
    }

    // 🚀 添加虚线矩形边框（用于椭圆选择指示）
    fn add_dashed_circle_border(
        &self,
        center: (f32, f32),
        radius_x: f32,
        radius_y: f32,
        vertices: &mut Vec<f32>,
    ) {
        let screen_width = self.size.width as f32;
        let screen_height = self.size.height as f32;

        // 计算包围椭圆的矩形边界
        let left = center.0 - radius_x;
        let right = center.0 + radius_x;
        let top = center.1 - radius_y;
        let bottom = center.1 + radius_y;

        // 转换到NDC坐标
        let x1 = (left / screen_width) * 2.0 - 1.0;
        let y1 = 1.0 - (top / screen_height) * 2.0;
        let x2 = (right / screen_width) * 2.0 - 1.0;
        let y2 = 1.0 - (bottom / screen_height) * 2.0;

        let color = [0.7, 0.7, 0.7]; // 灰色虚线
        let thickness = 2.0;

        // 🚀 简化的虚线绘制 - 使用更多段数让虚线更细密
        let segments_per_side = 20; // 每边20段，让虚线更细密

        // 上边虚线
        for i in 0..segments_per_side {
            if i % 2 == 0 {
                // 只画偶数段，形成虚线效果
                let t1 = i as f32 / segments_per_side as f32;
                let t2 = (i + 1) as f32 / segments_per_side as f32;
                let sx = x1 + (x2 - x1) * t1;
                let ex = x1 + (x2 - x1) * t2;
                vertices.extend_from_slice(&[
                    sx, y1, color[0], color[1], color[2], 1.0, thickness, ex, y1, color[0],
                    color[1], color[2], 1.0, thickness,
                ]);
            }
        }

        // 右边虚线
        for i in 0..segments_per_side {
            if i % 2 == 0 {
                let t1 = i as f32 / segments_per_side as f32;
                let t2 = (i + 1) as f32 / segments_per_side as f32;
                let sy = y1 + (y2 - y1) * t1;
                let ey = y1 + (y2 - y1) * t2;
                vertices.extend_from_slice(&[
                    x2, sy, color[0], color[1], color[2], 1.0, thickness, x2, ey, color[0],
                    color[1], color[2], 1.0, thickness,
                ]);
            }
        }

        // 下边虚线
        for i in 0..segments_per_side {
            if i % 2 == 0 {
                let t1 = i as f32 / segments_per_side as f32;
                let t2 = (i + 1) as f32 / segments_per_side as f32;
                let sx = x2 - (x2 - x1) * t1;
                let ex = x2 - (x2 - x1) * t2;
                vertices.extend_from_slice(&[
                    sx, y2, color[0], color[1], color[2], 1.0, thickness, ex, y2, color[0],
                    color[1], color[2], 1.0, thickness,
                ]);
            }
        }

        // 左边虚线
        for i in 0..segments_per_side {
            if i % 2 == 0 {
                let t1 = i as f32 / segments_per_side as f32;
                let t2 = (i + 1) as f32 / segments_per_side as f32;
                let sy = y2 - (y2 - y1) * t1;
                let ey = y2 - (y2 - y1) * t2;
                vertices.extend_from_slice(&[
                    x1, sy, color[0], color[1], color[2], 1.0, thickness, x1, ey, color[0],
                    color[1], color[2], 1.0, thickness,
                ]);
            }
        }
    }

    // 🚀 缩放文本元素（同时调整位置、大小和字体）
    fn scale_text_element(
        position: &mut (f32, f32),
        font_size: &mut f32,
        content: &str,
        mouse_pos: (f32, f32),
        handle_type: HandleType,
    ) {
        // 计算当前文本的边界
        let lines: Vec<&str> = content.split('\n').collect();
        let line_count = lines.len() as f32;
        let max_line_width = lines
            .iter()
            .map(|line| line.len() as f32 * *font_size * 0.6)
            .fold(0.0, f32::max);

        let current_width = max_line_width.max(100.0);
        let current_height = *font_size * 1.2 * line_count;

        // 计算文本中心点
        let center_x = position.0 + current_width / 2.0;
        let center_y = position.1 + current_height / 2.0;

        // 根据手柄类型计算缩放 - 所有手柄都朝移动方向改变大小
        let scale_factor = match handle_type {
            HandleType::TopLeft => {
                // 🚀 修复：左上角朝左上方向移动时扩大
                let dx = position.0 - mouse_pos.0; // 向左移动为正
                let dy = position.1 - mouse_pos.1; // 向上移动为正
                let scale_x = (current_width + dx) / current_width;
                let scale_y = (current_height + dy) / current_height;
                scale_x.min(scale_y).max(0.1).min(5.0) // 限制在0.1-5倍之间
            }
            HandleType::TopRight => {
                // 🚀 修复：右上角朝右上方向移动时扩大
                let dx = mouse_pos.0 - (position.0 + current_width); // 向右移动为正
                let dy = position.1 - mouse_pos.1; // 向上移动为正
                let scale_x = (current_width + dx) / current_width;
                let scale_y = (current_height + dy) / current_height;
                scale_x.min(scale_y).max(0.1).min(5.0)
            }
            HandleType::BottomLeft => {
                // 🚀 修复：左下角朝左下方向移动时扩大
                let dx = position.0 - mouse_pos.0; // 向左移动为正
                let dy = mouse_pos.1 - (position.1 + current_height); // 向下移动为正
                let scale_x = (current_width + dx) / current_width;
                let scale_y = (current_height + dy) / current_height;
                scale_x.min(scale_y).max(0.1).min(5.0)
            }
            HandleType::BottomRight => {
                // 🚀 修复：右下角朝右下方向移动时扩大
                let dx = mouse_pos.0 - (position.0 + current_width); // 向右移动为正
                let dy = mouse_pos.1 - (position.1 + current_height); // 向下移动为正
                let scale_x = (current_width + dx) / current_width;
                let scale_y = (current_height + dy) / current_height;
                scale_x.min(scale_y).max(0.1).min(5.0) // 限制在0.1-5倍之间
            }
            _ => 1.0, // 其他手柄不缩放
        };

        // 应用缩放，添加安全检查
        if scale_factor.is_finite() && scale_factor > 0.0 {
            let new_font_size = (*font_size * scale_factor).max(8.0).min(200.0); // 字体大小限制在8-200之间
            let new_width = current_width * scale_factor;
            let new_height = current_height * scale_factor;

            // 更新字体大小
            *font_size = new_font_size;

            // 根据手柄类型调整位置，让文本框朝移动方向扩大
            match handle_type {
                HandleType::TopLeft => {
                    // 🚀 修复：左上角朝左上扩大，调整左上角位置
                    position.0 = position.0 + current_width - new_width; // 向左扩大
                    position.1 = position.1 + current_height - new_height; // 向上扩大
                }
                HandleType::TopRight => {
                    // 🚀 修复：右上角朝右上扩大，调整上边位置
                    position.1 = position.1 + current_height - new_height; // 向上扩大
                    // 右边不调整，让文本向右扩大
                }
                HandleType::BottomLeft => {
                    // 🚀 修复：左下角朝左下扩大，调整左边位置
                    position.0 = position.0 + current_width - new_width; // 向左扩大
                    // 下边不调整，让文本向下扩大
                }
                HandleType::BottomRight => {
                    // 🚀 修复：右下角朝右下扩大，位置不变
                    // position 不变，让文本向右下方向扩大
                }
                _ => {}
            }

            println!(
                "🚀 文本缩放: 字体大小={:.1} -> {:.1}, 缩放因子={:.2}",
                *font_size / scale_factor,
                *font_size,
                scale_factor
            );
        } else {
            println!("🚀 警告：无效的缩放因子: {}", scale_factor);
            return; // 跳过无效的缩放
        }

        println!(
            "🚀 文本缩放: 字体大小={:.1} -> {:.1}, 缩放因子={:.2}",
            *font_size / scale_factor,
            *font_size,
            scale_factor
        );
    }

    // 🚀 添加虚线文本边框（用于文本选择指示）
    fn add_dashed_text_border(
        &self,
        position: (f32, f32),
        width: f32,
        height: f32,
        vertices: &mut Vec<f32>,
    ) {
        let screen_width = self.size.width as f32;
        let screen_height = self.size.height as f32;

        // 🚀 添加padding到文本边界
        let padding = 8.0; // 8像素的padding
        let left = position.0 - padding;
        let right = position.0 + width + padding;
        let top = position.1 - padding;
        let bottom = position.1 + height + padding;

        // 转换到NDC坐标
        let x1 = (left / screen_width) * 2.0 - 1.0;
        let y1 = 1.0 - (top / screen_height) * 2.0;
        let x2 = (right / screen_width) * 2.0 - 1.0;
        let y2 = 1.0 - (bottom / screen_height) * 2.0;

        let color = [0.0, 0.0, 0.0]; // 黑色虚线
        let thickness = 2.0;

        // 🚀 修复：使用固定的虚线段长度，保持密度一致
        let dash_length = 10.0; // 虚线段长度（像素）
        let gap_length = 5.0; // 间隔长度（像素）
        let pattern_length = dash_length + gap_length;

        // 上边虚线
        let mut current_pos = 0.0;
        while current_pos < width {
            let end_pos = (current_pos + dash_length).min(width);
            let t1 = current_pos / width;
            let t2 = end_pos / width;
            let sx = x1 + (x2 - x1) * t1;
            let ex = x1 + (x2 - x1) * t2;
            vertices.extend_from_slice(&[
                sx, y1, color[0], color[1], color[2], 1.0, thickness, ex, y1, color[0], color[1],
                color[2], 1.0, thickness,
            ]);
            current_pos += pattern_length;
        }

        // 下边虚线
        current_pos = 0.0;
        while current_pos < width {
            let end_pos = (current_pos + dash_length).min(width);
            let t1 = current_pos / width;
            let t2 = end_pos / width;
            let sx = x1 + (x2 - x1) * t1;
            let ex = x1 + (x2 - x1) * t2;
            vertices.extend_from_slice(&[
                sx, y2, color[0], color[1], color[2], 1.0, thickness, ex, y2, color[0], color[1],
                color[2], 1.0, thickness,
            ]);
            current_pos += pattern_length;
        }

        // 左边虚线
        current_pos = 0.0;
        while current_pos < height {
            let end_pos = (current_pos + dash_length).min(height);
            let t1 = current_pos / height;
            let t2 = end_pos / height;
            let sy = y1 + (y2 - y1) * t1;
            let ey = y1 + (y2 - y1) * t2;
            vertices.extend_from_slice(&[
                x1, sy, color[0], color[1], color[2], 1.0, thickness, x1, ey, color[0], color[1],
                color[2], 1.0, thickness,
            ]);
            current_pos += pattern_length;
        }

        // 右边虚线
        current_pos = 0.0;
        while current_pos < height {
            let end_pos = (current_pos + dash_length).min(height);
            let t1 = current_pos / height;
            let t2 = end_pos / height;
            let sy = y1 + (y2 - y1) * t1;
            let ey = y1 + (y2 - y1) * t2;
            vertices.extend_from_slice(&[
                x2, sy, color[0], color[1], color[2], 1.0, thickness, x2, ey, color[0], color[1],
                color[2], 1.0, thickness,
            ]);
            current_pos += pattern_length;
        }
    }

    // 🚀 缓存优化的绘图渲染：使用智能缓存减少重复计算
    fn render_drawings_batched(&mut self, render_pass: &mut wgpu::RenderPass) {
        // 🚀 收集所有绘图元素的顶点（使用缓存）
        let mut line_vertices = Vec::new();

        // 🚀 添加已完成的绘图元素（使用缓存）
        for element in &self.drawing_elements.clone() {
            self.add_element_vertices(element, &mut line_vertices);
        }

        // 🚀 添加当前正在绘制的元素（动态元素，可能需要实时计算）
        if let Some(ref current) = self.current_drawing.clone() {
            self.add_element_vertices(current, &mut line_vertices);

            // 🚀 为正在编辑的文本添加边框
            if let DrawingElement::Text { is_editing, .. } = current {
                if *is_editing {
                    self.render_text_border(&mut line_vertices);
                }
            }
        }

        // 如果没有顶点数据，直接返回
        if line_vertices.is_empty() {
            return;
        }

        // 简单直接的渲染
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Drawing Buffer"),
                contents: bytemuck::cast_slice(&line_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        render_pass.set_pipeline(&self.drawing_render_pipeline);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..(line_vertices.len() / 7) as u32, 0..1);
    }

    fn create_icon_quad_vertices_with_padding(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        padding: f32,
    ) -> [[f32; 4]; 6] {
        let screen_width = self.size.width as f32;
        let screen_height = self.size.height as f32;

        let icon_x = x + padding;
        let icon_y = y + padding;
        let icon_width = width - padding * 2.0;
        let icon_height = height - padding * 2.0;

        let ndc_x1 = (icon_x / screen_width) * 2.0 - 1.0;
        let ndc_y1 = 1.0 - (icon_y / screen_height) * 2.0;
        let ndc_x2 = ((icon_x + icon_width) / screen_width) * 2.0 - 1.0;
        let ndc_y2 = 1.0 - ((icon_y + icon_height) / screen_height) * 2.0;

        [
            [ndc_x1, ndc_y2, 0.0, 1.0],
            [ndc_x2, ndc_y2, 1.0, 1.0],
            [ndc_x2, ndc_y1, 1.0, 0.0],
            [ndc_x1, ndc_y2, 0.0, 1.0],
            [ndc_x2, ndc_y1, 1.0, 0.0],
            [ndc_x1, ndc_y1, 0.0, 0.0],
        ]
    }
    // 🚀 GPU优化：使用实例化渲染减少绘制调用，提高GPU利用率
    fn render_svg_toolbar_icons(&mut self, render_pass: &mut wgpu::RenderPass) {
        if self.toolbar_buttons.is_empty() {
            return;
        }

        render_pass.set_pipeline(&self.icon_render_pipeline);

        // 批量收集所有图标的实例数据
        let mut instance_data = Vec::new();
        let mut bind_groups = Vec::new();

        for (i, button) in self.toolbar_buttons.iter().enumerate() {
            if let Some(icon_bind_group) = self.get_icon_bind_group(button.tool) {
                let (btn_x, btn_y, btn_w, btn_h) = button.rect;

                // 计算实例变换矩阵
                let padding = if i == 3 || i == 4 || i == 5 || i == 6 {
                    3.0
                } else {
                    2.0
                };
                let icon_vertices = self
                    .create_icon_quad_vertices_with_padding(btn_x, btn_y, btn_w, btn_h, padding);

                instance_data.extend_from_slice(&icon_vertices);
                bind_groups.push(icon_bind_group);
            }
        }

        if !instance_data.is_empty() {
            // 创建或重用实例缓冲区
            let needed_size = (instance_data.len() * std::mem::size_of::<f32>()) as u64;

            // 🔧 GPU优化：使用更大的缓冲区避免频繁重新分配，降低GPU负载
            let _buffer_size = (needed_size * 4).max(8192);

            let instance_buffer =
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Icon Instance Buffer"),
                        contents: bytemuck::cast_slice(&instance_data),
                        usage: wgpu::BufferUsages::VERTEX,
                    });

            // 批量渲染所有图标
            for (i, bind_group) in bind_groups.iter().enumerate() {
                render_pass.set_bind_group(0, *bind_group, &[]);
                let vertex_start = (i * 6) as u32;
                render_pass.set_vertex_buffer(
                    0,
                    instance_buffer
                        .slice((vertex_start * 4 * 4) as u64..((vertex_start + 6) * 4 * 4) as u64),
                );
                render_pass.draw(0..6, 0..1);
            }
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.configure_surface();
        self.update_uniforms();
        // 🚀 窗口大小改变时，重新创建背景缓存纹理
        self.background_cache_texture = None;
        self.background_cache_view = None;
        self.background_cache_bind_group = None;
        self.invalidate_background_cache();

        // 🚀 更新文本渲染器视图大小
        self.text_renderer
            .resize(new_size.width, new_size.height, &self.queue);
    }
}

struct App {
    state: Option<State>,
    mouse_pressed: bool,
    box_start: (f32, f32),
    first_drag_move: bool,
    box_created: bool,
    current_box: Option<(f32, f32, f32, f32)>,
    drag_mode: DragMode,
    needs_redraw: bool,
    mouse_press_position: Option<(f32, f32)>,
}

#[derive(PartialEq)]
enum DragMode {
    Creating,
    Moving,
    Resizing(ResizeHandle),
    None,
}

#[derive(PartialEq, Clone, Copy)]
enum ResizeHandle {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleRight,
    BottomRight,
    BottomCenter,
    BottomLeft,
    MiddleLeft,
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
            needs_redraw: false,
            mouse_press_position: None,
        }
    }
}

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
                        .with_transparent(false)
                        .with_visible(false)
                        .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
                        .with_fullscreen(Some(Fullscreen::Borderless(None))),
                )
                .unwrap(),
        );

        let mut state = pollster::block_on(State::new(window.clone()));

        if let Some((rgba, width, height)) = screenshot_data {
            state.load_screenshot_from_data(rgba, width, height);
        }

        window.set_visible(true);
        self.state = Some(state);
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // 🔧 GPU优化：使用Wait模式降低CPU和GPU负载，只在有事件时处理
        event_loop.set_control_flow(ControlFlow::Wait);

        if let Some(state) = self.state.as_mut() {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::RedrawRequested => state.render(),
                WindowEvent::Resized(size) => state.resize(size),

                WindowEvent::MouseInput {
                    state: button_state,
                    button,
                    ..
                } => {
                    use winit::event::{ElementState, MouseButton};

                    if button == MouseButton::Left {
                        match button_state {
                            ElementState::Pressed => {
                                // 🚀 如果正在文本输入模式，点击其他地方完成文本输入
                                if state.text_input_active {
                                    println!("🚀 文本输入模式下点击其他地方，完成文本输入");
                                    state.finish_text_input();
                                    state.window.request_redraw();
                                    // 继续处理点击事件，不要直接返回
                                }

                                // 🚀 优先检查绘图元素交互（无论工具栏是否激活）
                                if let Some(mouse_pos) = state.mouse_position {
                                    // 首先检查是否点击了手柄
                                    if let Some(ref selected) = state.selected_element.clone() {
                                        for handle in &selected.handles {
                                            if state.hit_test_handle(mouse_pos, handle) {
                                                // 🚀 开始拖拽手柄前保存状态
                                                state.save_state_for_undo();

                                                // 🚀 更新工具栏状态以反映当前拖拽的元素类型
                                                if selected.index < state.drawing_elements.len() {
                                                    let element = state.drawing_elements
                                                        [selected.index]
                                                        .clone();
                                                    state.update_tool_from_element(&element);
                                                }

                                                state.dragging_handle = Some(handle.clone());
                                                if handle.handle_type == HandleType::Move {
                                                    if let Some(ref mut sel) =
                                                        state.selected_element
                                                    {
                                                        sel.is_moving = true;
                                                        sel.move_offset = mouse_pos;
                                                    }
                                                }
                                                state.window.request_redraw();
                                                return;
                                            }
                                        }
                                    }

                                    // 检查是否点击了绘图元素
                                    let mut clicked_element = false;
                                    // 🚀 先找到要点击的元素，避免借用冲突
                                    let mut clicked_element_index = None;
                                    for (i, element) in
                                        state.drawing_elements.iter().enumerate().rev()
                                    {
                                        if state.hit_test_element(mouse_pos, element) {
                                            clicked_element_index = Some(i);
                                            break;
                                        }
                                    }

                                    if let Some(i) = clicked_element_index {
                                        // 🚀 检测双击文本元素进行编辑
                                        if let DrawingElement::Text { .. } =
                                            &state.drawing_elements[i]
                                        {
                                            if state.is_double_click(mouse_pos) {
                                                println!("🚀 双击文本元素，开始编辑");
                                                state.start_edit_text(i);
                                                state.window.request_redraw();
                                                return;
                                            }
                                        }

                                        state.select_element(i);

                                        // 🚀 开始移动元素前保存状态
                                        state.save_state_for_undo();

                                        // 🚀 点击元素内部开始拖动
                                        if let Some(ref mut selected) = state.selected_element {
                                            selected.is_moving = true;
                                            selected.move_offset = mouse_pos;
                                        }
                                        state.window.request_redraw();
                                        clicked_element = true;
                                        return;
                                    }

                                    // 如果没有点击任何绘图元素
                                    if !clicked_element {
                                        // 如果工具栏激活，开始绘图
                                        if state.toolbar_active {
                                            // 取消之前的选择
                                            if state.selected_element.is_some() {
                                                state.deselect_element();
                                            }
                                            state.start_drawing(mouse_pos.0, mouse_pos.1);
                                            state.window.request_redraw();
                                            return;
                                        } else {
                                            // 🚀 点击空白区域，取消选择
                                            if state.selected_element.is_some() {
                                                state.deselect_element();
                                                state.window.request_redraw();
                                            }
                                        }
                                    }
                                }
                                self.mouse_pressed = true;
                                self.first_drag_move = true;

                                if !self.box_created {
                                    self.drag_mode = DragMode::Creating;
                                } else {
                                    self.drag_mode = DragMode::Moving;
                                }
                            }
                            ElementState::Released => {
                                // 完成绘图（但不包括文本输入）
                                if state.drawing_state == DrawingState::Drawing {
                                    // 🚀 对于文本工具，不要在鼠标释放时完成绘图
                                    if state.current_tool != Tool::Text {
                                        state.finish_current_drawing();
                                        state.window.request_redraw();
                                        return;
                                    }
                                    // 🚀 对于文本工具，只是停止绘图状态，但保持 current_drawing
                                    else if !state.text_input_active {
                                        // 如果文本输入没有激活，则完成绘图
                                        state.finish_current_drawing();
                                        state.window.request_redraw();
                                        return;
                                    }
                                }

                                // 🚀 停止拖拽手柄
                                if state.dragging_handle.is_some() {
                                    state.dragging_handle = None;
                                    state.window.request_redraw();
                                    return;
                                }

                                // 🚀 停止元素拖动
                                if let Some(ref mut selected) = state.selected_element {
                                    if selected.is_moving {
                                        selected.is_moving = false;
                                        state.window.request_redraw();
                                        return;
                                    }
                                }
                                self.mouse_pressed = false;
                                self.first_drag_move = false;
                                self.mouse_press_position = None;

                                if let Some(mouse_pos) = state.mouse_position {
                                    let toolbar_tool =
                                        state.get_toolbar_button_at(mouse_pos.0, mouse_pos.1);
                                    if let Some(tool) = toolbar_tool {
                                        // 🚀 检查按钮是否禁用，禁用的按钮不响应点击
                                        if !state.is_toolbar_button_disabled(tool) {
                                            let should_exit = state.handle_toolbar_click(tool);
                                            state.window.request_redraw();
                                            if should_exit {
                                                event_loop.exit();
                                                return;
                                            }
                                        } else {
                                            println!("⚠️ 按钮 {:?} 已禁用，忽略点击", tool);
                                        }
                                        self.mouse_pressed = false;
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
                                                state.show_toolbar();
                                                state.update_box(min_x, min_y, max_x, max_y);
                                                state.window.request_redraw();
                                            }
                                        }
                                    }
                                    DragMode::Resizing(_) | DragMode::Moving => {
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

                    if old_hovered != state.hovered_button {
                        state.window.request_redraw();
                    }
                    // 🚀 处理手柄拖拽
                    if state.dragging_handle.is_some() {
                        state.handle_drag((position.x as f32, position.y as f32));
                        state.window.request_redraw();
                        return;
                    }

                    // 🚀 处理元素内部拖动
                    let should_move = if let Some(ref selected) = state.selected_element {
                        selected.is_moving
                    } else {
                        false
                    };

                    if should_move {
                        let mouse_pos = (position.x as f32, position.y as f32);
                        let (offset, selected_index) =
                            if let Some(ref selected) = state.selected_element {
                                let offset = (
                                    mouse_pos.0 - selected.move_offset.0,
                                    mouse_pos.1 - selected.move_offset.1,
                                );
                                (offset, selected.index)
                            } else {
                                return;
                            };

                        if selected_index < state.drawing_elements.len() {
                            let element = &mut state.drawing_elements[selected_index];
                            State::move_element_static(element, offset);

                            // 更新手柄位置
                            let element_clone = element.clone();
                            let new_handles =
                                state.generate_handles_for_element(&element_clone, selected_index);

                            if let Some(ref mut selected) = state.selected_element {
                                selected.handles = new_handles;
                                selected.move_offset = mouse_pos;
                            }

                            state.needs_redraw = true;
                            state.window.request_redraw();
                            return;
                        }
                    }

                    // 🚀 更新手柄悬停状态
                    if let Some(ref selected) = state.selected_element.clone() {
                        let mouse_pos = (position.x as f32, position.y as f32);
                        let mut found_handle = None;

                        for handle in &selected.handles {
                            if state.hit_test_handle(mouse_pos, handle) {
                                found_handle = Some(handle.clone());
                                break;
                            }
                        }

                        if state.hovered_handle.as_ref().map(|h| &h.handle_type)
                            != found_handle.as_ref().map(|h| &h.handle_type)
                        {
                            state.hovered_handle = found_handle;
                            state.window.request_redraw();
                        }
                    }

                    // 🚀 更新鼠标指针状态
                    state.update_cursor((position.x as f32, position.y as f32));

                    // 更新绘图
                    if state.drawing_state == DrawingState::Drawing {
                        state.update_drawing(position.x as f32, position.y as f32);
                        state.window.request_redraw();
                        return;
                    }
                    if self.mouse_pressed && self.mouse_press_position.is_none() {
                        let mouse_pos = (position.x as f32, position.y as f32);
                        self.mouse_press_position = Some(mouse_pos);

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

                    // 更新鼠标指针样式
                    if !self.mouse_pressed {
                        let mouse_x = position.x as f32;
                        let mouse_y = position.y as f32;

                        let toolbar_button = state.get_toolbar_button_at(mouse_x, mouse_y);
                        let current_box = self.current_box;
                        let handle_size = state.handle_size;
                        let toolbar_active = state.toolbar_active;

                        if let Some(tool) = toolbar_button {
                            // 🚀 检查按钮是否禁用
                            if state.is_toolbar_button_disabled(tool) {
                                state
                                    .window
                                    .set_cursor(winit::window::CursorIcon::NotAllowed);
                            } else {
                                state.window.set_cursor(winit::window::CursorIcon::Pointer);
                            }
                        } else if self.box_created && !toolbar_active {
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
                                    .set_cursor(winit::window::CursorIcon::NotAllowed);
                            }
                        } else if self.box_created && toolbar_active {
                            if is_mouse_in_box_body_static(
                                mouse_x,
                                mouse_y,
                                current_box,
                                handle_size,
                            ) {
                                state.window.set_cursor(winit::window::CursorIcon::Default);
                            } else {
                                state
                                    .window
                                    .set_cursor(winit::window::CursorIcon::NotAllowed);
                            }
                        } else {
                            state
                                .window
                                .set_cursor(winit::window::CursorIcon::Crosshair);
                        }
                    }

                    if state.toolbar_active {
                        return;
                    }
                    if !self.mouse_pressed || self.drag_mode == DragMode::None {
                        return;
                    }

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

                WindowEvent::ModifiersChanged(modifiers) => {
                    // 存储修饰键状态
                    state.modifiers = modifiers;
                }

                WindowEvent::KeyboardInput { event, .. } => {
                    use winit::event::ElementState;
                    use winit::keyboard::{KeyCode, PhysicalKey};

                    if event.state == ElementState::Pressed {
                        // 🚀 如果正在文本输入模式，优先处理文本输入
                        if state.text_input_active {
                            println!("🚀 文本输入模式激活，处理按键: {:?}", event.physical_key);
                            state.handle_text_input(&event);
                            state.window.request_redraw(); // 确保重绘
                            return;
                        }

                        // 检查修饰键状态
                        let ctrl_pressed = state.modifiers.state().control_key();
                        let shift_pressed = state.modifiers.state().shift_key();

                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::KeyZ) if ctrl_pressed && !shift_pressed => {
                                // Ctrl+Z: 撤销
                                state.undo();
                            }
                            PhysicalKey::Code(KeyCode::KeyY) if ctrl_pressed => {
                                // Ctrl+Y: 重做
                                state.redo();
                            }
                            PhysicalKey::Code(KeyCode::KeyZ) if ctrl_pressed && shift_pressed => {
                                // Ctrl+Shift+Z: 重做（备选快捷键）
                                state.redo();
                            }
                            PhysicalKey::Code(KeyCode::KeyR) => {
                                self.box_created = false;
                                self.current_box = None;
                                self.drag_mode = DragMode::None;
                                state.hide_toolbar();
                                state.update_box(-1.0, -1.0, -1.0, -1.0);
                                state.window.request_redraw();
                            }
                            PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
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
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
