use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use quick_xml::Reader;
use quick_xml::events::Event;
use regex::Regex;
use serde_json::Value;
use zip::ZipArchive;

use crate::models::ChunkKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedText {
    pub text: String,
    pub chunks: Vec<ExtractedChunk>,
    pub artifact_path: Option<PathBuf>,
    pub needs_ai: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedChunk {
    pub kind: Option<ChunkKind>,
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
            kind: Some(ChunkKind::Text),
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
            kind: Some(ChunkKind::Slide),
            slide: Some(slide),
            ..Self::text(text)
        }
    }

    fn table(source_range: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            kind: Some(ChunkKind::Table),
            source_range: Some(source_range.into()),
            ..Self::text(text)
        }
    }
}

pub fn extract_document_text(path: impl AsRef<Path>, file_type: &str) -> Result<ExtractedText> {
    let path = path.as_ref();
    match file_type {
        "text/plain" | "text/markdown" | "text/csv" | "text/tab-separated-values" => {
            Ok(ExtractedText {
                text: fs::read_to_string(path)?,
                chunks: Vec::new(),
                artifact_path: None,
                needs_ai: false,
            })
        }
        "application/json" => extract_json_text(path),
        "text/html" | "application/xml" | "text/xml" => extract_markup_text(path),
        "application/yaml" | "text/yaml" | "text/x-yaml" => Ok(ExtractedText {
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
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
            extract_xlsx_text(path)
        }
        other => bail!("unsupported file type for extraction: {other}"),
    }
}

fn extract_json_text(path: &Path) -> Result<ExtractedText> {
    let raw = fs::read_to_string(path)?;
    let text = match serde_json::from_str::<Value>(&raw) {
        Ok(value) => {
            let mut parts = Vec::new();
            collect_json_text(&value, &mut parts);
            if parts.is_empty() {
                raw
            } else {
                parts.join(" ")
            }
        }
        Err(_) => raw,
    };
    Ok(ExtractedText {
        text,
        chunks: Vec::new(),
        artifact_path: None,
        needs_ai: false,
    })
}

fn collect_json_text(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::Null => {}
        Value::Bool(value) => parts.push(value.to_string()),
        Value::Number(value) => parts.push(value.to_string()),
        Value::String(value) => {
            if !value.trim().is_empty() {
                parts.push(value.trim().to_string());
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_json_text(value, parts);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                parts.push(key.clone());
                collect_json_text(value, parts);
            }
        }
    }
}

fn extract_markup_text(path: &Path) -> Result<ExtractedText> {
    let raw = fs::read_to_string(path)?;
    let text = extract_xml_text(&raw).unwrap_or_else(|_| strip_markup_text(&raw));
    Ok(ExtractedText {
        text,
        chunks: Vec::new(),
        artifact_path: None,
        needs_ai: false,
    })
}

fn strip_markup_text(raw: &str) -> String {
    let without_tags = Regex::new(r"<[^>]+>")
        .expect("valid tag regex")
        .replace_all(raw, " ");
    without_tags
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

fn extract_xlsx_text(path: &Path) -> Result<ExtractedText> {
    let mut archive = ZipArchive::new(fs::File::open(path)?)?;
    let shared_strings = extract_xlsx_shared_strings(&mut archive)?;
    let mut sheets = Vec::new();

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let name = file.name().to_string();
        let Some(sheet) = sheet_number_from_path(&name) else {
            continue;
        };
        let mut xml = String::new();
        file.read_to_string(&mut xml)?;
        let text = extract_xlsx_sheet_text(&xml, &shared_strings)?;
        if !text.trim().is_empty() {
            sheets.push((sheet, text));
        }
    }
    sheets.sort_by_key(|(sheet, _)| *sheet);

    let chunks = sheets
        .iter()
        .map(|(sheet, text)| ExtractedChunk::table(format!("sheet={sheet}"), text.clone()))
        .collect::<Vec<_>>();
    let text = sheets
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

fn extract_xlsx_shared_strings(archive: &mut ZipArchive<fs::File>) -> Result<Vec<String>> {
    let Ok(mut file) = archive.by_name("xl/sharedStrings.xml") else {
        return Ok(Vec::new());
    };
    let mut xml = String::new();
    file.read_to_string(&mut xml)?;
    extract_xml_text_parts(&xml)
}

fn extract_xlsx_sheet_text(xml: &str, shared_strings: &[String]) -> Result<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut parts = Vec::new();
    let mut cell_type: Option<String> = None;
    let mut in_value = false;

    loop {
        match reader.read_event()? {
            Event::Start(start) => match local_name(start.name().as_ref()) {
                b"c" => {
                    cell_type = start
                        .attributes()
                        .with_checks(false)
                        .filter_map(Result::ok)
                        .find(|attr| local_name(attr.key.as_ref()) == b"t")
                        .map(|attr| String::from_utf8_lossy(attr.value.as_ref()).to_string());
                }
                b"v" => {
                    in_value = true;
                }
                b"t" if cell_type.as_deref() == Some("inlineStr") => {
                    in_value = true;
                }
                _ => {}
            },
            Event::Text(text) if in_value => {
                let value = text.unescape()?.into_owned();
                if cell_type.as_deref() == Some("s") {
                    if let Ok(index) = value.trim().parse::<usize>()
                        && let Some(shared) = shared_strings.get(index)
                    {
                        parts.push(shared.clone());
                    }
                } else if !value.trim().is_empty() {
                    parts.push(value.trim().to_string());
                }
            }
            Event::End(end) => match local_name(end.name().as_ref()) {
                b"v" | b"t" => {
                    in_value = false;
                }
                b"c" => {
                    cell_type = None;
                    in_value = false;
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }

    Ok(parts.join(" "))
}

fn extract_xml_text(xml: &str) -> Result<String> {
    Ok(extract_xml_text_parts(xml)?.join(" "))
}

fn extract_xml_text_parts(xml: &str) -> Result<Vec<String>> {
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
    Ok(parts)
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

fn sheet_number_from_path(path: &str) -> Option<u32> {
    let file_name = path.strip_prefix("xl/worksheets/")?;
    let number = file_name
        .strip_prefix("sheet")?
        .strip_suffix(".xml")?
        .parse()
        .ok()?;
    Some(number)
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
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

    #[test]
    fn extracts_csv_tsv_json_html_xml_and_yaml_text() {
        let dir = tempfile::tempdir().unwrap();
        let cases = [
            (
                "customers.csv",
                "text/csv",
                "customer,status\nAcme,approved",
            ),
            (
                "orders.tsv",
                "text/tab-separated-values",
                "order\tstate\nA001\tpaid",
            ),
            (
                "policy.json",
                "application/json",
                r#"{"workflow":"approval","risk":3,"enabled":true}"#,
            ),
            (
                "page.html",
                "text/html",
                "<html><body><h1>Portal</h1><p>Submit request</p></body></html>",
            ),
            (
                "data.xml",
                "application/xml",
                "<root><step>Archive contract</step></root>",
            ),
            (
                "config.yaml",
                "application/yaml",
                "role: reviewer\nstatus: pending",
            ),
        ];

        for (name, file_type, content) in cases {
            let file = dir.path().join(name);
            std::fs::write(&file, content).unwrap();
            let extracted = extract_document_text(&file, file_type).unwrap();
            assert!(!extracted.text.trim().is_empty(), "{name}");
            assert!(!extracted.needs_ai, "{name}");
        }
    }

    #[test]
    fn extracts_xlsx_text_as_table_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("book.xlsx");
        write_zip_entries(
            &file,
            &[
                (
                    "xl/sharedStrings.xml",
                    r#"
                    <sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                      <si><t>customer_name</t></si>
                      <si><t>approval_status</t></si>
                      <si><t>Acme Corp</t></si>
                    </sst>
                    "#,
                ),
                (
                    "xl/worksheets/sheet1.xml",
                    r#"
                    <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                      <sheetData>
                        <row>
                          <c t="s"><v>0</v></c>
                          <c t="s"><v>1</v></c>
                        </row>
                        <row>
                          <c t="s"><v>2</v></c>
                          <c><v>2026</v></c>
                        </row>
                      </sheetData>
                    </worksheet>
                    "#,
                ),
            ],
        );

        let extracted = extract_document_text(
            &file,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .unwrap();

        assert!(extracted.text.contains("customer_name"));
        assert!(extracted.text.contains("approval_status"));
        assert!(extracted.text.contains("Acme Corp"));
        assert!(extracted.text.contains("2026"));
        assert_eq!(extracted.chunks.len(), 1);
        assert_eq!(
            extracted.chunks[0].kind,
            Some(crate::models::ChunkKind::Table)
        );
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
