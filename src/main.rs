#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use resvg::tiny_skia::Pixmap;
use std::sync::Arc;
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
const DEFAULT_HANDLE_SIZE: f32 = 12.0;
const DEFAULT_HANDLE_BORDER_WIDTH: f32 = 1.0;
const DEFAULT_BORDER_COLOR: [f32; 3] = CYAN;
const DEFAULT_HANDLE_COLOR: [f32; 3] = CYAN;

// 拖拽配置常量
const MIN_BOX_SIZE: f32 = 20.0;
const FRAME_LIMIT_DRAG: u128 = 14;
const FRAME_LIMIT_IDLE: u128 = 33;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Tool {
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
        radius: f32,
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
        size: f32,
    },
}
#[derive(Debug, Clone, Copy, PartialEq)]
enum DrawingState {
    Idle,
    Drawing,
    Dragging,
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

    drawing_elements: Vec<DrawingElement>,
    current_drawing: Option<DrawingElement>,
    drawing_state: DrawingState,
    drawing_start_pos: Option<(f32, f32)>,
    pen_points: Vec<(f32, f32)>,

    // 绘图渲染相关
    drawing_render_pipeline: wgpu::RenderPipeline,
    drawing_vertex_buffer: Option<wgpu::Buffer>,
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

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

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
                cache: None,
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
            cache: None,
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
            drawing_render_pipeline,
            drawing_vertex_buffer: None,
        };

        state.configure_surface();

        state.initialize_toolbar();
        state.initialize_svg_icons();
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
                    radius: 0.0,
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
                    thickness: 2.0,
                });
            }
            Tool::Text => {
                // 文字工具暂时简化处理
                self.current_drawing = Some(DrawingElement::Text {
                    position: (x, y),
                    content: "文字".to_string(),
                    color: RED,
                    size: 16.0,
                });
                self.finish_current_drawing();
            }
            _ => {}
        }
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
                DrawingElement::Circle { center, radius, .. } => {
                    let dx = x - center.0;
                    let dy = y - center.1;
                    *radius = (dx * dx + dy * dy).sqrt();
                }
                DrawingElement::Arrow { end, .. } => {
                    *end = (x, y);
                }
                DrawingElement::Pen { .. } => {
                    // 使用优化的点添加方法
                    self.add_pen_point(x, y);
                }
                _ => {}
            }
        }
    }
    // 新增：完成当前绘图
    fn finish_current_drawing(&mut self) {
        if let Some(drawing) = self.current_drawing.take() {
            self.drawing_elements.push(drawing);
        }
        self.drawing_state = DrawingState::Idle;
        self.drawing_start_pos = None;
        self.pen_points.clear();
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

    // 新增：生成绘图顶点数据
    fn generate_drawing_vertices(&self) -> Vec<f32> {
        let mut vertices = Vec::new();

        // 转换所有绘图元素为顶点数据
        for element in &self.drawing_elements {
            self.add_element_vertices(element, &mut vertices);
        }

        // 添加当前正在绘制的元素
        if let Some(ref current) = self.current_drawing {
            self.add_element_vertices(current, &mut vertices);
        }

        vertices
    }
    // 优化：使用持久化顶点缓冲区
    fn create_or_update_drawing_buffer(&mut self) {
        let vertices = self.generate_drawing_vertices();
        if vertices.is_empty() {
            return;
        }

        // 如果缓冲区不存在或大小不够，重新创建
        let needed_size = (vertices.len() * std::mem::size_of::<f32>()) as u64;
        let should_recreate = if let Some(ref buffer) = self.drawing_vertex_buffer {
            buffer.size() < needed_size
        } else {
            true
        };

        if should_recreate {
            // 创建比需要大一些的缓冲区，避免频繁重新分配
            let buffer_size = (needed_size * 2).max(4096); // 至少4KB

            self.drawing_vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Drawing Vertex Buffer"),
                size: buffer_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        // 更新缓冲区数据
        if let Some(ref buffer) = self.drawing_vertex_buffer {
            self.queue
                .write_buffer(buffer, 0, bytemuck::cast_slice(&vertices));
        }
    }

    // 优化：画笔点采样，减少点数量
    fn add_pen_point(&mut self, x: f32, y: f32) {
        if let Some(DrawingElement::Pen { points, .. }) = &mut self.current_drawing {
            // 距离采样：只有移动足够距离才添加新点
            let min_distance = 3.0; // 像素

            if let Some(last_point) = points.last() {
                let dx = x - last_point.0;
                let dy = y - last_point.1;
                let distance = (dx * dx + dy * dy).sqrt();

                if distance >= min_distance {
                    points.push((x, y));
                    self.pen_points.push((x, y));
                }
            } else {
                points.push((x, y));
                self.pen_points.push((x, y));
            }
        }
    }
    // 新增：添加单个元素的顶点数据
    fn add_element_vertices(&self, element: &DrawingElement, vertices: &mut Vec<f32>) {
        let screen_width = self.size.width as f32;
        let screen_height = self.size.height as f32;

        match element {
            DrawingElement::Rectangle {
                start,
                end,
                color,
                thickness,
            } => {
                let x1 = (start.0 / screen_width) * 2.0 - 1.0;
                let y1 = 1.0 - (start.1 / screen_height) * 2.0;
                let x2 = (end.0 / screen_width) * 2.0 - 1.0;
                let y2 = 1.0 - (end.1 / screen_height) * 2.0;

                let lines = [
                    (x1, y1, x2, y1),
                    (x2, y1, x2, y2),
                    (x2, y2, x1, y2),
                    (x1, y2, x1, y1),
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
                radius,
                color,
                thickness,
            } => {
                let segments = 32;
                let cx = (center.0 / screen_width) * 2.0 - 1.0;
                let cy = 1.0 - (center.1 / screen_height) * 2.0;
                let r_x = radius / screen_width * 2.0;
                let r_y = radius / screen_height * 2.0;

                for i in 0..segments {
                    let angle1 = (i as f32) * 2.0 * std::f32::consts::PI / segments as f32;
                    let angle2 = ((i + 1) as f32) * 2.0 * std::f32::consts::PI / segments as f32;

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
                // 主线
                let x1 = (start.0 / screen_width) * 2.0 - 1.0;
                let y1 = 1.0 - (start.1 / screen_height) * 2.0;
                let x2 = (end.0 / screen_width) * 2.0 - 1.0;
                let y2 = 1.0 - (end.1 / screen_height) * 2.0;

                vertices.extend_from_slice(&[
                    x1, y1, color[0], color[1], color[2], 1.0, *thickness, x2, y2, color[0],
                    color[1], color[2], 1.0, *thickness,
                ]);

                // 箭头
                let dx = end.0 - start.0;
                let dy = end.1 - start.1;
                let len = (dx * dx + dy * dy).sqrt();
                if len > 0.0 {
                    let ux = dx / len;
                    let uy = dy / len;
                    let arrow_len = 15.0;
                    let arrow_width = 8.0;

                    let p1_x = end.0 - arrow_len * ux + arrow_width * uy;
                    let p1_y = end.1 - arrow_len * uy - arrow_width * ux;
                    let p2_x = end.0 - arrow_len * ux - arrow_width * uy;
                    let p2_y = end.1 - arrow_len * uy + arrow_width * ux;

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
                // 优化：减少线段数量，使用更大的步长
                let step = if points.len() > 100 { 2 } else { 1 }; // 点多时跳过一些点

                for i in (0..points.len().saturating_sub(step)).step_by(step) {
                    let j = (i + step).min(points.len() - 1);

                    let x1 = (points[i].0 / screen_width) * 2.0 - 1.0;
                    let y1 = 1.0 - (points[i].1 / screen_height) * 2.0;
                    let x2 = (points[j].0 / screen_width) * 2.0 - 1.0;
                    let y2 = 1.0 - (points[j].1 / screen_height) * 2.0;

                    vertices.extend_from_slice(&[
                        x1, y1, color[0], color[1], color[2], 1.0, *thickness, x2, y2, color[0],
                        color[1], color[2], 1.0, *thickness,
                    ]);
                }
            }
            DrawingElement::Text { .. } => {
                // 文字渲染比较复杂，这里暂时跳过
                // 可以考虑使用文字纹理或者其他文字渲染库
            }
        }
    }

    fn update_mouse_position(&mut self, x: f32, y: f32) {
        self.mouse_position = Some((x, y));
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

        if old_hovered != self.hovered_button {
            self.update_uniforms();
        }
    }

    fn load_svg_texture(&self, svg_data: &str, size: u32) -> wgpu::Texture {
        let opt = usvg::Options::default();
        let tree = usvg::Tree::from_str(svg_data, &opt).unwrap();
        let mut pixmap = Pixmap::new(size, size).unwrap();

        let tree_size = tree.size();
        let scale_x = size as f32 / tree_size.width();
        let scale_y = size as f32 / tree_size.height();
        let scale = scale_x.min(scale_y);

        let offset_x = (size as f32 - tree_size.width() * scale) * 0.5;
        let offset_y = (size as f32 - tree_size.height() * scale) * 0.5;

        let transform =
            usvg::Transform::from_translate(offset_x, offset_y).post_scale(scale, scale);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let rgba_data = pixmap.take();

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("SVG Icon Texture"),
            view_formats: &[],
        });

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
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            texture.size(),
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
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
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
        if let Some((box_min_x, box_min_y, box_max_x, box_max_y)) = self.current_box_coords {
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
    }

    fn hide_toolbar(&mut self) {
        self.show_toolbar = false;
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

    fn set_current_tool(&mut self, tool: Tool) {
        self.current_tool = tool;
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
            Tool::Rectangle | Tool::Circle | Tool::Arrow | Tool::Pen | Tool::Text => {
                self.set_current_tool(tool);
                self.update_uniforms();
                false
            }
            Tool::Undo => {
                self.undo_drawing();
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
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
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
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
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

            // 渲染绘图元素
            self.render_drawings_batched(&mut render_pass);
            if self.show_toolbar {
                self.render_svg_toolbar_icons(&mut render_pass);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn render_drawings_batched(&mut self, render_pass: &mut wgpu::RenderPass) {
        // 收集所有绘图元素的顶点
        let mut line_vertices = Vec::new();

        // 添加已完成的绘图元素
        for element in &self.drawing_elements {
            self.add_element_vertices(element, &mut line_vertices);
        }

        // 添加当前正在绘制的元素
        if let Some(ref current) = self.current_drawing {
            self.add_element_vertices(current, &mut line_vertices);
        }

        // 如果没有顶点数据，直接返回
        if line_vertices.is_empty() {
            return;
        }

        // 创建或重用顶点缓冲区
        let needed_size = (line_vertices.len() * std::mem::size_of::<f32>()) as u64;

        // 检查是否需要重新创建缓冲区
        let should_recreate = if let Some(ref buffer) = self.drawing_vertex_buffer {
            buffer.size() < needed_size
        } else {
            true
        };

        if should_recreate {
            // 创建比需要稍大的缓冲区，避免频繁重新分配
            let buffer_size = (needed_size * 2).max(8192); // 至少8KB

            self.drawing_vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Batched Drawing Buffer"),
                size: buffer_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        // 更新缓冲区数据
        if let Some(ref buffer) = self.drawing_vertex_buffer {
            self.queue
                .write_buffer(buffer, 0, bytemuck::cast_slice(&line_vertices));

            // 一次性渲染所有线条
            render_pass.set_pipeline(&self.drawing_render_pipeline);
            render_pass.set_vertex_buffer(0, buffer.slice(..));
            render_pass.draw(0..(line_vertices.len() / 7) as u32, 0..1);
        }
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
    fn render_svg_toolbar_icons(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.icon_render_pipeline);

        for (i, button) in self.toolbar_buttons.iter().enumerate() {
            if let Some(icon_bind_group) = self.get_icon_bind_group(button.tool) {
                let (btn_x, btn_y, btn_w, btn_h) = button.rect;

                // 为第三个图标（Arrow，索引为2）设置更大的padding，使其显示更小
                let icon_vertices = if i == 3 || i == 4 || i == 5 || i == 6 {
                    // Arrow图标使用更大的padding，使其显示更小
                    self.create_icon_quad_vertices_with_padding(btn_x, btn_y, btn_w, btn_h, 3.0)
                } else {
                    // 其他图标使用正常padding
                    self.create_icon_quad_vertices_with_padding(btn_x, btn_y, btn_w, btn_h, 2.0)
                };

                let temp_vertex_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("Icon {} Vertex Buffer", i)),
                            contents: bytemuck::cast_slice(&icon_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });

                render_pass.set_bind_group(0, icon_bind_group, &[]);
                render_pass.set_vertex_buffer(0, temp_vertex_buffer.slice(..));
                render_pass.draw(0..6, 0..1);
            }
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.configure_surface();
        self.update_uniforms();
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
    last_update_time: std::time::Instant,
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
            last_update_time: std::time::Instant::now(),
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
                                if state.toolbar_active {
                                    // 如果工具栏激活，开始绘图
                                    if let Some(mouse_pos) = state.mouse_position {
                                        state.start_drawing(mouse_pos.0, mouse_pos.1);
                                        state.window.request_redraw();
                                    }
                                    return;
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
                                // 完成绘图
                                if state.drawing_state == DrawingState::Drawing {
                                    state.finish_current_drawing();
                                    state.window.request_redraw();
                                    return;
                                }
                                self.mouse_pressed = false;
                                self.first_drag_move = false;
                                self.mouse_press_position = None;
                                event_loop.set_control_flow(ControlFlow::Wait);

                                if let Some(mouse_pos) = state.mouse_position {
                                    let toolbar_tool =
                                        state.get_toolbar_button_at(mouse_pos.0, mouse_pos.1);
                                    if let Some(tool) = toolbar_tool {
                                        let should_exit = state.handle_toolbar_click(tool);
                                        state.window.request_redraw();
                                        if should_exit {
                                            event_loop.exit();
                                            return;
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
                    let new_pos = (position.x as f32, position.y as f32);
                    if let Some((last_x, last_y)) = state.mouse_position {
                        let dx = new_pos.0 - last_x;
                        let dy = new_pos.1 - last_y;
                        let distance = (dx * dx + dy * dy).sqrt();

                        // 只有移动距离超过阈值才处理（特别对画笔有用）
                        if distance < 0.5 && self.drag_mode == DragMode::None {
                            return;
                        }
                    }
                    let old_hovered = state.hovered_button;
                    state.update_mouse_position(position.x as f32, position.y as f32);

                    if old_hovered != state.hovered_button {
                        state.window.request_redraw();
                    }
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

                        let toolbar_button_exists =
                            state.get_toolbar_button_at(mouse_x, mouse_y).is_some();
                        let current_box = self.current_box;
                        let handle_size = state.handle_size;
                        let toolbar_active = state.toolbar_active;

                        if toolbar_button_exists {
                            state.window.set_cursor(winit::window::CursorIcon::Pointer);
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

                WindowEvent::KeyboardInput { event, .. } => {
                    use winit::keyboard::{KeyCode, PhysicalKey};

                    if event.state == winit::event::ElementState::Pressed {
                        match event.physical_key {
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
