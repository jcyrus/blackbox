use crate::app::App;
use crate::app::{
    SYNTAX_SET, SYNTECT_THEME, TokenKind, next_markdown_token, parse_code_fence_language,
    syntect_to_ratatui,
};
use crate::model::mode::Mode;
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
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

            let highlight_cursor = self.mode == Mode::Normal
                || self.mode == Mode::Sidebar
                || self.mode == Mode::Command
                || self.mode == Mode::Backlinks
                || self.mode == Mode::FinderOpen;
            let show_line_nums = self.config.editor.line_numbers;
            let rel_line_nums = self.config.editor.relative_line_numbers;
            let cursor_row = self.buffer.cursor.row;
            let gutter_width = self.buffer.line_count().to_string().len().max(3);

            self.render_cache.lines = (top..bottom)
                .map(|i| {
                    let text = self.buffer.line_text(i).unwrap_or_default();
                    let mut spans = self.render_markdown_line(&text, &mut code_block_lang);
                    let is_cursor_line = i == cursor_row;

                    if show_line_nums {
                        let mut num = i + 1;
                        if rel_line_nums && self.mode == Mode::Normal && !is_cursor_line {
                            num = (i as isize - cursor_row as isize).abs() as usize;
                        }

                        let gutter_style = if is_cursor_line {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };

                        let gutter_text = format!("{:>width$} ", num, width = gutter_width);
                        spans.insert(0, Span::styled(gutter_text, gutter_style));
                    }

                    if is_cursor_line && highlight_cursor {
                        for span in spans.iter_mut() {
                            span.style = span.style.bg(Color::Rgb(30, 30, 45));
                        }
                    }

                    Line::from(spans)
                })
                .collect();
            self.render_cache.top = top;
            self.render_cache.bottom = bottom;
            self.render_cache.dirty = false;
        }

        let mut editor = Paragraph::new(self.render_cache.lines.clone());
        if self.config.editor.soft_wrap {
            editor = editor.wrap(Wrap { trim: false });
        }
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
    ) -> Vec<Span<'static>> {
        if let Some(lang) = parse_code_fence_language(text) {
            if code_block_lang.is_some() {
                *code_block_lang = None;
            } else {
                *code_block_lang = Some(lang);
            }
            return vec![Span::styled(
                text.to_string(),
                Style::default()
                    .fg(Color::Rgb(180, 180, 200))
                    .bg(Color::Rgb(25, 25, 42))
                    .add_modifier(Modifier::BOLD),
            )];
        }

        if let Some(lang) = code_block_lang.as_deref() {
            return self.render_code_block_line(text, lang);
        }

        let base_style = self.base_markdown_style(text);
        self.render_inline_markdown(text, base_style)
    }
    pub(crate) fn render_code_block_line(&self, text: &str, language: &str) -> Vec<Span<'static>> {
        let syntax = SYNTAX_SET
            .find_syntax_by_token(language)
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());
        let mut highlighter = HighlightLines::new(syntax, &SYNTECT_THEME);

        let highlighted = highlighter.highlight_line(text, &SYNTAX_SET);
        let Ok(tokens) = highlighted else {
            return vec![Span::styled(
                text.to_string(),
                Style::default()
                    .fg(Color::Rgb(200, 200, 200))
                    .bg(Color::Rgb(18, 18, 28)),
            )];
        };

        let spans: Vec<Span<'static>> = tokens
            .into_iter()
            .map(|(style, segment)| Span::styled(segment.to_string(), syntect_to_ratatui(style)))
            .collect();

        if spans.is_empty() {
            vec![Span::styled(
                text.to_string(),
                Style::default()
                    .fg(Color::Rgb(200, 200, 200))
                    .bg(Color::Rgb(18, 18, 28)),
            )]
        } else {
            spans
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
    pub(crate) fn render_inline_markdown(
        &self,
        text: &str,
        base_style: Style,
    ) -> Vec<Span<'static>> {
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
            vec![Span::styled(text.to_string(), base_style)]
        } else {
            spans
        }
    }
}
