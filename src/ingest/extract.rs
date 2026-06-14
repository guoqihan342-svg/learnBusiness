use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedText {
    pub text: String,
    pub artifact_path: Option<PathBuf>,
    pub needs_ai: bool,
}

pub fn extract_document_text(path: impl AsRef<Path>, file_type: &str) -> Result<ExtractedText> {
    let path = path.as_ref();
    match file_type {
        "text/plain" | "text/markdown" => Ok(ExtractedText {
            text: fs::read_to_string(path)?,
            artifact_path: None,
            needs_ai: false,
        }),
        "application/pdf" => Ok(ExtractedText {
            text: pdf_extract::extract_text(path)?,
            artifact_path: None,
            needs_ai: false,
        }),
        file_type if file_type.starts_with("image/") => Ok(ExtractedText {
            text: String::new(),
            artifact_path: Some(path.to_path_buf()),
            needs_ai: true,
        }),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        | "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            Ok(ExtractedText {
                text: String::new(),
                artifact_path: Some(path.to_path_buf()),
                needs_ai: true,
            })
        }
        other => bail!("unsupported file type for extraction: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_plain_text_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sample.txt");
        std::fs::write(&file, "业务流程").unwrap();

        let extracted = extract_document_text(&file, "text/plain").unwrap();
        assert!(extracted.text.contains("业务流程"));
    }
}
