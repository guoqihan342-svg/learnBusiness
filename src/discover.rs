use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::Result;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredDocument {
    pub path: PathBuf,
    pub file_type: String,
    pub sha256: String,
    pub size_bytes: u64,
}

pub fn discover_documents(root: impl AsRef<Path>) -> Result<Vec<DiscoveredDocument>> {
    let mut documents = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if !is_supported_document(path) {
            continue;
        }

        let metadata = entry.metadata()?;
        documents.push(DiscoveredDocument {
            path: path.to_path_buf(),
            file_type: guess_file_type(path),
            sha256: sha256_file(path)?,
            size_bytes: metadata.len(),
        });
    }
    documents.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(documents)
}

pub fn is_supported_document(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("txt")
            | Some("md")
            | Some("pdf")
            | Some("png")
            | Some("jpg")
            | Some("jpeg")
            | Some("webp")
            | Some("docx")
            | Some("pptx")
    )
}

pub fn guess_file_type(path: &Path) -> String {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("txt") => "text/plain".to_string(),
        Some("md") => "text/markdown".to_string(),
        Some("pdf") => "application/pdf".to_string(),
        Some("docx") => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string()
        }
        Some("pptx") => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation".to_string()
        }
        _ => mime_guess::from_path(path)
            .first_or_octet_stream()
            .essence_str()
            .to_string(),
    }
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_supported_documents_with_hashes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
        std::fs::write(dir.path().join("b.exe"), "skip").unwrap();

        let docs = discover_documents(dir.path()).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].file_type, "text/plain");
        assert_eq!(docs[0].sha256.len(), 64);
    }
}
