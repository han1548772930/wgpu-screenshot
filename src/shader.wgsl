struct Uniforms {
    box_min: vec2<f32>,           // 8 bytes (索引0-1)
    box_max: vec2<f32>,           // 8 bytes (索引2-3)
    screen_size: vec2<f32>,       // 8 bytes (索引4-5)
    border_width: f32,            // 4 bytes (索引6)
    handle_size: f32,             // 4 bytes (索引7)
    handle_border_width: f32,     // 4 bytes (索引8)
    _padding0: f32,               // 4 bytes (索引9) - 必须添加！
    border_color: vec4<f32>,      // 16 bytes (索引10-13)
    handle_color: vec4<f32>,      // 16 bytes (索引14-17)
}

@group(0) @binding(0)
var t_texture: texture_2d<f32>;
@group(0) @binding(1)
var s_sampler: sampler;
@group(0) @binding(2)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@location(0) position: vec4<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(position.xy, 0.0, 1.0);
    out.tex_coords = position.zw;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let original_color = textureSample(t_texture, s_sampler, in.tex_coords);
    
    // 简单的有效性检查
    let has_box = uniforms.box_min.x >= 0.0 && uniforms.box_min.y >= 0.0;

    if !has_box {
        // 没有框时，全屏暗化
        return original_color * 0.3;
    }
    
    // 计算当前像素的屏幕坐标
    let screen_pos = in.tex_coords * uniforms.screen_size;
    
    // 检查是否在框内
    let in_box = all(screen_pos >= uniforms.box_min) && all(screen_pos <= uniforms.box_max);

    if in_box {
        // 框的中心和尺寸
        let box_center = (uniforms.box_min + uniforms.box_max) * 0.5;
        
        // 8个调整手柄的位置
        let handle_positions = array<vec2<f32>, 8>(
            vec2<f32>(uniforms.box_min.x, uniforms.box_min.y), // 左上
            vec2<f32>(box_center.x, uniforms.box_min.y),       // 上中
            vec2<f32>(uniforms.box_max.x, uniforms.box_min.y), // 右上
            vec2<f32>(uniforms.box_max.x, box_center.y),       // 右中
            vec2<f32>(uniforms.box_max.x, uniforms.box_max.y), // 右下
            vec2<f32>(box_center.x, uniforms.box_max.y),       // 下中
            vec2<f32>(uniforms.box_min.x, uniforms.box_max.y), // 左下
            vec2<f32>(uniforms.box_min.x, box_center.y)        // 左中
        );
        
        // 检查是否在任何手柄内
        var in_handle = false;
        var handle_border = false;

        for (var i = 0; i < 8; i++) {
            let handle_min = handle_positions[i] - vec2<f32>(uniforms.handle_size * 0.5);
            let handle_max = handle_positions[i] + vec2<f32>(uniforms.handle_size * 0.5);

            if all(screen_pos >= handle_min) && all(screen_pos <= handle_max) {
                in_handle = true;
                
                // 使用可配置的手柄边框宽度
                let inner_min = handle_min + vec2<f32>(uniforms.handle_border_width);
                let inner_max = handle_max - vec2<f32>(uniforms.handle_border_width);

                if all(screen_pos >= inner_min) && all(screen_pos <= inner_max) {
                    // 在手柄内部，显示白色背景
                    handle_border = false;
                } else {
                    // 在手柄边框区域，显示手柄颜色
                    handle_border = true;
                }
                break;
            }
        }

        if in_handle {
            if handle_border {
                // 临时调试：直接返回青色
                // return vec4<f32>(0.0, 1.0, 1.0, 1.0);
                return vec4<f32>(uniforms.handle_color.xyz, 1.0);
            } else {
                // 手柄内部：白色
                return vec4<f32>(1.0, 1.0, 1.0, 1.0);
            }
        } else {
            // 检查主边框
            let dist_to_left = screen_pos.x - uniforms.box_min.x;
            let dist_to_right = uniforms.box_max.x - screen_pos.x;
            let dist_to_top = screen_pos.y - uniforms.box_min.y;
            let dist_to_bottom = uniforms.box_max.y - screen_pos.y;

            let min_dist = min(min(dist_to_left, dist_to_right), min(dist_to_top, dist_to_bottom));

            if min_dist <= uniforms.border_width {
                // 临时调试：直接返回红色
                // return vec4<f32>(1.0, 0.0, 0.0, 1.0);
                return vec4<f32>(uniforms.border_color.xyz, 1.0);
            } else {
                // 框内原色
                return original_color;
            }
        }
    } else {
        // 框外暗化
        return original_color * 0.3;
    }
}