use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::{WinxError, WinxResult};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    ReadFile,
    WriteFile,
    EditFile,
    ExecuteCommand,
    ReadImage,
    SaveContext,
    AccessNetwork,
    LoadPlugin,
    AccessEnvironment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    pub allowed_actions: HashSet<Action>,
    pub allowed_paths: Vec<PathBuf>,
    pub denied_paths: Vec<PathBuf>,
    pub allowed_commands: Vec<String>,
    pub denied_commands: Vec<String>,
}

impl Default for Role {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            allowed_actions: HashSet::new(),
            allowed_paths: vec![],
            denied_paths: vec![],
            allowed_commands: vec![],
            denied_commands: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoleBasedAccess {
    roles: Vec<Role>,
    default_role: String,
    user_roles: std::collections::HashMap<String, String>,
}

impl RoleBasedAccess {
    pub fn new() -> Self {
        let mut default_role = Role::default();
        default_role.allowed_actions.insert(Action::ReadFile);
        default_role.allowed_actions.insert(Action::SaveContext);

        Self {
            roles: vec![default_role],
            default_role: "default".to_string(),
            user_roles: std::collections::HashMap::new(),
        }
    }

    pub fn add_role(&mut self, role: Role) {
        // Replace if exists
        self.roles.retain(|r| r.name != role.name);
        self.roles.push(role);
    }

    pub fn assign_role(&mut self, user: &str, role_name: &str) -> WinxResult<()> {
        if !self.roles.iter().any(|r| r.name == role_name) {
            return Err(WinxError::invalid_argument(format!(
                "Role '{}' does not exist",
                role_name
            )));
        }
        self.user_roles
            .insert(user.to_string(), role_name.to_string());
        Ok(())
    }

    pub fn check_permission(&self, user: &str, action: &Action, path: Option<&Path>) -> bool {
        let role_name = self.user_roles.get(user).unwrap_or(&self.default_role);
        let role = match self.roles.iter().find(|r| &r.name == role_name) {
            Some(role) => role,
            None => return false,
        };

        // Check if the action is allowed
        if !role.allowed_actions.contains(action) {
            return false;
        }

        // If there's a path, check path permissions
        if let Some(path) = path {
            // First check denied paths
            for denied in &role.denied_paths {
                if path.starts_with(denied) {
                    return false;
                }
            }

            // Check allowed paths if any are specified
            if !role.allowed_paths.is_empty() {
                let mut is_allowed = false;
                for allowed in &role.allowed_paths {
                    if path.starts_with(allowed) {
                        is_allowed = true;
                        break;
                    }
                }
                if !is_allowed {
                    return false;
                }
            }
        }

        true
    }
}

impl Default for RoleBasedAccess {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SecurityManager {
    verify_signatures: bool,
    allowed_paths: Vec<PathBuf>,
    allowed_hosts: Vec<String>,
    sandboxed: bool,
    role_based_access: RoleBasedAccess,
}

impl SecurityManager {
    pub fn new() -> Self {
        Self {
            verify_signatures: true,
            allowed_paths: vec![],
            allowed_hosts: vec![],
            sandboxed: false,
            role_based_access: RoleBasedAccess::new(),
        }
    }

    pub fn with_settings(
        verify_signatures: bool,
        allowed_paths: Vec<PathBuf>,
        allowed_hosts: Vec<String>,
        sandboxed: bool,
    ) -> Self {
        Self {
            verify_signatures,
            allowed_paths,
            allowed_hosts,
            sandboxed,
            role_based_access: RoleBasedAccess::new(),
        }
    }

    pub async fn verify_plugin_signature(&self, _path: &str) -> WinxResult<bool> {
        if !self.verify_signatures {
            return Ok(true);
        }

        // TODO: Initialize sigstore trust repository
        // let repo = TrustRepository::new()?;

        // TODO: Build sigstore client
        // let client = ClientBuilder::default()
        //     .with_trust_repository(&repo)?
        //     .build()?;

        // TODO: Implement actual signature verification
        // For now, we'll just return an error indicating it's not implemented
        Err(WinxError::other(
            "Plugin signature verification not yet implemented",
        ))
    }

    pub fn check_permission(
        &self,
        user: &str,
        action: Action,
        path: Option<&Path>,
    ) -> WinxResult<()> {
        if !self.role_based_access.check_permission(user, &action, path) {
            return Err(WinxError::permission_error(format!(
                "User '{}' does not have permission for action '{:?}' on path '{:?}'",
                user, action, path
            )));
        }

        // Additional path checks
        if let Some(path) = path {
            // If sandboxed, only allow access to allowed paths
            if self.sandboxed && !self.allowed_paths.is_empty() {
                let mut is_allowed = false;
                for allowed in &self.allowed_paths {
                    if path.starts_with(allowed) {
                        is_allowed = true;
                        break;
                    }
                }
                if !is_allowed {
                    return Err(WinxError::permission_error(format!(
                        "Access to path '{}' is not allowed in sandboxed mode",
                        path.display()
                    )));
                }
            }
        }

        Ok(())
    }

    pub fn is_host_allowed(&self, host: &str) -> bool {
        if self.allowed_hosts.is_empty() {
            return true; // No restrictions
        }
        self.allowed_hosts.contains(&host.to_string())
    }

    pub fn is_sandboxed(&self) -> bool {
        self.sandboxed
    }

    pub fn add_allowed_path(&mut self, path: PathBuf) {
        if !self.allowed_paths.contains(&path) {
            self.allowed_paths.push(path);
        }
    }

    pub fn add_allowed_host(&mut self, host: String) {
        if !self.allowed_hosts.contains(&host) {
            self.allowed_hosts.push(host);
        }
    }

    pub fn set_sandboxed(&mut self, sandboxed: bool) {
        self.sandboxed = sandboxed;
    }

    pub fn set_verify_signatures(&mut self, verify: bool) {
        self.verify_signatures = verify;
    }

    pub fn get_role_based_access(&mut self) -> &mut RoleBasedAccess {
        &mut self.role_based_access
    }

    pub fn create_admin_role() -> Role {
        let mut admin = Role {
            name: "admin".to_string(),
            allowed_actions: HashSet::new(),
            allowed_paths: vec![],
            denied_paths: vec![],
            allowed_commands: vec![],
            denied_commands: vec![],
        };

        admin.allowed_actions.insert(Action::ReadFile);
        admin.allowed_actions.insert(Action::WriteFile);
        admin.allowed_actions.insert(Action::EditFile);
        admin.allowed_actions.insert(Action::ExecuteCommand);
        admin.allowed_actions.insert(Action::ReadImage);
        admin.allowed_actions.insert(Action::SaveContext);
        admin.allowed_actions.insert(Action::AccessNetwork);
        admin.allowed_actions.insert(Action::LoadPlugin);
        admin.allowed_actions.insert(Action::AccessEnvironment);

        admin
    }

    pub fn create_readonly_role() -> Role {
        let mut readonly = Role {
            name: "readonly".to_string(),
            allowed_actions: HashSet::new(),
            allowed_paths: vec![],
            denied_paths: vec![],
            allowed_commands: vec![],
            denied_commands: vec![],
        };

        readonly.allowed_actions.insert(Action::ReadFile);
        readonly.allowed_actions.insert(Action::ReadImage);
        readonly.allowed_actions.insert(Action::ExecuteCommand);

        // Only allow non-destructive commands in readonly mode
        readonly.allowed_commands.push("ls".to_string());
        readonly.allowed_commands.push("pwd".to_string());
        readonly.allowed_commands.push("cat".to_string());
        readonly.allowed_commands.push("grep".to_string());
        readonly.allowed_commands.push("find".to_string());

        readonly
    }
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new()
    }
}
