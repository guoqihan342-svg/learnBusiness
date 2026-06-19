use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use quick_xml::Reader;
use quick_xml::events::Event;
use zip::ZipArchive;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedText {
    pub text: String,
    pub chunks: Vec<ExtractedChunk>,
    pub artifact_path: Option<PathBuf>,
    pub needs_ai: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedChunk {
    pub text: String,
    pub page: Option<u32>,
    pub slide: Option<u32>,
    pub source_range: Option<String>,
    pub artifact_path: Option<PathBuf>,
    pub confidence: Option<u8>,
    pub ai_generated: bool,
}

impl ExtractedChunk {
    fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            page: None,
            slide: None,
            source_range: None,
            artifact_path: None,
            confidence: None,
            ai_generated: false,
        }
    }

    fn slide(slide: u32, text: impl Into<String>) -> Self {
        Self {
            slide: Some(slide),
            ..Self::text(text)
        }
    }
}

pub fn extract_document_text(path: impl AsRef<Path>, file_type: &str) -> Result<ExtractedText> {
    let path = path.as_ref();
    match file_type {
        "text/plain" | "text/markdown" => Ok(ExtractedText {
            text: fs::read_to_string(path)?,
            chunks: Vec::new(),
            artifact_path: None,
            needs_ai: false,
        }),
        "application/pdf" => Ok(ExtractedText {
            text: pdf_extract::extract_text(path)?,
            chunks: Vec::new(),
            artifact_path: None,
            needs_ai: false,
        }),
        file_type if file_type.starts_with("image/") => Ok(ExtractedText {
            text: String::new(),
            chunks: Vec::new(),
            artifact_path: Some(path.to_path_buf()),
            needs_ai: true,
        }),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            extract_docx_text(path)
        }
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            extract_pptx_text(path)
        }
        other => bail!("unsupported file type for extraction: {other}"),
    }
}

fn extract_docx_text(path: &Path) -> Result<ExtractedText> {
    let mut archive = ZipArchive::new(fs::File::open(path)?)?;
    let mut document_xml = String::new();
    archive
        .by_name("word/document.xml")?
        .read_to_string(&mut document_xml)?;
    let text = extract_xml_text(&document_xml)?;
    let chunks = if text.trim().is_empty() {
        Vec::new()
    } else {
        vec![ExtractedChunk::text(text.clone())]
    };
    Ok(ExtractedText {
        text,
        chunks,
        artifact_path: None,
        needs_ai: false,
    })
}

fn extract_pptx_text(path: &Path) -> Result<ExtractedText> {
    let mut archive = ZipArchive::new(fs::File::open(path)?)?;
    let mut slides = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let name = file.name().to_string();
        let Some(slide) = slide_number_from_path(&name) else {
            continue;
        };
        let mut xml = String::new();
        file.read_to_string(&mut xml)?;
        let text = extract_xml_text(&xml)?;
        if !text.trim().is_empty() {
            slides.push((slide, text));
        }
    }
    slides.sort_by_key(|(slide, _)| *slide);

    let chunks = slides
        .iter()
        .map(|(slide, text)| ExtractedChunk::slide(*slide, text.clone()))
        .collect::<Vec<_>>();
    let text = slides
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        .join("\n\n");

    Ok(ExtractedText {
        text,
        chunks,
        artifact_path: None,
        needs_ai: false,
    })
}

fn extract_xml_text(xml: &str) -> Result<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut parts = Vec::new();
    loop {
        match reader.read_event()? {
            Event::Text(text) => {
                let value = text.unescape()?.into_owned();
                if !value.trim().is_empty() {
                    parts.push(value.trim().to_string());
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(parts.join(" "))
}

fn slide_number_from_path(path: &str) -> Option<u32> {
    let file_name = path.strip_prefix("ppt/slides/")?;
    let number = file_name
        .strip_prefix("slide")?
        .strip_suffix(".xml")?
        .parse()
        .ok()?;
    Some(number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn extracts_plain_text_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sample.txt");
        std::fs::write(&file, "业务流程").unwrap();

        let extracted = extract_document_text(&file, "text/plain").unwrap();
        assert!(extracted.text.contains("业务流程"));
    }

    #[test]
    fn extracts_docx_text_from_document_xml() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("process.docx");
        write_zip_entries(
            &file,
            &[(
                "word/document.xml",
                r#"
                <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                  <w:body>
                    <w:p><w:r><w:t>客户提交申请</w:t></w:r></w:p>
                    <w:p><w:r><w:t>运营审核归档</w:t></w:r></w:p>
                  </w:body>
                </w:document>
                "#,
            )],
        );

        let extracted = extract_document_text(
            &file,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        )
        .unwrap();

        assert!(extracted.text.contains("客户提交申请"));
        assert!(extracted.text.contains("运营审核归档"));
        assert!(!extracted.needs_ai);
    }

    #[test]
    fn extracts_pptx_text_grouped_by_slide() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("deck.pptx");
        write_zip_entries(
            &file,
            &[
                (
                    "ppt/slides/slide2.xml",
                    r#"
                    <p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
                      <p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>第二页风险点</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld>
                    </p:sld>
                    "#,
                ),
                (
                    "ppt/slides/slide1.xml",
                    r#"
                    <p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
                      <p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>第一页核心流程</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld>
                    </p:sld>
                    "#,
                ),
            ],
        );

        let extracted = extract_document_text(
            &file,
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        )
        .unwrap();

        assert!(extracted.text.contains("第一页核心流程"));
        assert!(extracted.text.contains("第二页风险点"));
    }

    fn write_zip_entries(path: &Path, entries: &[(&str, &str)]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (name, content) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
}
