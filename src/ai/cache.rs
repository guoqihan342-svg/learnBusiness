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

    #[test]
    fn ai_cache_key_is_deterministic_for_same_inputs() {
        let a = AiCacheKey::new("ollama", "llava", "describe_image", "v1", "abc", false);
        let b = AiCacheKey::new("ollama", "llava", "describe_image", "v1", "abc", false);

        assert_eq!(a.to_filename(), b.to_filename());
        assert_eq!(a.to_filename().len(), 69);
        assert!(a.to_filename().ends_with(".json"));
    }

    #[test]
    fn ai_cache_key_isolated_by_all_dimensions() {
        let base = AiCacheKey::new("mock", "model-a", "answer", "v1", "abc", false);
        let base_name = base.to_filename();
        let variants = [
            AiCacheKey::new("ollama", "model-a", "answer", "v1", "abc", false),
            AiCacheKey::new("mock", "model-b", "answer", "v1", "abc", false),
            AiCacheKey::new("mock", "model-a", "describe_image", "v1", "abc", false),
            AiCacheKey::new("mock", "model-a", "answer", "v2", "abc", false),
            AiCacheKey::new("mock", "model-a", "answer", "v1", "def", false),
            AiCacheKey::new("mock", "model-a", "answer", "v1", "abc", true),
        ];

        for variant in variants {
            assert_ne!(base_name, variant.to_filename());
        }
    }
}
