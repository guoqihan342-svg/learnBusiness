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
}
