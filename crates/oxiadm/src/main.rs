mod messaging;

use clap::{Parser, Subcommand};
use messaging::LavinMQClient;
use miette::{Context, IntoDiagnostic, Result};

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

/// Handle commands using messaging
async fn handle_command_messaging(client: &LavinMQClient, command: &Commands) -> Result<()> {
    match command {
        Commands::Person { command } => {
            handle_person_command_messaging(client, command).await?;
        }
        Commands::Note { command } => {
            handle_note_command_messaging(client, command).await?;
        }
        Commands::Activity { command } => {
            handle_activity_command_messaging(client, command).await?;
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

/// Ensure the subject has an appropriate prefix
fn format_subject(subject: &str) -> String {
    // If the subject already has a protocol prefix, return it as is
    if subject.starts_with("acct:") || subject.starts_with("https://") || subject.contains(':') {
        return subject.to_string();
    }

    // Otherwise, add the acct: prefix
    format!("acct:{}", subject)
}
