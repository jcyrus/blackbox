use crate::app::App;
use crate::app::{
    SYNTAX_SET, SYNTECT_THEME, TokenKind,
    next_markdown_token, parse_code_fence_language, syntect_to_ratatui,
};
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use syntect::easy::HighlightLines;

impl App {
    pub(crate) fn render_editor(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let top = self.buffer.viewport.top_line;
        let bottom = (top + area.height as usize).min(self.buffer.line_count());

        let needs_rebuild = self.render_cache.dirty
            || self.render_cache.top != top
            || self.render_cache.bottom != bottom;

        if needs_rebuild {
            let mut code_block_lang = self.code_block_lang_before_line(top);
            self.render_cache.lines = (top..bottom)
                .map(|i| {
                    let text = self.buffer.line_text(i).unwrap_or_default();
                    self.render_markdown_line(&text, &mut code_block_lang)
                })
                .collect();
            self.render_cache.top = top;
            self.render_cache.bottom = bottom;
            self.render_cache.dirty = false;
        }

        let editor = Paragraph::new(self.render_cache.lines.clone());
        frame.render_widget(editor, area);
    }
    pub(crate) fn code_block_lang_before_line(&self, line_index: usize) -> Option<String> {
        if line_index == 0 {
            return None;
        }

        let mut code_block_lang = None;
        for i in 0..line_index {
            let text = self.buffer.line_text(i).unwrap_or_default();
            if let Some(lang) = parse_code_fence_language(&text) {
                if code_block_lang.is_some() {
                    code_block_lang = None;
                } else {
                    code_block_lang = Some(lang);
                }
            }
        }
        code_block_lang
    }
    pub(crate) fn render_markdown_line(
        &self,
        text: &str,
        code_block_lang: &mut Option<String>,
    ) -> Line<'static> {
        if let Some(lang) = parse_code_fence_language(text) {
            if code_block_lang.is_some() {
                *code_block_lang = None;
            } else {
                *code_block_lang = Some(lang);
            }
            return Line::from(Span::styled(
                text.to_string(),
                Style::default()
                    .fg(Color::Rgb(180, 180, 200))
                    .bg(Color::Rgb(25, 25, 42))
                    .add_modifier(Modifier::BOLD),
            ));
        }

        if let Some(lang) = code_block_lang.as_deref() {
            return self.render_code_block_line(text, lang);
        }

        let base_style = self.base_markdown_style(text);
        self.render_inline_markdown(text, base_style)
    }
    pub(crate) fn render_code_block_line(&self, text: &str, language: &str) -> Line<'static> {
        let syntax = SYNTAX_SET
            .find_syntax_by_token(language)
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());
        let mut highlighter = HighlightLines::new(syntax, &SYNTECT_THEME);

        let highlighted = highlighter.highlight_line(text, &SYNTAX_SET);
        let Ok(tokens) = highlighted else {
            return Line::from(Span::styled(
                text.to_string(),
                Style::default()
                    .fg(Color::Rgb(200, 200, 200))
                    .bg(Color::Rgb(18, 18, 28)),
            ));
        };

        let spans: Vec<Span<'static>> = tokens
            .into_iter()
            .map(|(style, segment)| Span::styled(segment.to_string(), syntect_to_ratatui(style)))
            .collect();

        if spans.is_empty() {
            Line::from(Span::styled(
                text.to_string(),
                Style::default()
                    .fg(Color::Rgb(200, 200, 200))
                    .bg(Color::Rgb(18, 18, 28)),
            ))
        } else {
            Line::from(spans)
        }
    }
    pub(crate) fn base_markdown_style(&self, text: &str) -> Style {
        let trimmed = text.trim_start();

        if trimmed.starts_with("# ") {
            return Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD);
        }
        if trimmed.starts_with("## ") {
            return Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD);
        }
        if trimmed.starts_with("### ") {
            return Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD);
        }
        if trimmed.starts_with(">") {
            return Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC);
        }
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            return Style::default().fg(Color::LightCyan);
        }

        Style::default().fg(Color::Gray)
    }
    pub(crate) fn render_inline_markdown(&self, text: &str, base_style: Style) -> Line<'static> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut cursor = 0;

        while cursor < text.len() {
            let next = next_markdown_token(text, cursor);

            let Some((start, end, kind)) = next else {
                if cursor < text.len() {
                    spans.push(Span::styled(text[cursor..].to_string(), base_style));
                }
                break;
            };

            if start > cursor {
                spans.push(Span::styled(text[cursor..start].to_string(), base_style));
            }

            let token_style = match kind {
                TokenKind::WikiLink => base_style
                    .fg(Color::Rgb(0, 255, 136))
                    .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
                TokenKind::Link => base_style
                    .fg(Color::Rgb(255, 102, 0))
                    .add_modifier(Modifier::UNDERLINED),
                TokenKind::InlineCode => base_style
                    .fg(Color::Rgb(220, 220, 220))
                    .bg(Color::Rgb(32, 32, 48)),
                TokenKind::Bold => base_style.add_modifier(Modifier::BOLD),
                TokenKind::Italic => base_style.add_modifier(Modifier::ITALIC),
            };

            spans.push(Span::styled(text[start..end].to_string(), token_style));
            cursor = end;
        }

        if spans.is_empty() {
            Line::from(Span::styled(text.to_string(), base_style))
        } else {
            Line::from(spans)
        }
    }
}
