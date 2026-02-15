//! WebFinger resolution for actor and target identifiers
//!
//! Resolves `user@domain` identifiers to full ActivityPub URLs via WebFinger,
//! and passes through full URLs unchanged.

use miette::{Context, IntoDiagnostic, Result, miette};
use oxifed::webfinger::WebFingerClient;

/// Returns true if the string looks like `user@domain` rather than a URL.
pub fn is_user_at_domain(s: &str) -> bool {
    !s.contains("://") && s.contains('@')
}

/// Resolve a `user@domain` identifier to its ActivityPub actor URL via WebFinger.
///
/// Performs a WebFinger lookup for `acct:user@domain` and extracts the `self` link
/// with `application/activity+json` type.
async fn resolve_webfinger(identifier: &str) -> Result<String> {
    let acct = format!("acct:{}", identifier);
    let client = WebFingerClient::new();

    let jrd = client
        .finger(&acct, None)
        .await
        .into_diagnostic()
        .wrap_err_with(|| format!("WebFinger lookup failed for '{}'", identifier))?;

    // Look for the "self" link with ActivityPub content type
    let self_link = jrd.find_link("self").ok_or_else(|| {
        miette!(
            help = format!(
                "The remote server's WebFinger response did not include a 'self' link. \
                 Verify that '{}' is a valid ActivityPub account.",
                identifier
            ),
            "WebFinger response for '{}' has no 'self' link",
            identifier
        )
    })?;

    let href = self_link.href.as_ref().ok_or_else(|| {
        miette!(
            help = format!(
                "The remote server returned a 'self' link without an href. \
                 This may indicate a misconfigured server for '{}'.",
                identifier
            ),
            "WebFinger 'self' link for '{}' has no href",
            identifier
        )
    })?;

    Ok(href.clone())
}

/// Resolve a target identifier: if it's a URL, pass it through; if it's `user@domain`, resolve via WebFinger.
pub async fn resolve_target(target: &str) -> Result<String> {
    if is_user_at_domain(target) {
        resolve_webfinger(target).await
    } else {
        Ok(target.to_string())
    }
}

/// Resolve the actor: use the explicit argument if given, otherwise fall back to the saved context.
/// Then resolve the identifier (URL or user@domain) to a full ActivityPub URL.
pub async fn resolve_actor(explicit: Option<&str>) -> Result<String> {
    let identifier = match explicit {
        Some(actor) => actor.to_string(),
        None => crate::context::get_current_actor()?,
    };
    resolve_target(&identifier).await
}
