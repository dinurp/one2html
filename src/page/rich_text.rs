use crate::page::Renderer;
use crate::utils::{px, StyleSet};
use color_eyre::eyre::ContextCompat;
use color_eyre::Result;
use once_cell::sync::Lazy;
use onenote_parser::contents::RichText;
use onenote_parser::property::common::ColorRef;
use onenote_parser::property::rich_text::{ParagraphAlignment, ParagraphStyling};
use regex::{Captures, Regex};

impl<'a> Renderer<'a> {
    pub(crate) fn render_rich_text(&mut self, text: &RichText) -> Result<String> {
        let mut content = String::new();
        let mut style = self.parse_paragraph_styles(text);

        if let Some((note_tag_html, note_tag_styles)) = self.render_note_tags(text.note_tags()) {
            content.push_str(&note_tag_html);
            style.extend(note_tag_styles);
        }

        content.push_str(&self.parse_content(text)?);

        if content.starts_with("http://") || content.starts_with("https://") {
            content = format!("<a href=\"{}\">{}</a>", content, content);
        }

        match text.paragraph_style().style_id() {
            Some(t) if !self.in_list && is_tag(t) => {
                Ok(format!("<{} style=\"{}\">{}</{}>", t, style, content, t))
            }
            _ if style.len() > 0 => Ok(format!("<span style=\"{}\">{}</span>", style, content)),
            _ => Ok(content),
        }
    }

    fn parse_content(&self, data: &RichText) -> Result<String> {
        let indices = data.text_run_indices();
        let styles = data.text_run_formatting();

        let mut text = data.text().to_string();

        if text.is_empty() {
            return Ok("&nbsp;".to_string());
        }

        if indices.is_empty() {
            return Ok(fix_newlines(&text));
        }

        assert!(indices.len() + 1 >= styles.len());

        // Split text into parts specified by indices
        let mut parts: Vec<String> = vec![];

        for i in indices.iter().copied().rev() {
            let part = text.chars().skip(i as usize).collect();
            text = text.chars().take(i as usize).collect();

            parts.push(part);
        }

        if !indices.is_empty() {
            parts.push(text);
        }

        let mut in_hyperlink = false;

        let content = parts
            .into_iter()
            .rev()
            .zip(styles.iter())
            .map(|(text, style)| {
                if style.hyperlink() {
                    let text = self.render_hyperlink(text, style, in_hyperlink);
                    in_hyperlink = true;

                    text
                } else {
                    in_hyperlink = false;

                    let style = self.parse_style(style);

                    if style.len() > 0 {
                        Ok(format!("<span style=\"{}\">{}</span>", style, text))
                    } else {
                        Ok(text)
                    }
                }
            })
            .collect::<Result<String>>()?;

        Ok(fix_newlines(&content))
    }

    fn render_hyperlink(
        &self,
        text: String,
        style: &ParagraphStyling,
        in_hyperlink: bool,
    ) -> Result<String> {
        const HYPERLINK_MARKER: &str = "\u{fddf}HYPERLINK \"";

        let style = self.parse_style(style);

        if text.starts_with(HYPERLINK_MARKER) {
            let url = text
                .strip_prefix(HYPERLINK_MARKER)
                .wrap_err("Hyperlink has no start marker")?
                .strip_suffix('"')
                .wrap_err("Hyperlink has no end marker")?;

            Ok(format!("<a href=\"{}\" style=\"{}\">", url, style))
        } else if in_hyperlink {
            Ok(text + "</a>")
        } else {
            Ok(format!(
                "<a href=\"{}\" style=\"{}\">{}</a>",
                text, style, text
            ))
        }
    }

    fn parse_paragraph_styles(&self, text: &RichText) -> StyleSet {
        if text.text() == "" {
            return StyleSet::new();
        }

        let mut styles = self.parse_style(text.paragraph_style());

        if let [style] = text.text_run_formatting() {
            styles.extend(self.parse_style(style))
        }

        if text.paragraph_space_before() > 0.0 {
            styles.set("padding-top", px(text.paragraph_space_before()))
        }

        if text.paragraph_space_after() > 0.0 {
            styles.set("padding-bottom", px(text.paragraph_space_after()))
        }

        if let Some(line_spacing) = text.paragraph_line_spacing_exact() {
            if line_spacing > 0.0 {
                dbg!(text);
                unimplemented!();
            }
        }

        match text.paragraph_alignment() {
            ParagraphAlignment::Center => styles.set("text-align", "center".to_string()),
            ParagraphAlignment::Right => styles.set("text-align", "right".to_string()),
            _ => {}
        }

        styles
    }

    fn parse_style(&self, style: &ParagraphStyling) -> StyleSet {
        let mut styles = StyleSet::new();

        if style.bold() {
            styles.set("font-weight", "bold".to_string());
        }

        if style.italic() {
            styles.set("font-style", "italic".to_string());
        }

        if style.underline() {
            styles.set("text-decoration", "underline".to_string());
        }

        if style.superscript() {
            styles.set("vertical-align", "super".to_string());
        }

        if style.subscript() {
            styles.set("vertical-align", "sub".to_string());
        }

        if style.strikethrough() {
            styles.set("text-decoration", "line-through".to_string());
        }

        if let Some(font) = style.font() {
            styles.set("font-family", font.to_string());
        }

        if let Some(size) = style.font_size() {
            styles.set("font-size", ((size as f32) / 2.0).to_string() + "pt");
        }

        if let Some(ColorRef::Manual { r, g, b }) = style.font_color() {
            styles.set("color", format!("rgb({},{},{})", r, g, b));
        }

        if let Some(ColorRef::Manual { r, g, b }) = style.highlight() {
            styles.set("background-color", format!("rgb({},{},{})", r, g, b));
        }

        if style.paragraph_alignment().is_some() {
            unimplemented!()
        }

        if let Some(space) = style.paragraph_space_before() {
            if space != 0.0 {
                unimplemented!()
            }
        }

        if let Some(space) = style.paragraph_space_after() {
            if space != 0.0 {
                unimplemented!()
            }
        }

        if let Some(space) = style.paragraph_line_spacing_exact() {
            if space != 0.0 {
                unimplemented!()
            }
        }

        if style.math_formatting() {
            // FIXME: Handle math formatting
            // See https://docs.microsoft.com/en-us/windows/win32/api/richedit/ns-richedit-gettextex
            // for unicode chars used
            // unimplemented!()
        }

        styles
    }
}

fn is_tag(tag: &str) -> bool {
    !matches!(tag, "PageDateTime" | "PageTitle")
}

fn fix_newlines(text: &str) -> String {
    static REGEX_LEADING_SPACES: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"<br>(\s+)").expect("failed to compile regex"));

    let text = text
        .replace("\u{000b}", "<br>")
        .replace("\n", "<br>")
        .replace("\r", "<br>");

    REGEX_LEADING_SPACES
        .replace_all(&text, |captures: &Captures| {
            "<br>".to_string() + &"&nbsp;".repeat(captures[1].len())
        })
        .to_string()
}
