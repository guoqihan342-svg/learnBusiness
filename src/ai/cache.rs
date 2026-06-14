use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiCacheKey {
    provider: String,
    model: String,
    purpose: String,
    prompt_version: String,
    content_hash: String,
    redaction_applied: bool,
}

impl AiCacheKey {
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        purpose: impl Into<String>,
        prompt_version: impl Into<String>,
        content_hash: impl Into<String>,
        redaction_applied: bool,
    ) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            purpose: purpose.into(),
            prompt_version: prompt_version.into(),
            content_hash: content_hash.into(),
            redaction_applied,
        }
    }

    pub fn to_filename(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.provider.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.model.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.purpose.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.prompt_version.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.content_hash.as_bytes());
        hasher.update(b"\0");
        hasher.update(if self.redaction_applied {
            b"redacted".as_slice()
        } else {
            b"raw".as_slice()
        });
        format!("{:x}.json", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_cache_key_changes_when_prompt_version_changes() {
        let a = AiCacheKey::new("openai", "gpt-4o-mini", "describe_image", "v1", "abc", true);
        let b = AiCacheKey::new("openai", "gpt-4o-mini", "describe_image", "v2", "abc", true);
        assert_ne!(a.to_filename(), b.to_filename());
    }
}
