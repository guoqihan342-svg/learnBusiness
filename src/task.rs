use std::collections::HashSet;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    ReadLocal,
    WriteWorkspace,
    ExternalNetwork,
    AiExternal,
    McpExternal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionSet {
    grants: HashSet<Permission>,
}

impl PermissionSet {
    pub fn new(grants: Vec<Permission>) -> Self {
        Self {
            grants: grants.into_iter().collect(),
        }
    }

    pub fn contains(&self, permission: Permission) -> bool {
        self.grants.contains(&permission)
    }

    pub fn trusted_cli_defaults() -> Self {
        Self::new(vec![
            Permission::ReadLocal,
            Permission::WriteWorkspace,
            Permission::AiExternal,
            Permission::ExternalNetwork,
        ])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub permission: Permission,
}

impl ToolDescriptor {
    pub fn new(name: impl Into<String>, permission: Permission) -> Self {
        Self {
            name: name.into(),
            permission,
        }
    }

    pub fn ensure_allowed(&self, grants: &PermissionSet) -> Result<()> {
        if grants.contains(self.permission) {
            Ok(())
        } else {
            bail!(
                "tool '{}' requires missing permission {:?}",
                self.name,
                self.permission
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDescriptor {
    pub id: String,
    pub role: String,
    pub goal: String,
    pub allowed_tools: Vec<String>,
    pub model_policy: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDescriptor {
    pub id: String,
    pub kind: String,
    pub input_refs: Vec<String>,
    pub output_refs: Vec<String>,
    pub required_permissions: Vec<Permission>,
    pub token_budget: Option<u32>,
    pub max_iterations: u32,
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_permission_denies_ungranted_external_ai() {
        let tool = ToolDescriptor::new("describe_image", Permission::AiExternal);
        let grants = PermissionSet::new(vec![Permission::ReadLocal]);
        assert!(tool.ensure_allowed(&grants).is_err());
    }

    #[test]
    fn dry_run_describe_image_does_not_require_external_permissions() {
        let policy = CommandPermissionPolicy::describe_image(true);
        assert!(policy.requires(Permission::ReadLocal));
        assert!(policy.requires(Permission::WriteWorkspace));
        assert!(!policy.requires(Permission::AiExternal));
        assert!(!policy.requires(Permission::ExternalNetwork));
    }

    #[test]
    fn non_dry_run_describe_image_requires_external_ai_permissions() {
        let policy = CommandPermissionPolicy::describe_image(false);
        assert!(policy.requires(Permission::AiExternal));
        assert!(policy.requires(Permission::ExternalNetwork));
    }

    #[test]
    fn run_with_permissions_does_not_execute_when_permission_is_missing() {
        let policy = CommandPermissionPolicy::new(
            "ingest",
            vec![Permission::ReadLocal, Permission::WriteWorkspace],
        );
        let grants = PermissionSet::new(vec![Permission::ReadLocal]);
        let mut executed = false;

        let result = run_with_permissions(&policy, &grants, || {
            executed = true;
            Ok(())
        });

        assert!(result.is_err());
        assert!(!executed);
    }

    #[test]
    fn search_requires_only_read_permission() {
        let policy = CommandPermissionPolicy::search();
        assert!(policy.requires(Permission::ReadLocal));
        assert!(!policy.requires(Permission::AiExternal));
        assert!(!policy.requires(Permission::ExternalNetwork));
    }

    #[test]
    fn ingest_image_dry_run_does_not_require_external_network() {
        let policy = CommandPermissionPolicy::ingest_with_options(true, true);
        assert!(policy.requires(Permission::ReadLocal));
        assert!(policy.requires(Permission::WriteWorkspace));
        assert!(!policy.requires(Permission::ExternalNetwork));
    }

    #[test]
    fn ingest_image_description_requires_external_permissions() {
        let policy = CommandPermissionPolicy::ingest_with_options(true, false);
        assert!(policy.requires(Permission::AiExternal));
        assert!(policy.requires(Permission::ExternalNetwork));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPermissionPolicy {
    pub name: String,
    required: Vec<Permission>,
}

impl CommandPermissionPolicy {
    pub fn new(name: impl Into<String>, required: Vec<Permission>) -> Self {
        Self {
            name: name.into(),
            required,
        }
    }

    pub fn init() -> Self {
        Self::new("init", vec![Permission::WriteWorkspace])
    }

    pub fn ingest() -> Self {
        Self::ingest_with_options(false, false)
    }

    pub fn ingest_with_options(describe_images: bool, dry_run_ai: bool) -> Self {
        let mut required = vec![Permission::ReadLocal, Permission::WriteWorkspace];
        if describe_images && !dry_run_ai {
            required.push(Permission::AiExternal);
            required.push(Permission::ExternalNetwork);
        }
        Self::new("ingest", required)
    }

    pub fn status() -> Self {
        Self::new("status", vec![Permission::ReadLocal])
    }

    pub fn inspect_ai() -> Self {
        Self::new("inspect-ai", vec![Permission::ReadLocal])
    }

    pub fn search() -> Self {
        Self::new("search", vec![Permission::ReadLocal])
    }

    pub fn report() -> Self {
        Self::new(
            "report",
            vec![Permission::ReadLocal, Permission::WriteWorkspace],
        )
    }

    pub fn ask() -> Self {
        Self::new(
            "ask",
            vec![
                Permission::ReadLocal,
                Permission::WriteWorkspace,
                Permission::AiExternal,
            ],
        )
    }

    pub fn describe_image(dry_run: bool) -> Self {
        let mut required = vec![Permission::ReadLocal, Permission::WriteWorkspace];
        if !dry_run {
            required.push(Permission::AiExternal);
            required.push(Permission::ExternalNetwork);
        }
        Self::new("describe-image", required)
    }

    pub fn requires(&self, permission: Permission) -> bool {
        self.required.contains(&permission)
    }

    pub fn ensure_allowed(&self, grants: &PermissionSet) -> Result<()> {
        for permission in &self.required {
            if !grants.contains(*permission) {
                bail!(
                    "command '{}' requires missing permission {:?}",
                    self.name,
                    permission
                );
            }
        }
        Ok(())
    }
}

pub fn run_with_permissions<T>(
    policy: &CommandPermissionPolicy,
    grants: &PermissionSet,
    action: impl FnOnce() -> Result<T>,
) -> Result<T> {
    policy.ensure_allowed(grants)?;
    action()
}
