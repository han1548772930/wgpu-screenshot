struct Uniforms {
    box_min: vec2<f32>,           // 8 bytes (索引0-1)
    box_max: vec2<f32>,           // 8 bytes (索引2-3)
    screen_size: vec2<f32>,       // 8 bytes (索引4-5)
    border_width: f32,            // 4 bytes (索引6)
    handle_size: f32,             // 4 bytes (索引7)
    handle_border_width: f32,     // 4 bytes (索引8)
    show_toolbar: f32,            // 4 bytes (索引9)
    toolbar_height: f32,          // 4 bytes (索引10)
    hovered_button: f32,          // 4 bytes (索引11)
    toolbar_active: f32,          // 4 bytes (索引12)
    selected_button: f32,         // 4 bytes (索引13)
    toolbar_button_size: f32,     // 4 bytes (索引14)
    toolbar_button_margin: f32,   // 4 bytes (索引15)
    border_color: vec4<f32>,      // 16 bytes (索引16-19)
    handle_color: vec4<f32>,      // 16 bytes (索引20-23)
    toolbar_button_count: f32,    // 4 bytes (索引24)
    // 🚀 背景缓存控制参数
    background_cache_valid: f32,  // 4 bytes (索引25) - 背景缓存是否有效
    force_background_update: f32, // 4 bytes (索引26) - 强制更新背景缓存
    // 🚀 绘图元素手柄参数
    show_handles: f32,           // 4 bytes (索引27) - 是否显示手柄
    // 🚀 撤销按钮状态
    undo_button_enabled: f32,    // 4 bytes (索引28) - 撤销按钮是否启用
}

@group(0) @binding(0)
var t_texture: texture_2d<f32>;
@group(0) @binding(1)
var s_sampler: sampler;
@group(0) @binding(2)
var<uniform> uniforms: Uniforms;

// 🚀 背景缓存纹理 - 存储预渲染的背景（可选绑定）
// 注意：这些绑定只在主渲染管道中使用，背景缓存管道不使用

struct VertexInput {
    @location(0) position: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}
struct DrawingVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) thickness: f32,
}
// 🔧 WGSL GPU优化：简化绘图顶点着色器，减少计算量
@vertex
fn vs_drawing(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>, @location(2) thickness: f32) -> DrawingVertexOutput {
    var out: DrawingVertexOutput;
    // 🔧 GPU优化：直接使用位置，避免额外变换
    out.clip_position = vec4<f32>(position, 0.0, 1.0);
    // 🔧 GPU优化：预计算颜色，减少片段着色器负载
    out.color = color;
    // 🔧 GPU优化：简化厚度处理
    out.thickness = max(thickness, 1.0); // 确保最小厚度，避免过细线条的复杂计算
    return out;
}
@vertex
fn vs_main(@location(0) position: vec4<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(position.xy, 0.0, 1.0);
    out.tex_coords = position.zw;
    return out;
}

@vertex
fn vs_icon(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(vertex.position.xy, 0.0, 1.0);
    out.tex_coords = vertex.position.zw;
    return out;
}
// 🔧 GPU优化：使用compute shader优化的绘图片段着色器
@fragment
fn fs_drawing(in: DrawingVertexOutput) -> @location(0) vec4<f32> {
    // 直接返回颜色，compute shader已经处理了复杂计算
    return in.color;
}

// 🔧 GPU优化：添加compute shader支持的存储缓冲区结构
struct PenPointData {
    position: vec2<f32>,
    color: vec4<f32>,
    thickness: f32,
    _padding: f32, // 对齐到16字节
}

// 🔧 GPU优化：画笔点存储缓冲区（参考您的模式）
@group(1) @binding(0)
var<storage, read_write> pen_points: array<PenPointData>;

// 🔧 GPU优化：画笔处理的compute shader
@compute @workgroup_size(64, 1, 1)
fn cs_process_pen_points(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    // 边界检查
    if index >= arrayLength(&pen_points) {
        return;
    }

    // 简单的点处理 - 可以在这里添加更复杂的优化
    // 例如：距离过滤、平滑处理等
    let point = pen_points[index];

    // 这里可以添加GPU并行的点处理逻辑
    // 目前保持简单，直接传递数据
    pen_points[index] = point;
}
// 🚀 背景缓存系统 - 智能缓存管理
var<private> cached_background_state: vec4<f32> = vec4<f32>(-1.0); // x: box_hash, y: toolbar_state, z: cache_valid, w: reserved
var<private> cached_background_color: vec4<f32> = vec4<f32>(0.0);  // 缓存的背景颜色

// 🚀 绘图元素缓存系统 - 缓存几何图形的计算结果
var<private> cached_circle_vertices: array<vec2<f32>, 64>;  // 缓存圆形顶点
var<private> cached_circle_params: vec4<f32> = vec4<f32>(-1.0); // center.xy, radius, segments
var<private> cached_rectangle_vertices: array<vec2<f32>, 8>; // 缓存矩形顶点 (4条边，每条2个点)
var<private> cached_rectangle_params: vec4<f32> = vec4<f32>(-1.0); // start.xy, end.xy
var<private> cached_arrow_vertices: array<vec2<f32>, 12>; // 缓存箭头顶点 (主线2个点 + 箭头6个点 + 4个备用)
var<private> cached_arrow_params: vec4<f32> = vec4<f32>(-1.0); // start.xy, end.xy

// 优化的工具栏计算函数 - 使用缓存优化，减少重复计算
var<private> cached_toolbar_layout: vec4<f32> = vec4<f32>(-1.0);
var<private> cached_box_coords: vec4<f32> = vec4<f32>(-2.0);

// 🚀 背景状态哈希计算 - 用于检测背景是否需要更新
fn calculate_background_hash() -> f32 {
    // 基于关键参数计算简单哈希
    let box_hash = uniforms.box_min.x + uniforms.box_min.y * 1000.0 + uniforms.box_max.x * 10000.0 + uniforms.box_max.y * 100000.0;
    let toolbar_hash = uniforms.show_toolbar * 1000000.0 + uniforms.toolbar_active * 2000000.0;
    return box_hash + toolbar_hash;
}

// 🚀 智能背景缓存检查 - 判断是否可以使用缓存的背景
fn is_background_cache_valid() -> bool {
    let current_hash = calculate_background_hash();
    let toolbar_state = uniforms.show_toolbar + uniforms.toolbar_active * 10.0;

    // 检查缓存是否有效
    if uniforms.background_cache_valid > 0.0 && abs(cached_background_state.x - current_hash) < 0.1 && abs(cached_background_state.y - toolbar_state) < 0.1 && uniforms.force_background_update < 0.5 {
        return true;
    }

    // 更新缓存状态
    cached_background_state.x = current_hash;
    cached_background_state.y = toolbar_state;
    cached_background_state.z = 1.0; // 标记为有效

    return false;
}

// 🚀 缓存的圆形顶点计算 - 避免重复三角函数计算
fn get_cached_circle_vertices(center: vec2<f32>, radius: f32, segments: f32) -> array<vec2<f32>, 64> {
    let current_params = vec4<f32>(center.x, center.y, radius, segments);

    // 检查缓存是否有效
    if all(abs(cached_circle_params - current_params) < vec4<f32>(0.1)) {
        return cached_circle_vertices;
    }

    // 重新计算并缓存圆形顶点
    let seg_count = i32(segments);
    for (var i = 0; i < seg_count && i < 64; i++) {
        let angle = (f32(i) * 2.0 * 3.14159265) / segments;
        cached_circle_vertices[i] = center + vec2<f32>(cos(angle) * radius, sin(angle) * radius);
    }

    // 更新缓存参数
    cached_circle_params = current_params;
    return cached_circle_vertices;
}

// 🚀 缓存的矩形顶点计算 - 避免重复边界计算
fn get_cached_rectangle_vertices(start: vec2<f32>, end: vec2<f32>) -> array<vec2<f32>, 8> {
    let current_params = vec4<f32>(start.x, start.y, end.x, end.y);

    // 检查缓存是否有效
    if all(abs(cached_rectangle_params - current_params) < vec4<f32>(0.1)) {
        return cached_rectangle_vertices;
    }

    // 重新计算并缓存矩形顶点 (4条边，每条2个点)
    cached_rectangle_vertices[0] = vec2<f32>(start.x, start.y); // 上边起点
    cached_rectangle_vertices[1] = vec2<f32>(end.x, start.y);   // 上边终点
    cached_rectangle_vertices[2] = vec2<f32>(end.x, start.y);   // 右边起点
    cached_rectangle_vertices[3] = vec2<f32>(end.x, end.y);     // 右边终点
    cached_rectangle_vertices[4] = vec2<f32>(end.x, end.y);     // 下边起点
    cached_rectangle_vertices[5] = vec2<f32>(start.x, end.y);   // 下边终点
    cached_rectangle_vertices[6] = vec2<f32>(start.x, end.y);   // 左边起点
    cached_rectangle_vertices[7] = vec2<f32>(start.x, start.y); // 左边终点

    // 更新缓存参数
    cached_rectangle_params = current_params;
    return cached_rectangle_vertices;
}

// 🚀 缓存的箭头顶点计算 - 避免重复向量计算
fn get_cached_arrow_vertices(start: vec2<f32>, end: vec2<f32>) -> array<vec2<f32>, 12> {
    let current_params = vec4<f32>(start.x, start.y, end.x, end.y);

    // 检查缓存是否有效
    if all(abs(cached_arrow_params - current_params) < vec4<f32>(0.1)) {
        return cached_arrow_vertices;
    }

    // 重新计算并缓存箭头顶点
    // 主线
    cached_arrow_vertices[0] = start;
    cached_arrow_vertices[1] = end;

    // 计算箭头
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len = sqrt(dx * dx + dy * dy);

    if len > 0.0 {
        let ux = dx / len;
        let uy = dy / len;
        let arrow_len = 15.0;
        let arrow_width = 8.0;

        let p1 = end - vec2<f32>(arrow_len * ux - arrow_width * uy, arrow_len * uy + arrow_width * ux);
        let p2 = end - vec2<f32>(arrow_len * ux + arrow_width * uy, arrow_len * uy - arrow_width * ux);

        // 箭头线段
        cached_arrow_vertices[2] = end;
        cached_arrow_vertices[3] = p1;
        cached_arrow_vertices[4] = end;
        cached_arrow_vertices[5] = p2;

        // 预留位置用于其他箭头样式
        for (var i = 6; i < 12; i++) {
            cached_arrow_vertices[i] = end;
        }
    }

    // 更新缓存参数
    cached_arrow_params = current_params;
    return cached_arrow_vertices;
}

fn calculate_toolbar_layout() -> vec4<f32> {
    let current_box = vec4<f32>(uniforms.box_min, uniforms.box_max);

    // 检查缓存是否有效
    if all(cached_box_coords == current_box) && cached_toolbar_layout.x >= 0.0 {
        return cached_toolbar_layout;
    }

    // 重新计算并缓存
    let toolbar_width = uniforms.toolbar_button_count * uniforms.toolbar_button_size + (uniforms.toolbar_button_count - 1.0) * uniforms.toolbar_button_margin;

    var toolbar_y = uniforms.box_max.y + 5.0;
    let toolbar_bottom = toolbar_y + uniforms.toolbar_height;

    if toolbar_bottom > uniforms.screen_size.y {
        toolbar_y = uniforms.box_min.y - uniforms.toolbar_height - 10.0;
        if toolbar_y < 0.0 {
            toolbar_y = 10.0;
        }
    }

    var toolbar_start_x = uniforms.box_min.x;
    if toolbar_start_x + toolbar_width > uniforms.screen_size.x {
        toolbar_start_x = max(uniforms.screen_size.x - toolbar_width, 0.0);
    } else {
        toolbar_start_x = max(toolbar_start_x, 0.0);
    }

    // 更新缓存
    cached_box_coords = current_box;
    cached_toolbar_layout = vec4<f32>(toolbar_start_x, toolbar_y, toolbar_width, uniforms.toolbar_height);

    return cached_toolbar_layout;
}

// 修复图标着色器的悬停背景
@fragment
fn fs_icon(in: VertexOutput) -> @location(0) vec4<f32> {
    let icon_color = textureSample(t_texture, s_sampler, in.tex_coords);

    let screen_pos = in.clip_position.xy;
    let toolbar_layout = calculate_toolbar_layout();
    
    // 解构工具栏布局
    let toolbar_start_x = toolbar_layout.x;
    let toolbar_y = toolbar_layout.y;
    let toolbar_width = toolbar_layout.z;
    
    // 快速边界检查
    if screen_pos.y < toolbar_y || screen_pos.y > toolbar_y + uniforms.toolbar_height || screen_pos.x < toolbar_start_x || screen_pos.x > toolbar_start_x + toolbar_width {
        // 不在工具栏区域，应用alpha混合
        if icon_color.a < 0.1 {
            discard;
        }
        return icon_color;
    }
    
    // 计算按钮索引（只在确定在工具栏内时计算）
    let button_spacing = uniforms.toolbar_button_size + uniforms.toolbar_button_margin;
    let button_index = floor((screen_pos.x - toolbar_start_x) / button_spacing);
    
    // 边界检查
    if button_index < 0.0 || button_index >= uniforms.toolbar_button_count {
        if icon_color.a < 0.1 {
            discard;
        }
        return icon_color;
    }
    
    // 精确按钮区域检查
    let button_x_start = toolbar_start_x + button_index * button_spacing;
    let button_y_offset = (uniforms.toolbar_height - uniforms.toolbar_button_size) * 0.5;
    let button_y_start = toolbar_y + button_y_offset;

    let in_button = screen_pos.x >= button_x_start && screen_pos.x <= button_x_start + uniforms.toolbar_button_size && screen_pos.y >= button_y_start && screen_pos.y <= button_y_start + uniforms.toolbar_button_size;

    if !in_button {
        if icon_color.a < 0.1 {
            discard;
        }
        return icon_color;
    }
    
    // 在按钮区域内，检查状态
    let is_selected = abs(uniforms.selected_button - button_index) < 0.5;
    let is_hovered = abs(uniforms.hovered_button - button_index) < 0.5;

    // 🚀 检查是否是撤销按钮（索引5）
    let is_undo_button = abs(button_index - 5.0) < 0.5;

    if is_undo_button {
        // 🚀 使用专门的uniform来判断撤销按钮状态
        let undo_enabled = uniforms.undo_button_enabled > 0.5;

        if undo_enabled {
            // 🚀 启用状态：正常显示
            if is_hovered {
                // 悬停时稍微增加亮度
                if icon_color.a > 0.1 {
                    return vec4<f32>(icon_color.rgb * 1.1, icon_color.a);
                } else {
                    return vec4<f32>(0.7, 0.7, 0.7, 0.8);
                }
            } else {
                // 正常状态
                if icon_color.a < 0.1 {
                    discard;
                }
                return icon_color;
            }
        } else {
            // 🚀 禁用状态：显示灰色
            if icon_color.a > 0.1 {
                // 图标区域：变为灰色，降低透明度
                let gray_value = dot(icon_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
                return vec4<f32>(gray_value * 0.5, gray_value * 0.5, gray_value * 0.5, icon_color.a * 0.7);
            } else {
                // 透明区域：保持透明
                discard;
            }
        }
    } else if is_selected {
        // 选中状态：只改变图标颜色，不改变背景
        if icon_color.a > 0.1 {
            // 图标区域：变为绿色调
            let luminance = dot(icon_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
            return vec4<f32>(0.2 + luminance * 0.3, 0.8, 0.2 + luminance * 0.3, icon_color.a);
        } else {
            // 透明区域：保持透明，不显示背景
            discard;
        }
    } else if is_hovered {
        // 悬停状态：显示灰色背景
        if icon_color.a > 0.1 {
            // 图标区域：稍微增加亮度
            return vec4<f32>(icon_color.rgb * 1.1, icon_color.a);
        } else {
            // 透明区域：显示灰色背景
            return vec4<f32>(0.7, 0.7, 0.7, 0.8);
        }
    } else {
        // 普通状态，透明区域discard
        if icon_color.a < 0.1 {
            discard;
        }
    }

    return icon_color;
}

// 🚀 背景缓存着色器 - 专门用于渲染和缓存背景
@fragment
fn fs_background_cache(in: VertexOutput) -> @location(0) vec4<f32> {
    let original_color = textureSample(t_texture, s_sampler, in.tex_coords);

    // 🔧 GPU优化：早期退出，避免复杂计算
    if uniforms.box_min.x < 0.0 {
        cached_background_color = original_color * 0.3;
        return cached_background_color;
    }

    // 🔧 GPU优化：简化坐标计算
    let screen_pos = in.tex_coords * uniforms.screen_size;

    // 框区域检查
    let in_box_x = screen_pos.x >= uniforms.box_min.x && screen_pos.x <= uniforms.box_max.x;
    let in_box_y = screen_pos.y >= uniforms.box_min.y && screen_pos.y <= uniforms.box_max.y;
    let in_box = in_box_x && in_box_y;

    if !in_box {
        cached_background_color = original_color * 0.3;
        return cached_background_color;
    }

    // 在框内，返回原始颜色
    cached_background_color = original_color;
    return cached_background_color;
}

// 🚀 优化的主着色器 - 简化版本，暂时不使用背景缓存
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 🚀 智能背景缓存：检查是否可以使用缓存
    if is_background_cache_valid() {
        // 暂时直接执行完整渲染，后续可以添加缓存逻辑
        return fs_main_full_render(in);
    }

    // 缓存无效，执行完整渲染
    return fs_main_full_render(in);
}

// 🚀 完整渲染函数 - 当缓存无效时使用
fn fs_main_full_render(in: VertexOutput) -> vec4<f32> {
    let original_color = textureSample(t_texture, s_sampler, in.tex_coords);

    // 🔧 GPU优化：早期退出，避免复杂计算
    if uniforms.box_min.x < 0.0 {
        return original_color * 0.3;
    }

    // 🔧 GPU优化：简化坐标计算
    let screen_pos = in.tex_coords * uniforms.screen_size;

    // 🔧 WGSL GPU优化：简化工具栏渲染，减少分支和计算
    if uniforms.show_toolbar > 0.0 {
        let toolbar_layout = calculate_toolbar_layout();
        let toolbar_start_x = toolbar_layout.x;
        let toolbar_y = toolbar_layout.y;
        let toolbar_width = toolbar_layout.z;

        // 🔧 GPU优化：简化边界检查，减少条件分支
        let in_toolbar = screen_pos.y >= toolbar_y && screen_pos.y <= toolbar_y + uniforms.toolbar_height && screen_pos.x >= toolbar_start_x && screen_pos.x <= toolbar_start_x + toolbar_width;

        if in_toolbar {
            // 恢复完整的按钮检测
            let button_spacing = uniforms.toolbar_button_size + uniforms.toolbar_button_margin;
            let button_index = floor((screen_pos.x - toolbar_start_x) / button_spacing);

            if button_index < uniforms.toolbar_button_count {
                let button_start_x = toolbar_start_x + button_index * button_spacing;

                if screen_pos.x >= button_start_x && screen_pos.x <= button_start_x + uniforms.toolbar_button_size {
                    let button_y_offset = (uniforms.toolbar_height - uniforms.toolbar_button_size) * 0.5;
                    let button_start_y = toolbar_y + button_y_offset;

                    if screen_pos.y >= button_start_y && screen_pos.y <= button_start_y + uniforms.toolbar_button_size {
                        return vec4<f32>(1.0, 1.0, 1.0, 0.7);
                    }
                }
            }

            return vec4<f32>(1.0, 1.0, 1.0, 0.9);
        }
    }

    // 框区域检查
    let in_box_x = screen_pos.x >= uniforms.box_min.x && screen_pos.x <= uniforms.box_max.x;
    let in_box_y = screen_pos.y >= uniforms.box_min.y && screen_pos.y <= uniforms.box_max.y;
    let in_box = in_box_x && in_box_y;

    if !in_box {
        return original_color * 0.3;
    }

    return original_color + render_ui_overlay(screen_pos);
}

// 🚀 UI覆盖层渲染函数 - 渲染边框、手柄等动态元素
fn render_ui_overlay(screen_pos: vec2<f32>) -> vec4<f32> {
    // 恢复简单清晰的手柄渲染
    if uniforms.toolbar_active == 0.0 {
        let box_center = (uniforms.box_min + uniforms.box_max) * 0.5;
        let half_handle = uniforms.handle_size * 0.5;

        let handle_positions = array<vec2<f32>, 8>(
            uniforms.box_min,                                  // 左上
            vec2<f32>(box_center.x, uniforms.box_min.y),      // 上中
            vec2<f32>(uniforms.box_max.x, uniforms.box_min.y), // 右上
            vec2<f32>(uniforms.box_max.x, box_center.y),       // 右中
            uniforms.box_max,                                   // 右下
            vec2<f32>(box_center.x, uniforms.box_max.y),       // 下中
            vec2<f32>(uniforms.box_min.x, uniforms.box_max.y), // 左下
            vec2<f32>(uniforms.box_min.x, box_center.y)        // 左中
        );

        for (var i = 0; i < 8; i++) {
            let handle_pos = handle_positions[i];
            let dist = abs(screen_pos - handle_pos);

            if all(dist <= vec2<f32>(half_handle)) {
                let border_dist = half_handle - uniforms.handle_border_width;
                if all(dist <= vec2<f32>(border_dist)) {
                    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
                } else {
                    return vec4<f32>(uniforms.handle_color.rgb, 1.0);
                }
            }
        }
    }

    // 边框检查（优化版本）
    let border_dists = vec4<f32>(
        screen_pos.x - uniforms.box_min.x,      // 左
        uniforms.box_max.x - screen_pos.x,      // 右
        screen_pos.y - uniforms.box_min.y,      // 上
        uniforms.box_max.y - screen_pos.y       // 下
    );

    let min_dist = min(min(border_dists.x, border_dists.y), min(border_dists.z, border_dists.w));

    if min_dist <= uniforms.border_width {
        return vec4<f32>(uniforms.border_color.rgb, 1.0);
    }

    return vec4<f32>(0.0, 0.0, 0.0, 0.0); // 透明，无覆盖
}