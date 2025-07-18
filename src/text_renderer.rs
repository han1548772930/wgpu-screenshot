use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache,
    TextArea, TextAtlas, TextBounds, TextRenderer as GlyphonTextRenderer, Viewport,
};

/// 光标字符常量
pub const CURSOR_CHAR: char = '|';

/// Text renderer wrapper for glyphon
pub struct TextRenderer {
    pub font_system: FontSystem,
    pub cache: Cache,
    pub swash_cache: SwashCache,
    pub atlas: TextAtlas,
    pub text_renderer: GlyphonTextRenderer,
    pub viewport: Viewport,
}

impl TextRenderer {
    /// Create a new text renderer
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // 🚀 内存优化：创建空的字体系统，不自动加载系统字体
        let mut font_system = FontSystem::new_with_locale_and_db(
            "en-US".to_string(),
            glyphon::fontdb::Database::new(), // 使用空的字体数据库
        );

        // 🚀 加载支持中文的字体
        let dejavu_font_data = include_bytes!("../fonts/NotoSerifCJKsc-VF.ttf");
        println!(
            "🚀 加载 DejaVu Sans 字体，大小: {} 字节",
            dejavu_font_data.len()
        );
        font_system
            .db_mut()
            .load_font_data(dejavu_font_data.to_vec());

        // 🚀 加载表情符号字体
        let emoji_font_data = include_bytes!("../fonts/SegoeUIEmoji.ttf");
        println!(
            "🚀 加载 Segoe UI Emoji 字体，大小: {} 字节",
            emoji_font_data.len()
        );
        font_system
            .db_mut()
            .load_font_data(emoji_font_data.to_vec());

        // 检查字体是否加载成功
        let font_count = font_system.db().len();
        println!("🚀 字体数据库中的字体数量: {}", font_count);

        // Create cache and atlas
        let cache = Cache::new(device);
        let swash_cache = SwashCache::new();
        let mut atlas = TextAtlas::new(device, queue, &cache, format);

        // Create text renderer
        let text_renderer =
            GlyphonTextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);

        // Create viewport
        let viewport = Viewport::new(device, &cache);

        println!("🚀 文本渲染器初始化完成");

        Ok(Self {
            font_system,
            cache,
            swash_cache,
            atlas,
            text_renderer,
            viewport,
        })
    }

    /// Resize the text renderer view
    pub fn resize(&mut self, width: u32, height: u32, queue: &wgpu::Queue) {
        self.viewport.update(queue, Resolution { width, height });
    }

    /// Prepare text areas for rendering
    pub fn prepare<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text_areas: impl IntoIterator<Item = TextArea<'a>>,
    ) -> Result<(), glyphon::PrepareError> {
        self.text_renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            text_areas,
            &mut self.swash_cache,
        )
    }

    /// Render the prepared text
    pub fn render<'rpass>(
        &'rpass self,
        render_pass: &mut wgpu::RenderPass<'rpass>,
    ) -> Result<(), glyphon::RenderError> {
        self.text_renderer
            .render(&self.atlas, &self.viewport, render_pass)
    }

    /// Create a text buffer with the given text
    pub fn create_buffer(&mut self, text: &str, font_size: f32, width: f32, height: f32) -> Buffer {
        // 🚀 使用相对行高，更符合 glyphon 最佳实践
        let metrics = Metrics::relative(font_size, 1.2); // 1.2倍行高
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        // 🚀 修复：为多行文本设置合适的缓冲区大小
        let buffer_width = width.max(200.0); // 最小宽度200像素
        let buffer_height = if text.contains('\n') {
            // 多行文本需要更多高度
            height.max(font_size * 1.2 * text.matches('\n').count() as f32 + font_size * 2.0)
        } else {
            height.max(font_size * 2.0) // 单行文本的最小高度
        };

        buffer.set_size(
            &mut self.font_system,
            Some(buffer_width),
            Some(buffer_height),
        );

        // 🚀 使用支持中文的字体属性 - 使用 DejaVu Sans 作为主要字体
        buffer.set_text(
            &mut self.font_system,
            text,
            &Attrs::new().family(Family::Name("DejaVu Sans")),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        println!(
            "🚀 创建文本缓冲区: 文本='{:?}', 大小={}x{}, 行数={}",
            text,
            buffer_width,
            buffer_height,
            text.matches('\n').count() + 1
        );
        buffer
    }
}
