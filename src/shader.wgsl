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
    toolbar_active: f32,          // 4 bytes (索引12) - 工具栏是否激活
    selected_button: f32,         // 4 bytes (索引13) - 选中的按钮索引
    _padding: vec2<f32>,          // 8 bytes (索引14-15) - 对齐到16字节
    border_color: vec4<f32>,      // 16 bytes (索引16-19)
    handle_color: vec4<f32>,      // 16 bytes (索引20-23)
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


fn draw_icon(button_index: f32, local_pos: vec2<f32>, button_size: f32, is_hovered: bool, is_selected: bool) -> vec4<f32> {
    let center = vec2<f32>(button_size * 0.5);
    let icon_size = 10.0;
    let pos_from_center = local_pos - center;
    let line_width = 1.5;
    let scaled_pos = pos_from_center;

    // 确定图标颜色：选中时为绿色，否则为灰黑色
    var icon_color: vec4<f32>;
    if is_selected {
        icon_color = vec4<f32>(0.2, 0.8, 0.2, 1.0); // 绿色
    } else {
        icon_color = vec4<f32>(0.2, 0.2, 0.2, 1.0); // 灰黑色
    }

    if button_index == 0.0 {
        // 矩形工具
        let rect_width = icon_size * 1.4;
        let rect_height = icon_size * 1.4;
        let corner_radius = icon_size * 0.2;

        let rect_half_size = vec2<f32>(rect_width * 0.5, rect_height * 0.5);
        let corner_pos = abs(scaled_pos) - rect_half_size + vec2<f32>(corner_radius);
        let corner_dist = length(max(corner_pos, vec2<f32>(0.0))) + min(max(corner_pos.x, corner_pos.y), 0.0) - corner_radius;

        if abs(corner_dist) <= line_width * 0.8 {
            return icon_color;
        }
    } else if button_index == 1.0 {
        // 圆形工具
        let circle_radius = icon_size * 0.85;
        let dist = length(scaled_pos);

        let outer_edge = circle_radius;
        let inner_edge = circle_radius - line_width;

        if dist >= inner_edge - 0.5 && dist <= outer_edge + 0.5 {
            var alpha = 1.0;
            if dist > outer_edge - 0.5 {
                alpha *= 1.0 - smoothstep(outer_edge - 0.5, outer_edge + 0.5, dist);
            }
            if dist < inner_edge + 0.5 {
                alpha *= smoothstep(inner_edge - 0.5, inner_edge + 0.5, dist);
            }
            if alpha > 0.1 {
                return vec4<f32>(icon_color.rgb, alpha);
            }
        }
    } else if button_index == 2.0 {
        // 箭头工具
        let arrow_size = icon_size * 1.5;
        let thickness = line_width;

        let h_line_start_x = -arrow_size * 0.15;
        let h_line_end_x = arrow_size * 0.4;
        let h_line_y = -arrow_size * 0.4;

        if abs(scaled_pos.y - h_line_y) <= thickness * 0.5 && scaled_pos.x >= h_line_start_x && scaled_pos.x <= h_line_end_x {
            return icon_color;
        }

        let v_line_x = arrow_size * 0.4;
        let v_line_start_y = -arrow_size * 0.4;
        let v_line_end_y = arrow_size * 0.1;

        if abs(scaled_pos.x - v_line_x) <= thickness * 0.5 && scaled_pos.y >= v_line_start_y && scaled_pos.y <= v_line_end_y {
            return icon_color;
        }

        let diag_start = vec2<f32>(arrow_size * 0.4, -arrow_size * 0.4);
        let diag_end = vec2<f32>(-arrow_size * 0.4, arrow_size * 0.4);

        let diag_dir = normalize(diag_end - diag_start);
        let to_point = scaled_pos - diag_start;
        let proj_length = dot(to_point, diag_dir);
        let closest_point = diag_start + diag_dir * proj_length;
        let dist_to_line = length(scaled_pos - closest_point);

        if proj_length >= 0.0 && proj_length <= length(diag_end - diag_start) && dist_to_line <= thickness * 0.5 {
            return icon_color;
        }
    } else if button_index == 3.0 {
        // 笔工具
        let pen_size = icon_size * 1.6;
        let thickness = line_width;

        let pen_start = vec2<f32>(pen_size * 0.4, -pen_size * 0.4);
        let pen_end = vec2<f32>(-pen_size * 0.3, pen_size * 0.3);

        let pen_dir = normalize(pen_end - pen_start);
        let to_point = scaled_pos - pen_start;
        let proj_length = dot(to_point, pen_dir);
        let closest_point = pen_start + pen_dir * proj_length;
        let dist_to_pen = length(scaled_pos - closest_point);

        let pen_length = length(pen_end - pen_start);
        if proj_length >= 0.0 && proj_length <= pen_length && dist_to_pen <= thickness * 0.6 {
            return icon_color;
        }

        let tip_apex = pen_end + pen_dir * pen_size * 0.15;
        let tip_width = pen_size * 0.08;

        let tip_side1 = pen_end + vec2<f32>(-pen_dir.y, pen_dir.x) * tip_width;
        let tip_side2 = pen_end + vec2<f32>(pen_dir.y, -pen_dir.x) * tip_width;

        let v0 = tip_side2 - tip_apex;
        let v1 = tip_side1 - tip_apex;
        let v2 = scaled_pos - tip_apex;

        let cross1 = v0.x * v2.y - v0.y * v2.x;
        let cross2 = v1.x * v2.y - v1.y * v2.x;
        let cross3 = (tip_side1.x - tip_side2.x) * (scaled_pos.y - tip_side2.y) - (tip_side1.y - tip_side2.y) * (scaled_pos.x - tip_side2.x);

        if (cross1 >= 0.0 && cross2 <= 0.0 && cross3 >= 0.0) || (cross1 <= 0.0 && cross2 >= 0.0 && cross3 <= 0.0) {
            return icon_color;
        }
    } else if button_index == 4.0 {
        // 文字工具
        let letter_size = icon_size * 1.2;
        let thickness = line_width;

        if abs(scaled_pos.x) <= thickness * 0.5 && scaled_pos.y >= -letter_size * 0.6 && scaled_pos.y <= letter_size * 0.6 {
            return icon_color;
        }
        if abs(scaled_pos.y + letter_size * 0.4) <= thickness * 0.5 && abs(scaled_pos.x) <= letter_size * 0.6 {
            return icon_color;
        }
        if abs(scaled_pos.x + letter_size * 0.6) <= thickness * 0.5 && scaled_pos.y >= -letter_size * 0.4 && scaled_pos.y <= -letter_size * 0.2 {
            return icon_color;
        }
        if abs(scaled_pos.x - letter_size * 0.6) <= thickness * 0.5 && scaled_pos.y >= -letter_size * 0.4 && scaled_pos.y <= -letter_size * 0.2 {
            return icon_color;
        }
        if abs(scaled_pos.y - letter_size * 0.6) <= thickness * 0.5 && abs(scaled_pos.x) <= letter_size * 0.3 {
            return icon_color;
        }
    } else if button_index == 5.0 {
        // 撤销工具
        let undo_size = icon_size * 2.0;
        let thickness = line_width;

        let arrow_tip = vec2<f32>(-undo_size * 0.4, 0.0);
        let arrow_top = vec2<f32>(-undo_size * 0.1, -undo_size * 0.3);
        let arrow_bottom = vec2<f32>(-undo_size * 0.1, undo_size * 0.3);

        let upper_dir = normalize(arrow_top - arrow_tip);
        let upper_to_point = scaled_pos - arrow_tip;
        let upper_proj = dot(upper_to_point, upper_dir);
        let upper_dist_along = clamp(upper_proj, 0.0, length(arrow_top - arrow_tip));
        let upper_closest = arrow_tip + upper_dir * upper_dist_along;
        let upper_dist = length(scaled_pos - upper_closest);

        if upper_dist <= thickness * 0.5 && upper_proj >= 0.0 && upper_proj <= length(arrow_top - arrow_tip) {
            return icon_color;
        }

        let lower_dir = normalize(arrow_bottom - arrow_tip);
        let lower_to_point = scaled_pos - arrow_tip;
        let lower_proj = dot(lower_to_point, lower_dir);
        let lower_dist_along = clamp(lower_proj, 0.0, length(arrow_bottom - arrow_tip));
        let lower_closest = arrow_tip + lower_dir * lower_dist_along;
        let lower_dist = length(scaled_pos - lower_closest);

        if lower_dist <= thickness * 0.5 && lower_proj >= 0.0 && lower_proj <= length(arrow_bottom - arrow_tip) {
            return icon_color;
        }

        if abs(scaled_pos.y) <= thickness * 0.5 && scaled_pos.x >= -undo_size * 0.1 && scaled_pos.x <= undo_size * 0.1 {
            return icon_color;
        }

        let curve_center = vec2<f32>(undo_size * 0.1, undo_size * 0.2);
        let curve_radius = undo_size * 0.2;
        let curve_dist = length(scaled_pos - curve_center);

        if scaled_pos.x >= curve_center.x - curve_radius * 0.1 {
            if abs(curve_dist - curve_radius) <= thickness * 0.5 {
                return icon_color;
            }
        }

        let end_x = curve_center.x + curve_radius;
        if abs(scaled_pos.x - end_x) <= thickness * 0.5 && scaled_pos.y >= curve_center.y && scaled_pos.y <= curve_center.y + undo_size * 0.2 {
            return icon_color;
        }
    } else if button_index == 6.0 {
        // 保存工具
        let download_size = icon_size * 1.6;
        let thickness = line_width;

        if abs(scaled_pos.x) <= thickness * 0.5 && scaled_pos.y >= -download_size * 0.5 && scaled_pos.y <= download_size * 0.1 {
            return icon_color;
        }

        let arrow_tip = vec2<f32>(0.0, download_size * 0.1);
        let arrow_left = vec2<f32>(-download_size * 0.3, -download_size * 0.2);
        let arrow_right = vec2<f32>(download_size * 0.3, -download_size * 0.2);

        let left_dir = normalize(arrow_tip - arrow_left);
        let left_to_point = scaled_pos - arrow_left;
        let left_proj = clamp(dot(left_to_point, left_dir) / length(arrow_tip - arrow_left), 0.0, 1.0);
        let left_closest = arrow_left + left_dir * left_proj * length(arrow_tip - arrow_left);

        if length(scaled_pos - left_closest) <= thickness * 0.5 {
            return icon_color;
        }

        let right_dir = normalize(arrow_tip - arrow_right);
        let right_to_point = scaled_pos - arrow_right;
        let right_proj = clamp(dot(right_to_point, right_dir) / length(arrow_tip - arrow_right), 0.0, 1.0);
        let right_closest = arrow_right + right_dir * right_proj * length(arrow_tip - arrow_right);

        if length(scaled_pos - right_closest) <= thickness * 0.5 {
            return icon_color;
        }

        let container_y = download_size * 0.3;
        let container_width = download_size * 0.8;
        let container_height = download_size * 0.25;

        if abs(scaled_pos.y - container_y) <= thickness * 0.5 && abs(scaled_pos.x) <= container_width * 0.5 {
            return icon_color;
        }
        if abs(scaled_pos.x + container_width * 0.5) <= thickness * 0.5 && scaled_pos.y >= container_y && scaled_pos.y <= container_y + container_height {
            return icon_color;
        }
        if abs(scaled_pos.x - container_width * 0.5) <= thickness * 0.5 && scaled_pos.y >= container_y && scaled_pos.y <= container_y + container_height {
            return icon_color;
        }
        if abs(scaled_pos.y - (container_y + container_height)) <= thickness * 0.5 && abs(scaled_pos.x) <= container_width * 0.5 {
            return icon_color;
        }
    } else if button_index == 7.0 {
        // 退出按钮
        let x_size = icon_size * 0.7;
        let thickness = line_width;

        let diag1_dist = abs(scaled_pos.x - scaled_pos.y) / 1.414;
        if diag1_dist <= thickness && abs(scaled_pos.x) <= x_size && abs(scaled_pos.y) <= x_size {
            return icon_color;
        }

        let diag2_dist = abs(scaled_pos.x + scaled_pos.y) / 1.414;
        if diag2_dist <= thickness && abs(scaled_pos.x) <= x_size && abs(scaled_pos.y) <= x_size {
            return icon_color;
        }
    } else if button_index == 8.0 {
        // 完成按钮
        let check_size = icon_size * 1.8;
        let thickness = line_width;

        let start_point = vec2<f32>(-check_size * 0.4, 0.0);
        let turn_point = vec2<f32>(-check_size * 0.1, check_size * 0.3);
        let end_point = vec2<f32>(check_size * 0.4, -check_size * 0.4);

        let seg1_dir = normalize(turn_point - start_point);
        let seg1_length = length(turn_point - start_point);
        let seg1_to_point = scaled_pos - start_point;
        let seg1_proj = clamp(dot(seg1_to_point, seg1_dir), 0.0, seg1_length);
        let seg1_closest = start_point + seg1_dir * seg1_proj;

        if length(scaled_pos - seg1_closest) <= thickness * 0.5 && seg1_proj >= 0.0 && seg1_proj <= seg1_length {
            return icon_color;
        }

        let seg2_dir = normalize(end_point - turn_point);
        let seg2_length = length(end_point - turn_point);
        let seg2_to_point = scaled_pos - turn_point;
        let seg2_proj = clamp(dot(seg2_to_point, seg2_dir), 0.0, seg2_length);
        let seg2_closest = turn_point + seg2_dir * seg2_proj;

        if length(scaled_pos - seg2_closest) <= thickness * 0.5 && seg2_proj >= 0.0 && seg2_proj <= seg2_length {
            return icon_color;
        }
    }

    return vec4<f32>(0.0, 0.0, 0.0, 0.0); // 透明
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
    
    // 检查工具栏区域
    if uniforms.show_toolbar > 0.5 {
        // 计算工具栏宽度（9个按钮，每个40px，间距10px）
        let toolbar_width = 9.0 * 40.0 + 8.0 * 10.0;
        
        // 首先尝试在框的下方
        var toolbar_y = uniforms.box_max.y + 5.0;
        let toolbar_bottom = toolbar_y + uniforms.toolbar_height + 30.0;
        
        // 如果超出屏幕下边界，移到框的上方
        if toolbar_bottom > uniforms.screen_size.y {
            toolbar_y = uniforms.box_min.y - uniforms.toolbar_height - 10.0;
            
            // 如果移到上方还是超出屏幕，则放在屏幕顶部
            if toolbar_y < 0.0 {
                toolbar_y = 10.0;
            }
        }

        let final_toolbar_bottom = toolbar_y + uniforms.toolbar_height;

        if screen_pos.y >= toolbar_y && screen_pos.y <= final_toolbar_bottom {
            // 调整X坐标，确保工具栏不超出屏幕边界
            var toolbar_start_x = uniforms.box_min.x;
            if toolbar_start_x + toolbar_width > uniforms.screen_size.x {
                toolbar_start_x = max(uniforms.screen_size.x - toolbar_width, 0.0);
            } else {
                toolbar_start_x = max(toolbar_start_x, 0.0);
            }

            let toolbar_right = toolbar_start_x + toolbar_width;

            if screen_pos.x >= toolbar_start_x && screen_pos.x <= toolbar_right {
                // 计算当前是第几个按钮
                let button_pos = (screen_pos.x - toolbar_start_x) / (40.0 + 10.0);
                let button_index = floor(button_pos);

                if button_index < 9.0 {
                    let button_start_x = toolbar_start_x + button_index * (40.0 + 10.0);
                    let button_end_x = button_start_x + 40.0;

                    if screen_pos.x >= button_start_x && screen_pos.x <= button_end_x {
                        // 在按钮区域内
                        let local_pos = vec2<f32>(screen_pos.x - button_start_x, screen_pos.y - toolbar_y);
                        let is_hovered = abs(uniforms.hovered_button - button_index) < 0.5;
                        let is_selected = abs(uniforms.selected_button - button_index) < 0.5;
                        
                        // 绘制图标
                        let icon_color = draw_icon(button_index, local_pos, 40.0, is_hovered, is_selected);
                        if icon_color.a > 0.0 {
                            return icon_color;
                        }
                        
                        // 按钮背景处理
                        let center = vec2<f32>(20.0, 20.0);
                        let dist_to_center = length(local_pos - center);
                        let hover_radius = 18.0;
                        
                        if is_selected {
                            // 选中状态的背景
                            if dist_to_center <= hover_radius {
                                return vec4<f32>(0.9, 1.0, 0.9, 1.0); // 淡绿色背景
                            }
                        } else if is_hovered && dist_to_center <= hover_radius {
                            return vec4<f32>(0.85, 0.85, 0.85, 1.0); // 悬停时淡灰色背景
                        }
                        
                        // 按钮背景 - 默认白色
                        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
                    }
                }
                
                // 工具栏背景（按钮之间的间隙）
                return vec4<f32>(1.0, 1.0, 1.0, 0.9);
            }
        }
    }
    
    // 检查是否在框内
    let in_box = all(screen_pos >= uniforms.box_min) && all(screen_pos <= uniforms.box_max);

    if in_box {
        // 框的中心和尺寸
        let box_center = (uniforms.box_min + uniforms.box_max) * 0.5;
        
        // 只有在工具栏未激活时才绘制手柄
        if uniforms.toolbar_active < 0.5 {
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
                    
                    // 检查是否在手柄边框区域
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
                    // 手柄边框：使用手柄颜色
                    return vec4<f32>(uniforms.handle_color.rgb, 1.0);
                } else {
                    // 手柄内部：显示白色背景
                    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
                }
            }
        }
        
        // 检查主边框
        let dist_to_left = screen_pos.x - uniforms.box_min.x;
        let dist_to_right = uniforms.box_max.x - screen_pos.x;
        let dist_to_top = screen_pos.y - uniforms.box_min.y;
        let dist_to_bottom = uniforms.box_max.y - screen_pos.y;

        let min_dist = min(min(dist_to_left, dist_to_right), min(dist_to_top, dist_to_bottom));

        if min_dist <= uniforms.border_width {
            // 边框：使用边框颜色
            return vec4<f32>(uniforms.border_color.rgb, 1.0);
        } else {
            // 框内原色
            return original_color;
        }
    } else {
        // 框外暗化
        return original_color * 0.3;
    }
}