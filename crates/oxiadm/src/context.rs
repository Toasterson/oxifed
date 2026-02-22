//! Persistent actor context for oxiadm
//!
//! Stores server configurations and the current context at
//! `$XDG_CONFIG_HOME/oxiadm/context.toml` so that they don't need
//! to be specified on every command.
//!
//! ## Config format
//!
//! ```toml
//! [context]
//! current_server = "oxifed.io"
//! actor = "toasterson@oxifed.io"
//!
//! [[servers]]
//! hostname = "oxifed.io"
//! admin_api_url = "https://admin.oxifed.io"
//! issuer_url = "https://cloud.wegmueller.it"
//! client_id = "..."
//! access_token = "..."
//! ```

use miette::{Context, IntoDiagnostic, Result, miette};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OxiadmContext {
    #[serde(default)]
    pub context: ContextInner,

    #[serde(default)]
    pub servers: Vec<ServerConfig>,

    /// Legacy auth — kept for migration only.
    #[serde(default, skip_serializing_if = "AuthContext::is_empty")]
    pub auth: AuthContext,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ContextInner {
    /// The currently selected server hostname
    pub current_server: Option<String>,
    /// The current actor identity (e.g. "toasterson@oxifed.io")
    pub actor: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    /// Server hostname (e.g. "oxifed.io")
    pub hostname: String,
    /// Admin API URL discovered via WebFinger
    pub admin_api_url: String,
    /// OIDC issuer URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer_url: Option<String>,
    /// OAuth audience required by the admin API
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,
    /// OIDC client ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// OIDC client secret (from auto-registration)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    /// OAuth2 access token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    /// OAuth2 refresh token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Token expiry time (RFC 3339)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AuthContext {
    /// OIDC issuer URL
    pub issuer_url: Option<String>,
    /// OIDC client ID
    pub client_id: Option<String>,
    /// OIDC client secret (from auto-registration)
    pub client_secret: Option<String>,
    /// OAuth2 access token
    pub access_token: Option<String>,
    /// OAuth2 refresh token
    pub refresh_token: Option<String>,
    /// Token expiry time (RFC 3339)
    pub expires_at: Option<String>,
}

impl AuthContext {
    pub fn is_empty(&self) -> bool {
        self.issuer_url.is_none()
            && self.client_id.is_none()
            && self.client_secret.is_none()
            && self.access_token.is_none()
            && self.refresh_token.is_none()
            && self.expires_at.is_none()
    }
}

impl OxiadmContext {
    /// Find a server config by hostname.
    pub fn find_server(&self, hostname: &str) -> Option<&ServerConfig> {
        self.servers.iter().find(|s| s.hostname == hostname)
    }

    /// Find a mutable server config by hostname.
    pub fn find_server_mut(&mut self, hostname: &str) -> Option<&mut ServerConfig> {
        self.servers.iter_mut().find(|s| s.hostname == hostname)
    }

    /// Get the current server config based on `context.current_server`.
    pub fn current_server(&self) -> Result<&ServerConfig> {
        let hostname = self.context.current_server.as_ref().ok_or_else(|| {
            miette!(
                help = "Add a server first with: oxiadm add-server <hostname>",
                "No server is currently selected"
            )
        })?;
        self.find_server(hostname).ok_or_else(|| {
            miette!(
                help = format!("Add the server with: oxiadm add-server {}", hostname),
                "Server '{}' is configured as current but not found in server list",
                hostname
            )
        })
    }

    /// Get a mutable reference to the current server config.
    pub fn current_server_mut(&mut self) -> Result<&mut ServerConfig> {
        let hostname = self.context.current_server.clone().ok_or_else(|| {
            miette!(
                help = "Add a server first with: oxiadm add-server <hostname>",
                "No server is currently selected"
            )
        })?;
        self.find_server_mut(&hostname).ok_or_else(|| {
            miette!(
                help = format!("Add the server with: oxiadm add-server {}", hostname),
                "Server '{}' is configured as current but not found in server list",
                hostname
            )
        })
    }

    /// Migrate legacy auth to a server entry if applicable.
    fn migrate_legacy_auth(&mut self) {
        if self.auth.is_empty() || !self.servers.is_empty() {
            return;
        }

        // Try to derive hostname from the actor's domain part
        let hostname = self
            .context
            .actor
            .as_ref()
            .and_then(|actor| actor.split('@').nth(1))
            .map(|s| s.to_string());

        if let Some(hostname) = hostname {
            let server = ServerConfig {
                hostname: hostname.clone(),
                admin_api_url: String::new(), // Will need to be re-discovered
                issuer_url: self.auth.issuer_url.clone(),
                audience: None,
                client_id: self.auth.client_id.clone(),
                client_secret: self.auth.client_secret.clone(),
                access_token: self.auth.access_token.clone(),
                refresh_token: self.auth.refresh_token.clone(),
                expires_at: self.auth.expires_at.clone(),
            };
            self.servers.push(server);
            self.context.current_server = Some(hostname);
            self.auth = AuthContext::default();
        }
    }
}

/// Returns the path to the context file: `$XDG_CONFIG_HOME/oxiadm/context.toml`
fn context_file_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().ok_or_else(|| {
        miette!(
            help = "Ensure your system has a valid home directory configured",
            "Could not determine XDG config directory"
        )
    })?;
    Ok(config_dir.join("oxiadm").join("context.toml"))
}

/// Load the context from disk. Returns a default context if the file doesn't exist.
/// Performs legacy auth migration if needed.
pub fn load_context() -> Result<OxiadmContext> {
    let path = context_file_path()?;
    if !path.exists() {
        return Ok(OxiadmContext::default());
    }
    let contents = std::fs::read_to_string(&path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read context file at {}", path.display()))?;
    let mut ctx: OxiadmContext = toml::from_str(&contents)
        .into_diagnostic()
        .wrap_err("Failed to parse context file")?;

    // Migrate legacy auth if present
    if !ctx.auth.is_empty() && ctx.servers.is_empty() {
        ctx.migrate_legacy_auth();
        // Save the migrated context
        let _ = save_context(&ctx);
    }

    Ok(ctx)
}

/// Save the context to disk, creating parent directories as needed.
pub fn save_context(ctx: &OxiadmContext) -> Result<()> {
    let path = context_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to create config directory {}", parent.display()))?;
    }
    let contents = toml::to_string_pretty(ctx)
        .into_diagnostic()
        .wrap_err("Failed to serialize context")?;
    std::fs::write(&path, contents)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to write context file at {}", path.display()))
}

/// Get the current actor from context, returning a diagnostic error if unset.
pub fn get_current_actor() -> Result<String> {
    let ctx = load_context()?;
    ctx.context.actor.ok_or_else(|| {
        miette!(
            help = "Set a default actor with: oxiadm context set user@domain",
            "No actor context is set and no --actor flag was provided"
        )
    })
}

/// Get the stored access token for the current server, returning a diagnostic error if not logged in.
pub fn get_access_token() -> Result<String> {
    let ctx = load_context()?;
    let server = ctx.current_server()?;
    server.access_token.clone().ok_or_else(|| {
        miette!(
            help = "Log in first with: oxiadm login",
            "No access token found for server '{}' — you are not logged in",
            server.hostname
        )
    })
}

/// Get the admin API URL for the current server.
pub fn get_admin_api_url() -> Result<String> {
    let ctx = load_context()?;
    let server = ctx.current_server()?;
    if server.admin_api_url.is_empty() {
        return Err(miette!(
            help = format!(
                "Re-add the server with: oxiadm add-server {}",
                server.hostname
            ),
            "No admin API URL configured for server '{}'",
            server.hostname
        ));
    }
    Ok(server.admin_api_url.clone())
}
