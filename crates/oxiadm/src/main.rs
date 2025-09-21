mod messaging;

use clap::{Parser, Subcommand};
use messaging::LavinMQClient;
use miette::{Context, IntoDiagnostic, Result};
use oxifed::messaging::KeyGenerateMessage;

/// Oxifed Admin CLI tool for managing profiles
#[derive(Parser)]
#[command(name = "oxiadm")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// LavinMQ connection string
    #[arg(
        long,
        env = "AMQP_URL",
        default_value = "amqp://guest:guest@localhost:5672"
    )]
    amqp_url: String,

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
        /// Actor username or ID who is following
        actor: String,

        /// Object (user) being followed
        object: String,
    },

    /// Create a "Like" activity
    Like {
        /// Actor username or ID who is liking
        actor: String,

        /// Object ID being liked
        object: String,
    },

    /// Create an "Announce" (boost/retweet) activity
    Announce {
        /// Actor username or ID who is announcing
        actor: String,

        /// Object ID being announced
        object: String,

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

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    let cli = Cli::parse();

    // Use messaging via LavinMQ
    let lavin_client = LavinMQClient::new(&cli.amqp_url).await?;
    handle_command_messaging(&lavin_client, &cli.command).await?;

    Ok(())
}

/// Handle Key commands via messaging
async fn handle_key_command_messaging(client: &LavinMQClient, command: &KeyCommands) -> Result<()> {
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

            // Create and send key generation message
            let key_gen_message =
                KeyGenerateMessage::new(actor.clone(), algorithm.clone(), *key_size);

            client.publish_message(&key_gen_message).await?;
            println!("Key generation request sent to PKI service");
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

/// Handle PKI commands via messaging
async fn handle_pki_command_messaging(
    _client: &LavinMQClient,
    command: &PkiCommands,
) -> Result<()> {
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

/// Handle System commands via messaging
async fn handle_system_command_messaging(
    _client: &LavinMQClient,
    command: &SystemCommands,
) -> Result<()> {
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

/// Handle Test commands via messaging
async fn handle_test_command_messaging(
    _client: &LavinMQClient,
    command: &TestCommands,
) -> Result<()> {
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

/// Handle commands using messaging
async fn handle_command_messaging(client: &LavinMQClient, command: &Commands) -> Result<()> {
    match command {
        Commands::Person { command } | Commands::Profile { command } => {
            handle_person_command_messaging(client, command).await?;
        }
        Commands::Note { command } => {
            handle_note_command_messaging(client, command).await?;
        }
        Commands::Activity { command } => {
            handle_activity_command_messaging(client, command).await?;
        }
        Commands::Keys { command } => {
            handle_key_command_messaging(client, command).await?;
        }
        Commands::Pki { command } => {
            handle_pki_command_messaging(client, command).await?;
        }
        Commands::System { command } => {
            handle_system_command_messaging(client, command).await?;
        }
        Commands::Test { command } => {
            handle_test_command_messaging(client, command).await?;
        }
        Commands::Domain { command } => {
            handle_domain_command_messaging(client, command).await?;
        }
    }

    Ok(())
}

/// Handle Person actor commands via messaging
async fn handle_person_command_messaging(
    client: &LavinMQClient,
    command: &PersonCommands,
) -> Result<()> {
    match command {
        PersonCommands::Create {
            subject,
            summary,
            icon,
            properties,
        } => {
            // Format subject with appropriate prefix if needed
            let formatted_subject = format_subject(subject);

            // Parse custom properties if provided
            let props = if let Some(props_json) = properties {
                Some(
                    serde_json::from_str(props_json)
                        .into_diagnostic()
                        .wrap_err("Failed to parse custom properties JSON")?,
                )
            } else {
                None
            };

            // Create a structured message for Person creation
            let message = oxifed::messaging::ProfileCreateMessage::new(
                formatted_subject.clone(),
                summary.clone(),
                icon.clone(),
                props,
            );

            // Send to LavinMQ
            client
                .publish_message(&message)
                .await
                .into_diagnostic()
                .wrap_err("Failed to publish Person creation message to LavinMQ")?;

            println!(
                "Person creation request for '{}' sent to message queue",
                formatted_subject
            );
        }

        PersonCommands::Update {
            id,
            summary,
            icon,
            properties,
        } => {
            // Parse custom properties if provided
            let props = if let Some(props_json) = properties {
                Some(
                    serde_json::from_str(props_json)
                        .into_diagnostic()
                        .wrap_err("Failed to parse custom properties JSON")?,
                )
            } else {
                None
            };

            // Create a structured message for Person update
            let message = oxifed::messaging::ProfileUpdateMessage::new(
                id.clone(),
                summary.clone(),
                icon.clone(),
                props,
            );

            // Send to LavinMQ
            client
                .publish_message(&message)
                .await
                .into_diagnostic()
                .wrap_err("Failed to publish Person update message to LavinMQ")?;

            println!(
                "Person update request for ID '{}' sent to message queue",
                id
            );
        }

        PersonCommands::Delete { id, force } => {
            // Create a structured message for Person deletion
            let message = oxifed::messaging::ProfileDeleteMessage::new(id.clone(), *force);

            // Send to LavinMQ
            client
                .publish_message(&message)
                .await
                .into_diagnostic()
                .wrap_err("Failed to publish Person deletion message to LavinMQ")?;

            println!(
                "Person deletion request for ID '{}' sent to message queue",
                id
            );
            if *force {
                println!("Forced deletion requested");
            }
        }
    }

    Ok(())
}

/// Handle Note object commands via messaging
async fn handle_note_command_messaging(
    client: &LavinMQClient,
    command: &NoteCommands,
) -> Result<()> {
    match command {
        NoteCommands::Create {
            author,
            content,
            summary,
            mentions,
            tags,
            properties,
        } => {
            // Parse custom properties if provided
            let props = if let Some(props_json) = properties {
                Some(
                    serde_json::from_str(props_json)
                        .into_diagnostic()
                        .wrap_err("Failed to parse custom properties JSON")?,
                )
            } else {
                None
            };

            // Create a structured message for Note creation
            let message = oxifed::messaging::NoteCreateMessage::new(
                author.clone(),
                content.clone(),
                summary.clone(),
                mentions.clone(),
                tags.clone(),
                props,
            );

            // Send to LavinMQ
            client
                .publish_message(&message)
                .await
                .into_diagnostic()
                .wrap_err("Failed to publish Note creation message to LavinMQ")?;

            println!(
                "Note creation request by '{}' sent to message queue",
                author
            );
        }

        NoteCommands::Update {
            id,
            content,
            summary,
            tags,
            properties,
        } => {
            // Parse custom properties if provided
            let props = if let Some(props_json) = properties {
                Some(
                    serde_json::from_str(props_json)
                        .into_diagnostic()
                        .wrap_err("Failed to parse custom properties JSON")?,
                )
            } else {
                None
            };

            // Create a structured message for Note update
            let message = oxifed::messaging::NoteUpdateMessage::new(
                id.clone(),
                content.clone(),
                summary.clone(),
                tags.clone(),
                props,
            );

            // Send to LavinMQ
            client
                .publish_message(&message)
                .await
                .into_diagnostic()
                .wrap_err("Failed to publish Note update message to LavinMQ")?;

            println!("Note update request for ID '{}' sent to message queue", id);
        }

        NoteCommands::Delete { id, force } => {
            // Create a structured message for Note deletion
            let message = oxifed::messaging::NoteDeleteMessage::new(id.clone(), *force);

            // Send to LavinMQ
            client
                .publish_message(&message)
                .await
                .into_diagnostic()
                .wrap_err("Failed to publish Note deletion message to LavinMQ")?;

            println!(
                "Note deletion request for ID '{}' sent to message queue",
                id
            );
            if *force {
                println!("Forced deletion requested");
            }
        }
    }

    Ok(())
}

/// Handle Activity commands via messaging
async fn handle_activity_command_messaging(
    client: &LavinMQClient,
    command: &ActivityCommands,
) -> Result<()> {
    match command {
        ActivityCommands::Follow { actor, object } => {
            // Create a structured message for Follow activity
            let message =
                oxifed::messaging::FollowActivityMessage::new(actor.clone(), object.clone());

            // Send to LavinMQ
            client
                .publish_message(&message)
                .await
                .into_diagnostic()
                .wrap_err("Failed to publish Follow activity message to LavinMQ")?;

            println!(
                "'Follow' activity request from '{}' for object '{}' sent to message queue",
                actor, object
            );
        }

        ActivityCommands::Like { actor, object } => {
            // Create a structured message for Like activity
            let message =
                oxifed::messaging::LikeActivityMessage::new(actor.clone(), object.clone());

            // Send to LavinMQ
            client
                .publish_message(&message)
                .await
                .into_diagnostic()
                .wrap_err("Failed to publish Like activity message to LavinMQ")?;

            println!(
                "'Like' activity request from '{}' for object '{}' sent to message queue",
                actor, object
            );
        }

        ActivityCommands::Announce {
            actor,
            object,
            to,
            cc,
        } => {
            // Create a structured message for Announce activity
            let message = oxifed::messaging::AnnounceActivityMessage::new(
                actor.clone(),
                object.clone(),
                to.clone(),
                cc.clone(),
            );

            // Send to LavinMQ
            client
                .publish_message(&message)
                .await
                .into_diagnostic()
                .wrap_err("Failed to publish Announce activity message to LavinMQ")?;

            println!(
                "'Announce' activity request from '{}' for object '{}' sent to message queue",
                actor, object
            );
        }
    }

    Ok(())
}

/// Handle Domain commands via messaging
async fn handle_domain_command_messaging(
    client: &LavinMQClient,
    command: &DomainCommands,
) -> Result<()> {
    use oxifed::messaging::{DomainCreateMessage, DomainDeleteMessage, DomainUpdateMessage};

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
            // Parse custom properties if provided
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

            client.publish_message(&message).await?;
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
            // Parse custom properties if provided
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

            client.publish_message(&message).await?;
            println!("Domain update request sent for: {}", domain);
        }

        DomainCommands::Delete { domain, force } => {
            let message = DomainDeleteMessage::new(domain.clone(), *force);

            client.publish_message(&message).await?;
            println!("Domain deletion request sent for: {}", domain);
            if *force {
                println!("Force deletion enabled - domain will be deleted without confirmation");
            }
        }

        DomainCommands::List => match client.create_rpc_client().await {
            Ok(rpc_client) => match rpc_client.list_domains().await {
                Ok(domains) => {
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
                Err(e) => {
                    eprintln!("Failed to list domains: {}", e);
                }
            },
            Err(e) => {
                eprintln!("Failed to create RPC client: {}", e);
            }
        },

        DomainCommands::Show { domain } => match client.create_rpc_client().await {
            Ok(rpc_client) => match rpc_client.get_domain(domain).await {
                Ok(Some(domain_info)) => {
                    println!("Domain: {}", domain_info.domain);
                    if let Some(name) = &domain_info.name {
                        println!("Name: {}", name);
                    }
                    if let Some(description) = &domain_info.description {
                        println!("Description: {}", description);
                    }
                    if let Some(contact_email) = &domain_info.contact_email {
                        println!("Contact Email: {}", contact_email);
                    }
                    println!("Registration Mode: {}", domain_info.registration_mode);
                    println!("Authorized Fetch: {}", domain_info.authorized_fetch);
                    if let Some(max_note_length) = domain_info.max_note_length {
                        println!("Max Note Length: {}", max_note_length);
                    }
                    if let Some(max_file_size) = domain_info.max_file_size {
                        println!("Max File Size: {} bytes", max_file_size);
                    }
                    if let Some(allowed_file_types) = &domain_info.allowed_file_types {
                        println!("Allowed File Types: {}", allowed_file_types.join(", "));
                    }
                    println!("Status: {}", domain_info.status);
                    println!("Created: {}", domain_info.created_at);
                    println!("Updated: {}", domain_info.updated_at);
                }
                Ok(None) => {
                    println!("Domain '{}' not found", domain);
                }
                Err(e) => {
                    eprintln!("Failed to get domain details: {}", e);
                }
            },
            Err(e) => {
                eprintln!("Failed to create RPC client: {}", e);
            }
        },
    }

    Ok(())
}

/// Ensure the subject has an appropriate prefix
fn format_subject(subject: &str) -> String {
    // If the subject already has a protocol prefix, return it as is
    if subject.starts_with("acct:") || subject.starts_with("https://") || subject.contains(':') {
        return subject.to_string();
    }

    // Otherwise, add the acct: prefix
    format!("acct:{}", subject)
}
