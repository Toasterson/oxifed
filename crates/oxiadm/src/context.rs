//! Persistent actor context for oxiadm
//!
//! Stores the current actor identity and auth tokens at
//! `$XDG_CONFIG_HOME/oxiadm/context.toml` so that they don't need
//! to be specified on every command.

use miette::{Context, IntoDiagnostic, Result, miette};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OxiadmContext {
    #[serde(default)]
    pub context: ContextInner,
    #[serde(default)]
    pub auth: AuthContext,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ContextInner {
    /// The current actor identity (e.g. "toasterson@aopc.cloud")
    pub actor: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
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
pub fn load_context() -> Result<OxiadmContext> {
    let path = context_file_path()?;
    if !path.exists() {
        return Ok(OxiadmContext::default());
    }
    let contents = std::fs::read_to_string(&path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read context file at {}", path.display()))?;
    toml::from_str(&contents)
        .into_diagnostic()
        .wrap_err("Failed to parse context file")
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

/// Get the stored access token, returning a diagnostic error if not logged in.
pub fn get_access_token() -> Result<String> {
    let ctx = load_context()?;
    ctx.auth.access_token.ok_or_else(|| {
        miette!(
            help = "Log in first with: oxiadm login --issuer-url <URL>",
            "No access token found — you are not logged in"
        )
    })
}
