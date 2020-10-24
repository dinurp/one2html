use crate::page::Renderer;
use color_eyre::eyre::{ContextCompat, WrapErr};
use color_eyre::Result;
use onenote_parser::contents::EmbeddedFile;
use onenote_parser::property::embedded_file::FileType;
use std::fs;
use std::path::PathBuf;

impl<'a> Renderer<'a> {
    pub(crate) fn render_embedded_file(&mut self, file: &EmbeddedFile) -> Result<String> {
        let content;

        let filename = self.determine_filename(file.filename())?;
        fs::write(self.output.join(filename.clone()), file.data())
            .wrap_err("Failed to write embedded file")?;

        let file_type = Self::guess_type(file);

        match file_type {
            FileType::Audio => content = format!("<audio controls src=\"{}\"></audio>", filename),
            FileType::Video => content = format!("<video controls src=\"{}\"></video>", filename),
            FileType::Unknown => content = format!("<embed src=\"{}\" />", filename),
        };

        Ok(self.render_with_note_tags(file.note_tags(), content))
    }

    fn guess_type(file: &EmbeddedFile) -> FileType {
        match file.file_type() {
            FileType::Audio => return FileType::Audio,
            FileType::Video => return FileType::Video,
            _ => {}
        };

        let filename = file.filename();

        if let Some(mime) = mime_guess::from_path(filename).first() {
            if mime.type_() == "audio" {
                return FileType::Audio;
            }

            if mime.type_() == "video" {
                return FileType::Video;
            }
        }
        FileType::Unknown
    }

    pub(crate) fn determine_filename(&mut self, filename: &str) -> Result<String> {
        let mut i = 0;
        let mut current_filename = filename.to_string();

        loop {
            if !self.section.files.contains(&current_filename) {
                self.section.files.insert(current_filename.clone());

                return Ok(current_filename);
            }

            let path = PathBuf::from(filename);
            let ext = path
                .extension()
                .wrap_err("Embedded file has no extension")?
                .to_str()
                .wrap_err("Embedded file name is non utf-8")?;
            let base = path
                .as_os_str()
                .to_str()
                .wrap_err("Embedded file name is non utf-8")?
                .strip_suffix(ext)
                .wrap_err("Failed to strip extension from file name")?
                .trim_matches('.');

            current_filename = format!("{}-{}.{}", base, i, ext);

            i += 1;
        }
    }
}
