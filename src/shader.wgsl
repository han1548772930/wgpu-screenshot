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
    _padding2: vec3<f32>,         // 12 bytes (索引25-27)
}

@group(0) @binding(0)
var t_texture: texture_2d<f32>;
@group(0) @binding(1)
var s_sampler: sampler;
@group(0) @binding(2)
var<uniform> uniforms: Uniforms;

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
@vertex
fn vs_drawing(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>, @location(2) thickness: f32) -> DrawingVertexOutput {
    var out: DrawingVertexOutput;
    out.clip_position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    out.thickness = thickness;
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
@fragment
fn fs_drawing(in: DrawingVertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
// 优化的工具栏计算函数
fn calculate_toolbar_layout() -> vec4<f32> {
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

    return vec4<f32>(toolbar_start_x, toolbar_y, toolbar_width, uniforms.toolbar_height);
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

    if is_selected {
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

// 优化的主着色器
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let original_color = textureSample(t_texture, s_sampler, in.tex_coords);
    
    // 早期退出：无效框
    if uniforms.box_min.x < 0.0 || uniforms.box_min.y < 0.0 {
        return original_color * 0.3;
    }

    let screen_pos = in.tex_coords * uniforms.screen_size;
    
    // 工具栏处理（提前计算，避免重复）
    if uniforms.show_toolbar > 0.5 {
        let toolbar_layout = calculate_toolbar_layout();
        let toolbar_start_x = toolbar_layout.x;
        let toolbar_y = toolbar_layout.y;
        let toolbar_width = toolbar_layout.z;

        if screen_pos.y >= toolbar_y && screen_pos.y <= toolbar_y + uniforms.toolbar_height && screen_pos.x >= toolbar_start_x && screen_pos.x <= toolbar_start_x + toolbar_width {
            
            // 按钮区域检查（优化版本）
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
    
    // 手柄处理（只有在需要时才计算）
    if uniforms.toolbar_active < 0.5 {
        let box_center = (uniforms.box_min + uniforms.box_max) * 0.5;
        let half_handle = uniforms.handle_size * 0.5;
        
        // 使用更高效的手柄位置数组
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
        
        // 展开循环以提高性能
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

    return original_color;
}