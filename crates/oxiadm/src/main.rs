mod auth;
mod client;
mod context;
mod resolve;

use clap::{Parser, Subcommand};
use client::AdminApiClient;
use miette::{Context, IntoDiagnostic, Result};

/// Oxifed Admin CLI tool for managing profiles
#[derive(Parser)]
#[command(name = "oxiadm")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Admin API URL (overrides server context)
    #[arg(long, env = "OXIADM_API_URL")]
    api_url: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create or manage Person actors
    Person {
        #[command(subcommand)]
        command: PersonCommands,
    },

    /// Create or manage Note objects
    Note {
        #[command(subcommand)]
        command: NoteCommands,
    },

    /// Create or manage ActivityPub activities
    Activity {
        #[command(subcommand)]
        command: ActivityCommands,
    },

    /// Manage cryptographic keys
    Keys {
        #[command(subcommand)]
        command: KeyCommands,
    },

    /// PKI operations
    Pki {
        #[command(subcommand)]
        command: PkiCommands,
    },

    /// Profile management (alias for Person)
    Profile {
        #[command(subcommand)]
        command: PersonCommands,
    },

    /// System administration
    System {
        #[command(subcommand)]
        command: SystemCommands,
    },

    /// Test federation and signatures
    Test {
        #[command(subcommand)]
        command: TestCommands,
    },

    /// Domain management
    Domain {
        #[command(subcommand)]
        command: DomainCommands,
    },

    /// User management
    User {
        #[command(subcommand)]
        command: UserCommands,
    },

    /// Manage the current server/actor context
    Context {
        #[command(subcommand)]
        command: ContextCommands,
    },

    /// Authenticate with the admin API using OIDC Device Code Grant
    Login {
        /// OIDC issuer URL (overrides the server's stored issuer)
        #[arg(long, env = "OIDC_ISSUER_URL")]
        issuer_url: Option<String>,

        /// OIDC client ID (omit to use provider auto-registration)
        #[arg(long)]
        client_id: Option<String>,
    },

    /// Clear stored authentication tokens for the current server
    Logout,

    /// Add a server via WebFinger discovery and authenticate
    AddServer {
        /// Server hostname (e.g. oxifed.io)
        hostname: String,

        /// OIDC client ID (omit to use provider auto-registration)
        #[arg(long)]
        client_id: Option<String>,
    },
}

/// Commands for working with Person actors
#[derive(Subcommand)]
enum PersonCommands {
    /// Create a new Person actor
    Create {
        /// Subject identifier for the person (format: user@domain.org)
        subject: String,

        /// Bio/summary for the person
        #[arg(long)]
        summary: Option<String>,

        /// URL to profile picture
        #[arg(long)]
        icon: Option<String>,

        /// Custom properties in JSON format
        #[arg(long)]
        properties: Option<String>,
    },

    /// Update a Person actor
    Update {
        /// Username or full ActivityPub ID
        id: String,

        /// New bio/summary
        #[arg(long)]
        summary: Option<String>,

        /// New profile picture URL
        #[arg(long)]
        icon: Option<String>,

        /// Custom properties to update in JSON format
        #[arg(long)]
        properties: Option<String>,
    },

    /// Delete a Person actor
    Delete {
        /// Username or full ActivityPub ID
        id: String,

        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
}

/// Commands for working with Note objects
#[derive(Subcommand)]
enum NoteCommands {
    /// Create a new Note
    Create {
        /// Author username or ID
        author: String,

        /// Content of the note
        #[arg(long)]
        content: String,

        /// Optional summary for the note
        #[arg(long)]
        summary: Option<String>,

        /// Mentioned users (comma separated)
        #[arg(long)]
        mentions: Option<String>,

        /// Tags/hashtags (comma separated)
        #[arg(long)]
        tags: Option<String>,

        /// Custom properties in JSON format
        #[arg(long)]
        properties: Option<String>,
    },

    /// Update a Note
    Update {
        /// Note ID
        id: String,

        /// New content
        #[arg(long)]
        content: Option<String>,

        /// New summary
        #[arg(long)]
        summary: Option<String>,

        /// Add or update tags (comma separated)
        #[arg(long)]
        tags: Option<String>,

        /// Custom properties to update in JSON format
        #[arg(long)]
        properties: Option<String>,
    },

    /// Delete a Note
    Delete {
        /// Note ID
        id: String,

        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
}

/// Commands for working with ActivityPub activities
#[derive(Subcommand)]
enum ActivityCommands {
    /// Create a "Follow" activity
    Follow {
        /// Target to follow (user@domain or full URL)
        object: String,

        /// Actor performing the follow (overrides context)
        #[arg(long)]
        actor: Option<String>,
    },

    /// List accounts the actor is following and their status
    Following {
        /// Actor to query (overrides context, format: user@domain or full URL)
        #[arg(long)]
        actor: Option<String>,
    },

    /// List followers of the actor and their status
    Followers {
        /// Actor to query (overrides context, format: user@domain or full URL)
        #[arg(long)]
        actor: Option<String>,
    },

    /// Create a "Like" activity
    Like {
        /// Object to like (user@domain or full URL)
        object: String,

        /// Actor performing the like (overrides context)
        #[arg(long)]
        actor: Option<String>,
    },

    /// Create an "Announce" (boost/retweet) activity
    Announce {
        /// Object to announce (user@domain or full URL)
        object: String,

        /// Actor performing the announce (overrides context)
        #[arg(long)]
        actor: Option<String>,

        /// Target audience (optional)
        #[arg(long)]
        to: Option<String>,

        /// CC audience (optional)
        #[arg(long)]
        cc: Option<String>,
    },
}

/// Commands for managing cryptographic keys
#[derive(Subcommand)]
enum KeyCommands {
    /// Generate a new keypair for an actor
    Generate {
        /// Actor identifier (user@domain.com)
        #[arg(long)]
        actor: String,

        /// Algorithm (rsa or ed25519)
        #[arg(long)]
        algorithm: String,

        /// Key size for RSA (2048, 4096)
        #[arg(long)]
        key_size: Option<u32>,
    },

    /// Import existing keypair (BYOK)
    Import {
        /// Actor identifier (user@domain.com)
        #[arg(long)]
        actor: String,

        /// Path to public key PEM file
        #[arg(long)]
        public_key: String,

        /// Path to private key PEM file
        #[arg(long)]
        private_key: String,

        /// Algorithm (rsa or ed25519)
        #[arg(long)]
        algorithm: String,
    },

    /// Initiate domain verification for a key
    Verify {
        /// Actor identifier
        #[arg(long)]
        actor: String,

        /// Domain to verify
        #[arg(long)]
        domain: String,
    },

    /// Complete domain verification with challenge response
    VerifyComplete {
        /// Actor identifier
        #[arg(long)]
        actor: String,

        /// Domain being verified
        #[arg(long)]
        domain: String,

        /// Path to signed challenge response
        #[arg(long)]
        challenge_response: String,
    },

    /// Rotate a key
    Rotate {
        /// Actor identifier
        #[arg(long)]
        actor: String,

        /// Rotation type (scheduled or emergency)
        #[arg(long)]
        rotation_type: String,
    },

    /// View trust chain for a key
    TrustChain {
        /// Key ID URL
        #[arg(long)]
        key_id: String,
    },

    /// List keys by trust level
    List {
        /// Trust level filter
        #[arg(long)]
        trust_level: Option<String>,
    },
}

/// Commands for PKI administration
#[derive(Subcommand)]
enum PkiCommands {
    /// Initialize master key (one-time setup)
    InitMaster {
        /// Key size
        #[arg(long, default_value = "4096")]
        key_size: u32,

        /// Output file for key
        #[arg(long)]
        output: String,
    },

    /// Backup master key
    BackupMaster {
        /// Output file for backup
        #[arg(long)]
        output: String,

        /// Encrypt the backup
        #[arg(long)]
        encrypt: bool,
    },

    /// Generate domain key
    GenerateDomainKey {
        /// Domain name
        #[arg(long)]
        domain: String,
    },

    /// Sign domain key with master key
    SignDomainKey {
        /// Domain name
        #[arg(long)]
        domain: String,

        /// Path to master key
        #[arg(long)]
        master_key: String,
    },

    /// List all domains
    ListDomains,

    /// Recover from master key compromise
    RecoverMaster {
        /// Recovery token file
        #[arg(long)]
        recovery_token: String,

        /// New master key file
        #[arg(long)]
        new_master_key: String,
    },

    /// Recover user access with domain authority
    RecoverUser {
        /// Actor identifier
        #[arg(long)]
        actor: String,

        /// Domain name
        #[arg(long)]
        domain: String,

        /// Recovery method
        #[arg(long)]
        method: String,
    },
}

/// Commands for system administration
#[derive(Subcommand)]
enum SystemCommands {
    /// Check system health
    Health,

    /// View PKI status
    PkiStatus,

    /// Generate system report
    Report {
        /// Output file
        #[arg(long)]
        output: String,
    },

    /// Set domain configuration
    SetDomain {
        /// Domain name
        domain: String,

        /// Enable authorized fetch
        #[arg(long)]
        authorized_fetch: Option<bool>,

        /// Registration mode
        #[arg(long)]
        registration_mode: Option<String>,
    },

    /// Set instance settings
    SetInstance {
        /// Maximum note length
        #[arg(long)]
        max_note_length: Option<u32>,

        /// Maximum file size
        #[arg(long)]
        max_file_size: Option<String>,
    },
}

/// Commands for testing federation
#[derive(Subcommand)]
enum TestCommands {
    /// Test HTTP signature generation and verification
    Signatures {
        /// Actor identifier
        #[arg(long)]
        actor: String,

        /// Target URL
        #[arg(long)]
        target: String,
    },

    /// Verify federation connectivity
    Federation {
        /// Local actor
        #[arg(long)]
        actor: String,

        /// Remote actor
        #[arg(long)]
        remote_actor: String,
    },

    /// Test authorized fetch capability
    AuthorizedFetch {
        /// Actor identifier
        #[arg(long)]
        actor: String,

        /// Target URL
        #[arg(long)]
        target: String,
    },
}

/// Commands for managing domains
#[derive(Subcommand)]
enum DomainCommands {
    /// Create a new domain
    Create {
        /// Domain name
        domain: String,

        /// Domain display name
        #[arg(long)]
        name: Option<String>,

        /// Domain description
        #[arg(long)]
        description: Option<String>,

        /// Contact email for the domain
        #[arg(long)]
        contact_email: Option<String>,

        /// Domain rules (can be specified multiple times)
        #[arg(long)]
        rules: Option<Vec<String>>,

        /// Registration mode (open, approval, invite, closed)
        #[arg(long, default_value = "approval")]
        registration_mode: Option<String>,

        /// Enable authorized fetch
        #[arg(long, default_value = "false")]
        authorized_fetch: Option<bool>,

        /// Maximum note length
        #[arg(long)]
        max_note_length: Option<i32>,

        /// Maximum file size in bytes
        #[arg(long)]
        max_file_size: Option<i64>,

        /// Allowed file types (can be specified multiple times)
        #[arg(long)]
        allowed_file_types: Option<Vec<String>>,

        /// Additional properties as JSON
        #[arg(long)]
        properties: Option<String>,
    },

    /// Update an existing domain
    Update {
        /// Domain name
        domain: String,

        /// Domain display name
        #[arg(long)]
        name: Option<String>,

        /// Domain description
        #[arg(long)]
        description: Option<String>,

        /// Contact email for the domain
        #[arg(long)]
        contact_email: Option<String>,

        /// Domain rules (can be specified multiple times)
        #[arg(long)]
        rules: Option<Vec<String>>,

        /// Registration mode (open, approval, invite, closed)
        #[arg(long)]
        registration_mode: Option<String>,

        /// Enable authorized fetch
        #[arg(long)]
        authorized_fetch: Option<bool>,

        /// Maximum note length
        #[arg(long)]
        max_note_length: Option<i32>,

        /// Maximum file size in bytes
        #[arg(long)]
        max_file_size: Option<i64>,

        /// Allowed file types (can be specified multiple times)
        #[arg(long)]
        allowed_file_types: Option<Vec<String>>,

        /// Additional properties as JSON
        #[arg(long)]
        properties: Option<String>,
    },

    /// Delete a domain
    Delete {
        /// Domain name
        domain: String,

        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },

    /// List all domains
    List,

    /// Show domain details
    Show {
        /// Domain name
        domain: String,
    },
}

/// Commands for managing users
#[derive(Subcommand)]
enum UserCommands {
    /// Create a new user
    Create {
        /// Username (required)
        #[arg(long, short = 'u')]
        username: String,

        /// Display name (optional, defaults to username)
        #[arg(long, short = 'd')]
        display_name: Option<String>,

        /// Domain the user belongs to (required)
        #[arg(long)]
        domain: String,
    },

    /// List existing users
    List,

    /// Show user details including public key
    Show {
        /// Username to show
        username: String,
    },
}

/// Commands for managing the server/actor context
#[derive(Subcommand)]
enum ContextCommands {
    /// Set the current server and/or actor
    ///
    /// Pass a hostname (e.g. "oxifed.io") to set the server,
    /// or a user@domain (e.g. "toasterson@oxifed.io") to set both server and actor.
    Set {
        /// Server hostname or actor identifier (user@domain)
        identity: String,
    },

    /// Show the current context (server, auth status, actor)
    Show,

    /// Clear the current context
    Clear,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    let cli = Cli::parse();

    // Handle commands that don't need network / API client
    match &cli.command {
        Commands::Context { command } => return handle_context_command(command),
        Commands::Login {
            issuer_url,
            client_id,
        } => return auth::device_code_login(issuer_url.as_deref(), client_id.as_deref()).await,
        Commands::Logout => return auth::logout(),
        Commands::AddServer {
            hostname,
            client_id,
        } => return handle_add_server(hostname, client_id.as_deref()).await,
        _ => {}
    }

    // Resolve the API URL: explicit --api-url > env > context
    let api_url = match cli.api_url {
        Some(url) => url,
        None => context::get_admin_api_url()?,
    };

    // Refresh token if needed, then create API client
    auth::refresh_token_if_needed().await?;
    let access_token = context::get_access_token()?;
    let api_client = AdminApiClient::new(&api_url, access_token).await?;

    handle_command(&api_client, &cli.command).await?;

    Ok(())
}

/// Handle the `add-server` command: WebFinger discovery + OIDC login
async fn handle_add_server(hostname: &str, client_id: Option<&str>) -> Result<()> {
    use oxifed::webfinger::WebFingerClient;

    println!("Discovering server configuration for '{}'...", hostname);

    let wf_client = WebFingerClient::new();
    let resource = format!("https://{}", hostname);
    let jrd = wf_client
        .finger(&resource, None)
        .await
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "WebFinger discovery failed for '{}'. Is the server running and reachable?",
                hostname
            )
        })?;

    // Extract admin API URL
    let admin_api_url = jrd
        .find_link("https://oxifed.io/ns/admin-api")
        .and_then(|link| link.href.as_ref())
        .ok_or_else(|| {
            miette::miette!(
                help = format!(
                    "The server at '{}' did not advertise an admin API endpoint. \
                     Ensure domainservd is configured with ADMIN_API_URL.",
                    hostname
                ),
                "No admin-api link found in WebFinger response for '{}'",
                hostname
            )
        })?
        .clone();

    // Extract OIDC issuer URL
    let issuer_url = jrd
        .find_link("http://openid.net/specs/connect/1.0/issuer")
        .and_then(|link| link.href.as_ref())
        .ok_or_else(|| {
            miette::miette!(
                help = format!(
                    "The server at '{}' did not advertise an OIDC issuer. \
                     Ensure domainservd is configured with OIDC_ISSUER_URL.",
                    hostname
                ),
                "No OIDC issuer link found in WebFinger response for '{}'",
                hostname
            )
        })?
        .clone();

    // Extract optional OAuth audience
    let audience = jrd
        .find_link("https://oxifed.io/ns/oauth-audience")
        .and_then(|link| link.href.clone());

    println!("  Admin API: {}", admin_api_url);
    println!("  OIDC Issuer: {}", issuer_url);
    if let Some(ref aud) = audience {
        println!("  OAuth Audience: {}", aud);
    }
    println!();

    // Perform OIDC device code login
    auth::device_code_login_for_server(
        hostname,
        &admin_api_url,
        &issuer_url,
        client_id,
        audience.as_deref(),
    )
    .await?;

    println!();
    println!("Server '{}' added and set as current.", hostname);
    println!(
        "Hint: create a person with: oxiadm person create user@{}",
        hostname
    );
    println!(
        "      then set actor with: oxiadm context set user@{}",
        hostname
    );

    Ok(())
}

/// Handle context commands (no network needed)
fn handle_context_command(command: &ContextCommands) -> Result<()> {
    match command {
        ContextCommands::Set { identity } => {
            let mut ctx = context::load_context()?;

            if identity.contains('@') {
                // user@domain format: set both server and actor
                let domain = identity
                    .split('@')
                    .nth(1)
                    .ok_or_else(|| miette::miette!("Invalid identity format: {}", identity))?;

                // Validate the server exists
                if ctx.find_server(domain).is_none() {
                    return Err(miette::miette!(
                        help = format!("Add the server first with: oxiadm add-server {}", domain),
                        "Server '{}' is not configured",
                        domain
                    ));
                }

                ctx.context.current_server = Some(domain.to_string());
                ctx.context.actor = Some(identity.clone());
                context::save_context(&ctx)?;
                println!("Server set to: {}", domain);
                println!("Actor set to: {}", identity);
            } else {
                // hostname only: set server, clear actor
                if ctx.find_server(identity).is_none() {
                    return Err(miette::miette!(
                        help = format!("Add the server first with: oxiadm add-server {}", identity),
                        "Server '{}' is not configured",
                        identity
                    ));
                }

                ctx.context.current_server = Some(identity.clone());
                ctx.context.actor = None;
                context::save_context(&ctx)?;
                println!("Server set to: {}", identity);
                println!(
                    "Actor cleared (set with: oxiadm context set user@{})",
                    identity
                );
            }
        }
        ContextCommands::Show => {
            let ctx = context::load_context()?;

            // Current server
            match &ctx.context.current_server {
                Some(server) => {
                    println!("Current server: {}", server);
                    if let Some(s) = ctx.find_server(server) {
                        println!("  Admin API: {}", s.admin_api_url);
                        if let Some(ref issuer) = s.issuer_url {
                            println!("  OIDC Issuer: {}", issuer);
                        }
                        if s.access_token.is_some() {
                            let status = if let Some(ref expires_at) = s.expires_at {
                                match chrono::DateTime::parse_from_rfc3339(expires_at) {
                                    Ok(expiry) if chrono::Utc::now() < expiry => {
                                        format!("authenticated (expires {})", expires_at)
                                    }
                                    _ => "token expired".to_string(),
                                }
                            } else {
                                "authenticated".to_string()
                            };
                            println!("  Auth: {}", status);
                        } else {
                            println!("  Auth: not logged in");
                        }
                    }
                }
                None => println!("No server selected"),
            }

            // Current actor
            match &ctx.context.actor {
                Some(actor) => println!("Current actor: {}", actor),
                None => println!("No actor set"),
            }

            // List all configured servers
            if !ctx.servers.is_empty() {
                println!();
                println!("Configured servers:");
                for s in &ctx.servers {
                    let current = ctx
                        .context
                        .current_server
                        .as_ref()
                        .is_some_and(|cs| cs == &s.hostname);
                    let marker = if current { " *" } else { "" };
                    let auth = if s.access_token.is_some() {
                        "logged in"
                    } else {
                        "no auth"
                    };
                    println!("  {}{} ({})", s.hostname, marker, auth);
                }
            }
        }
        ContextCommands::Clear => {
            let mut ctx = context::load_context()?;
            ctx.context.actor = None;
            ctx.context.current_server = None;
            context::save_context(&ctx)?;
            println!("Context cleared (server and actor)");
        }
    }
    Ok(())
}

/// Handle all commands that require the API client
async fn handle_command(client: &AdminApiClient, command: &Commands) -> Result<()> {
    match command {
        Commands::Person { command } | Commands::Profile { command } => {
            handle_person_command(client, command).await?;
        }
        Commands::Note { command } => {
            handle_note_command(client, command).await?;
        }
        Commands::Activity { command } => {
            handle_activity_command(client, command).await?;
        }
        Commands::Keys { command } => {
            handle_key_command(client, command).await?;
        }
        Commands::Pki { command } => {
            handle_pki_command(command)?;
        }
        Commands::System { command } => {
            handle_system_command(command)?;
        }
        Commands::Test { command } => {
            handle_test_command(command)?;
        }
        Commands::Domain { command } => {
            handle_domain_command(client, command).await?;
        }
        Commands::User { command } => {
            handle_user_command(client, command).await?;
        }
        Commands::Context { .. }
        | Commands::Login { .. }
        | Commands::Logout
        | Commands::AddServer { .. } => {
            unreachable!("Handled before API client creation");
        }
    }
    Ok(())
}

/// Handle Person actor commands
async fn handle_person_command(client: &AdminApiClient, command: &PersonCommands) -> Result<()> {
    match command {
        PersonCommands::Create {
            subject,
            summary,
            icon,
            properties,
        } => {
            let formatted_subject = format_subject(subject);

            let props = if let Some(props_json) = properties {
                Some(
                    serde_json::from_str(props_json)
                        .into_diagnostic()
                        .wrap_err("Failed to parse custom properties JSON")?,
                )
            } else {
                None
            };

            let message = oxifed::messaging::ProfileCreateMessage::new(
                formatted_subject.clone(),
                summary.clone(),
                icon.clone(),
                props,
            );

            client.create_person(&message).await?;
            println!("Person creation request for '{}' sent", formatted_subject);
        }

        PersonCommands::Update {
            id,
            summary,
            icon,
            properties,
        } => {
            let props = if let Some(props_json) = properties {
                Some(
                    serde_json::from_str(props_json)
                        .into_diagnostic()
                        .wrap_err("Failed to parse custom properties JSON")?,
                )
            } else {
                None
            };

            let message = oxifed::messaging::ProfileUpdateMessage::new(
                id.clone(),
                summary.clone(),
                icon.clone(),
                props,
            );

            client.update_person(&message).await?;
            println!("Person update request for ID '{}' sent", id);
        }

        PersonCommands::Delete { id, force } => {
            client.delete_person(id, *force).await?;
            println!("Person deletion request for ID '{}' sent", id);
            if *force {
                println!("Forced deletion requested");
            }
        }
    }

    Ok(())
}

/// Handle Note object commands
async fn handle_note_command(client: &AdminApiClient, command: &NoteCommands) -> Result<()> {
    match command {
        NoteCommands::Create {
            author,
            content,
            summary,
            mentions,
            tags,
            properties,
        } => {
            let props = if let Some(props_json) = properties {
                Some(
                    serde_json::from_str(props_json)
                        .into_diagnostic()
                        .wrap_err("Failed to parse custom properties JSON")?,
                )
            } else {
                None
            };

            let message = oxifed::messaging::NoteCreateMessage::new(
                author.clone(),
                content.clone(),
                summary.clone(),
                mentions.clone(),
                tags.clone(),
                props,
            );

            client.create_note(&message).await?;
            println!("Note creation request by '{}' sent", author);
        }

        NoteCommands::Update {
            id,
            content,
            summary,
            tags,
            properties,
        } => {
            let props = if let Some(props_json) = properties {
                Some(
                    serde_json::from_str(props_json)
                        .into_diagnostic()
                        .wrap_err("Failed to parse custom properties JSON")?,
                )
            } else {
                None
            };

            let message = oxifed::messaging::NoteUpdateMessage::new(
                id.clone(),
                content.clone(),
                summary.clone(),
                tags.clone(),
                props,
            );

            client.update_note(&message).await?;
            println!("Note update request for ID '{}' sent", id);
        }

        NoteCommands::Delete { id, force } => {
            client.delete_note(id, *force).await?;
            println!("Note deletion request for ID '{}' sent", id);
            if *force {
                println!("Forced deletion requested");
            }
        }
    }

    Ok(())
}

/// Handle Activity commands
async fn handle_activity_command(
    client: &AdminApiClient,
    command: &ActivityCommands,
) -> Result<()> {
    match command {
        ActivityCommands::Follow { actor, object } => {
            let resolved_actor = resolve::resolve_actor(actor.as_deref()).await?;
            let resolved_object = resolve::resolve_target(object).await?;

            client.follow(&resolved_actor, &resolved_object).await?;
            println!(
                "'Follow' activity from '{}' for '{}' sent",
                resolved_actor, resolved_object
            );
        }

        ActivityCommands::Like { actor, object } => {
            let resolved_actor = resolve::resolve_actor(actor.as_deref()).await?;
            let resolved_object = resolve::resolve_target(object).await?;

            client.like(&resolved_actor, &resolved_object).await?;
            println!(
                "'Like' activity from '{}' for '{}' sent",
                resolved_actor, resolved_object
            );
        }

        ActivityCommands::Following { actor } => {
            let resolved_actor = resolve::resolve_actor(actor.as_deref()).await?;

            let follows = client.list_following(&resolved_actor).await?;
            if follows.is_empty() {
                println!("{} is not following anyone", resolved_actor);
            } else {
                println!("Following ({}):", follows.len());
                for f in &follows {
                    let status_indicator = match f.status.as_str() {
                        "accepted" => "[accepted]",
                        "pending" => "[pending] ",
                        "rejected" => "[rejected]",
                        _ => &f.status,
                    };
                    println!(
                        "  {} {} (since {})",
                        status_indicator, f.following, f.created_at
                    );
                }
            }
        }

        ActivityCommands::Followers { actor } => {
            let resolved_actor = resolve::resolve_actor(actor.as_deref()).await?;

            let follows = client.list_followers(&resolved_actor).await?;
            if follows.is_empty() {
                println!("{} has no followers", resolved_actor);
            } else {
                println!("Followers ({}):", follows.len());
                for f in &follows {
                    let status_indicator = match f.status.as_str() {
                        "accepted" => "[accepted]",
                        "pending" => "[pending] ",
                        "rejected" => "[rejected]",
                        _ => &f.status,
                    };
                    println!(
                        "  {} {} (since {})",
                        status_indicator, f.follower, f.created_at
                    );
                }
            }
        }

        ActivityCommands::Announce {
            actor,
            object,
            to,
            cc,
        } => {
            let resolved_actor = resolve::resolve_actor(actor.as_deref()).await?;
            let resolved_object = resolve::resolve_target(object).await?;

            client
                .announce(&resolved_actor, &resolved_object, to.clone(), cc.clone())
                .await?;
            println!(
                "'Announce' activity from '{}' for '{}' sent",
                resolved_actor, resolved_object
            );
        }
    }

    Ok(())
}

/// Handle Key commands
async fn handle_key_command(client: &AdminApiClient, command: &KeyCommands) -> Result<()> {
    match command {
        KeyCommands::Generate {
            actor,
            algorithm,
            key_size,
        } => {
            println!("Generating {} key for '{}'", algorithm, actor);
            if let Some(size) = key_size {
                println!("Key size: {}", size);
            }

            client.generate_key(actor, algorithm, *key_size).await?;
            println!("Key generation request sent");
        }

        KeyCommands::Import {
            actor,
            public_key,
            private_key,
            algorithm,
        } => {
            println!("Importing {} key for '{}'", algorithm, actor);
            println!("Public key: {}", public_key);
            println!("Private key: {}", private_key);
            println!("Key import request sent to PKI service");
        }

        KeyCommands::Verify { actor, domain } => {
            println!(
                "Initiating domain verification for '{}' on domain '{}'",
                actor, domain
            );
            println!("Domain verification request sent to PKI service");
        }

        KeyCommands::VerifyComplete {
            actor,
            domain,
            challenge_response,
        } => {
            println!(
                "Completing domain verification for '{}' on domain '{}'",
                actor, domain
            );
            println!("Challenge response file: {}", challenge_response);
            println!("Verification completion request sent to PKI service");
        }

        KeyCommands::Rotate {
            actor,
            rotation_type,
        } => {
            println!("Rotating key for '{}' with type '{}'", actor, rotation_type);
            println!("Key rotation request sent to PKI service");
        }

        KeyCommands::TrustChain { key_id } => {
            println!("Viewing trust chain for key: {}", key_id);
            println!("Trust chain request sent to PKI service");
        }

        KeyCommands::List { trust_level } => {
            if let Some(level) = trust_level {
                println!("Listing keys with trust level: {}", level);
            } else {
                println!("Listing all keys");
            }
            println!("Key list request sent to PKI service");
        }
    }

    Ok(())
}

/// Handle PKI commands (mostly stubs for now)
fn handle_pki_command(command: &PkiCommands) -> Result<()> {
    match command {
        PkiCommands::InitMaster { key_size, output } => {
            println!("Initializing master key with size {} bits", key_size);
            println!("Output file: {}", output);
            println!("Master key initialization request sent to PKI service");
        }

        PkiCommands::BackupMaster { output, encrypt } => {
            println!("Backing up master key to: {}", output);
            if *encrypt {
                println!("Backup will be encrypted");
            }
            println!("Master key backup request sent to PKI service");
        }

        PkiCommands::GenerateDomainKey { domain } => {
            println!("Generating domain key for: {}", domain);
            println!("Domain key generation request sent to PKI service");
        }

        PkiCommands::SignDomainKey { domain, master_key } => {
            println!(
                "Signing domain key for '{}' with master key: {}",
                domain, master_key
            );
            println!("Domain key signing request sent to PKI service");
        }

        PkiCommands::ListDomains => {
            println!("Listing all domains");
            println!("Domain list request sent to PKI service");
        }

        PkiCommands::RecoverMaster {
            recovery_token,
            new_master_key,
        } => {
            println!("Recovering master key");
            println!("Recovery token: {}", recovery_token);
            println!("New master key: {}", new_master_key);
            println!("Master key recovery request sent to PKI service");
        }

        PkiCommands::RecoverUser {
            actor,
            domain,
            method,
        } => {
            println!(
                "Recovering user access for '{}' on domain '{}' using method '{}'",
                actor, domain, method
            );
            println!("User recovery request sent to PKI service");
        }
    }

    Ok(())
}

/// Handle System commands (mostly stubs for now)
fn handle_system_command(command: &SystemCommands) -> Result<()> {
    match command {
        SystemCommands::Health => {
            println!("Checking system health");
            println!("Health check request sent to system service");
        }

        SystemCommands::PkiStatus => {
            println!("Checking PKI status");
            println!("PKI status request sent to system service");
        }

        SystemCommands::Report { output } => {
            println!("Generating system report to: {}", output);
            println!("System report request sent to system service");
        }

        SystemCommands::SetDomain {
            domain,
            authorized_fetch,
            registration_mode,
        } => {
            println!("Setting domain configuration for: {}", domain);
            if let Some(af) = authorized_fetch {
                println!("Authorized fetch: {}", af);
            }
            if let Some(mode) = registration_mode {
                println!("Registration mode: {}", mode);
            }
            println!("Domain configuration request sent to system service");
        }

        SystemCommands::SetInstance {
            max_note_length,
            max_file_size,
        } => {
            println!("Setting instance configuration");
            if let Some(length) = max_note_length {
                println!("Max note length: {}", length);
            }
            if let Some(size) = max_file_size {
                println!("Max file size: {}", size);
            }
            println!("Instance configuration request sent to system service");
        }
    }

    Ok(())
}

/// Handle Test commands (stubs)
fn handle_test_command(command: &TestCommands) -> Result<()> {
    match command {
        TestCommands::Signatures { actor, target } => {
            println!(
                "Testing HTTP signatures for '{}' to target: {}",
                actor, target
            );
            println!("Signature test request sent to federation service");
        }

        TestCommands::Federation {
            actor,
            remote_actor,
        } => {
            println!(
                "Testing federation connectivity from '{}' to '{}'",
                actor, remote_actor
            );
            println!("Federation test request sent to federation service");
        }

        TestCommands::AuthorizedFetch { actor, target } => {
            println!(
                "Testing authorized fetch for '{}' to target: {}",
                actor, target
            );
            println!("Authorized fetch test request sent to federation service");
        }
    }

    Ok(())
}

/// Handle Domain commands
async fn handle_domain_command(client: &AdminApiClient, command: &DomainCommands) -> Result<()> {
    use oxifed::messaging::{DomainCreateMessage, DomainUpdateMessage};

    match command {
        DomainCommands::Create {
            domain,
            name,
            description,
            contact_email,
            rules,
            registration_mode,
            authorized_fetch,
            max_note_length,
            max_file_size,
            allowed_file_types,
            properties,
        } => {
            let props = if let Some(props_json) = properties {
                Some(
                    serde_json::from_str(props_json)
                        .map_err(|e| miette::miette!("Failed to parse properties JSON: {}", e))?,
                )
            } else {
                None
            };

            let message = DomainCreateMessage::new(
                domain.clone(),
                name.clone(),
                description.clone(),
                contact_email.clone(),
                rules.clone(),
                registration_mode.clone(),
                *authorized_fetch,
                *max_note_length,
                *max_file_size,
                allowed_file_types.clone(),
                props,
            );

            client.create_domain(&message).await?;
            println!("Domain creation request sent for: {}", domain);
        }

        DomainCommands::Update {
            domain,
            name,
            description,
            contact_email,
            rules,
            registration_mode,
            authorized_fetch,
            max_note_length,
            max_file_size,
            allowed_file_types,
            properties,
        } => {
            let props = if let Some(props_json) = properties {
                Some(
                    serde_json::from_str(props_json)
                        .map_err(|e| miette::miette!("Failed to parse properties JSON: {}", e))?,
                )
            } else {
                None
            };

            let message = DomainUpdateMessage::new(
                domain.clone(),
                name.clone(),
                description.clone(),
                contact_email.clone(),
                rules.clone(),
                registration_mode.clone(),
                *authorized_fetch,
                *max_note_length,
                *max_file_size,
                allowed_file_types.clone(),
                props,
            );

            client.update_domain(&message).await?;
            println!("Domain update request sent for: {}", domain);
        }

        DomainCommands::Delete { domain, force } => {
            client.delete_domain(domain, *force).await?;
            println!("Domain deletion request sent for: {}", domain);
            if *force {
                println!("Force deletion enabled — domain will be deleted without confirmation");
            }
        }

        DomainCommands::List => {
            let domains = client.list_domains().await?;
            if domains.is_empty() {
                println!("No domains registered");
            } else {
                println!("Registered domains:");
                for domain in domains {
                    println!(
                        "  {} - {} ({})",
                        domain.domain,
                        domain.name.unwrap_or_else(|| "No name".to_string()),
                        domain.status
                    );
                }
            }
        }

        DomainCommands::Show { domain } => {
            let domain_info = client.get_domain(domain).await?;
            match domain_info {
                Some(d) => {
                    println!("Domain: {}", d.domain);
                    if let Some(name) = &d.name {
                        println!("Name: {}", name);
                    }
                    if let Some(description) = &d.description {
                        println!("Description: {}", description);
                    }
                    if let Some(contact_email) = &d.contact_email {
                        println!("Contact Email: {}", contact_email);
                    }
                    println!("Registration Mode: {}", d.registration_mode);
                    println!("Authorized Fetch: {}", d.authorized_fetch);
                    if let Some(max_note_length) = d.max_note_length {
                        println!("Max Note Length: {}", max_note_length);
                    }
                    if let Some(max_file_size) = d.max_file_size {
                        println!("Max File Size: {} bytes", max_file_size);
                    }
                    if let Some(allowed_file_types) = &d.allowed_file_types {
                        println!("Allowed File Types: {}", allowed_file_types.join(", "));
                    }
                    println!("Status: {}", d.status);
                    println!("Created: {}", d.created_at);
                    println!("Updated: {}", d.updated_at);
                }
                None => {
                    println!("Domain '{}' not found", domain);
                }
            }
        }
    }

    Ok(())
}

/// Handle User commands
async fn handle_user_command(client: &AdminApiClient, command: &UserCommands) -> Result<()> {
    use oxifed::messaging::UserCreateMessage;

    match command {
        UserCommands::Create {
            username,
            display_name,
            domain,
        } => {
            let message =
                UserCreateMessage::new(username.clone(), display_name.clone(), domain.clone());

            client.create_user(&message).await?;
            println!("User creation request for '{}@{}' sent", username, domain);
            if let Some(display_name) = display_name {
                println!("Display name: {}", display_name);
            }
        }

        UserCommands::List => {
            let users = client.list_users().await?;
            if users.is_empty() {
                println!("No users found");
            } else {
                println!("Registered users:");
                for user in users {
                    println!(
                        "  {}@{} - {} ({})",
                        user.username,
                        user.domain,
                        user.display_name
                            .unwrap_or_else(|| "No display name".to_string()),
                        user.actor_id
                    );
                }
            }
        }

        UserCommands::Show { username } => {
            let user_info = client.get_user(username).await?;
            match user_info {
                Some(u) => {
                    println!("Username: {}", u.username);
                    if let Some(display_name) = &u.display_name {
                        println!("Display Name: {}", display_name);
                    }
                    println!("Domain: {}", u.domain);
                    println!("Actor ID: {}", u.actor_id);
                    if let Some(public_key) = &u.public_key {
                        println!("Public Key: {}", public_key);
                    }
                    println!("Private Key Stored: {}", u.private_key_stored);
                    println!("Created: {}", u.created_at);
                    println!("Updated: {}", u.updated_at);
                }
                None => {
                    println!("User '{}' not found", username);
                }
            }
        }
    }

    Ok(())
}

/// Ensure the subject has an appropriate prefix
fn format_subject(subject: &str) -> String {
    if subject.starts_with("acct:") || subject.starts_with("https://") || subject.contains(':') {
        return subject.to_string();
    }
    format!("acct:{}", subject)
}
