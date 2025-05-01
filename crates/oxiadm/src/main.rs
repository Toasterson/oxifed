mod messaging;

use clap::{Parser, Subcommand};
use messaging::LavinMQClient;
use miette::{Context, IntoDiagnostic, Result};
use oxifed::messaging::ProfileCreateMessage;
use std::collections::HashMap;

/// Oxifed Admin CLI tool for managing profiles
#[derive(Parser)]
#[command(name = "oxiadm")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// LavinMQ connection string
    #[arg(long, env = "AMQP_URL", default_value = "amqp://guest:guest@localhost:5672")]
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
        
        /// Display name for the person
        #[arg(long)]
        name: Option<String>,
        
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
        
        /// New display name
        #[arg(long)]
        name: Option<String>,
        
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
        
        /// Optional title for the note
        #[arg(long)]
        name: Option<String>,
        
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
        
        /// New title
        #[arg(long)]
        name: Option<String>,
        
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
    
    /// Delete an activity
    Delete {
        /// Activity ID
        id: String,
        
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

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
        },
        Commands::Note { command } => {
            handle_note_command_messaging(client, command).await?;
        },
        Commands::Activity { command } => {
            handle_activity_command_messaging(client, command).await?;
        }
    }
    
    Ok(())
}

/// Handle Person actor commands via messaging
async fn handle_person_command_messaging(client: &LavinMQClient, command: &PersonCommands) -> Result<()> {
    match command {
        PersonCommands::Create { 
            subject, 
            name, 
            summary, 
            icon, 
            properties 
        } => {
            // Format subject with appropriate prefix if needed
            let formatted_subject = format_subject(subject);
            
            // Create a message describing the Person creation
            let mut message = serde_json::json!({
                "action": "create_person",
                "subject": formatted_subject,
                "name": name,
                "summary": summary,
                "icon": icon,
            });
            
            // Add custom properties if provided
            if let Some(props_json) = properties {
                let custom_props: serde_json::Value = serde_json::from_str(props_json)
                    .into_diagnostic()
                    .wrap_err("Failed to parse custom properties JSON")?;
                    
                if let Some(obj) = message.as_object_mut() {
                    obj.insert("properties".to_string(), custom_props);
                }
            }
            
            // Send to LavinMQ
            client.publish_json_message("person.create", &message).await
                .into_diagnostic()
                .wrap_err("Failed to publish Person creation message to LavinMQ")?;
                
            println!("Person creation request for '{}' sent to message queue", formatted_subject);
        },
        
        PersonCommands::Update {
            id,
            name,
            summary,
            icon,
            properties
        } => {
            // Create a message describing the Person update
            let mut message = serde_json::json!({
                "action": "update_person",
                "id": id
            });
            
            // Add optional fields if provided
            if let Some(obj) = message.as_object_mut() {
                if let Some(new_name) = name {
                    obj.insert("name".to_string(), serde_json::Value::String(new_name.clone()));
                }
                
                if let Some(new_summary) = summary {
                    obj.insert("summary".to_string(), serde_json::Value::String(new_summary.clone()));
                }
                
                if let Some(new_icon) = icon {
                    obj.insert("icon".to_string(), serde_json::Value::String(new_icon.clone()));
                }
                
                // Add custom properties if provided
                if let Some(props_json) = properties {
                    let custom_props: serde_json::Value = serde_json::from_str(props_json)
                        .into_diagnostic()
                        .wrap_err("Failed to parse custom properties JSON")?;
                        
                    obj.insert("properties".to_string(), custom_props);
                }
            }
            
            // Send to LavinMQ
            client.publish_json_message("person.update", &message).await
                .into_diagnostic()
                .wrap_err("Failed to publish Person update message to LavinMQ")?;
                
            println!("Person update request for ID '{}' sent to message queue", id);
        },
        
        PersonCommands::Delete { id, force } => {
            // Create a message requesting to delete a Person
            let message = serde_json::json!({
                "action": "delete_person",
                "id": id,
                "force": force
            });
            
            // Send to LavinMQ
            client.publish_json_message("person.delete", &message).await
                .into_diagnostic()
                .wrap_err("Failed to publish Person deletion message to LavinMQ")?;
                
            println!("Person deletion request for ID '{}' sent to message queue", id);
            if *force {
                println!("Forced deletion requested");
            }
        }
    }
    
    Ok(())
}

/// Handle Note object commands via messaging
async fn handle_note_command_messaging(client: &LavinMQClient, command: &NoteCommands) -> Result<()> {
    match command {
        NoteCommands::Create {
            author,
            content,
            name,
            summary,
            mentions,
            tags,
            properties
        } => {
            // Create a message describing the Note creation
            let mut message = serde_json::json!({
                "action": "create_note",
                "author": author,
                "content": content
            });
            
            // Add optional fields if provided
            if let Some(obj) = message.as_object_mut() {
                if let Some(title) = name {
                    obj.insert("name".to_string(), serde_json::Value::String(title.clone()));
                }
                
                if let Some(note_summary) = summary {
                    obj.insert("summary".to_string(), serde_json::Value::String(note_summary.clone()));
                }
                
                if let Some(mentions_str) = mentions {
                    obj.insert("mentions".to_string(), serde_json::Value::String(mentions_str.clone()));
                }
                
                if let Some(tags_str) = tags {
                    obj.insert("tags".to_string(), serde_json::Value::String(tags_str.clone()));
                }
                
                // Add custom properties if provided
                if let Some(props_json) = properties {
                    let custom_props: serde_json::Value = serde_json::from_str(props_json)
                        .into_diagnostic()
                        .wrap_err("Failed to parse custom properties JSON")?;
                        
                    obj.insert("properties".to_string(), custom_props);
                }
            }
            
            // Send to LavinMQ
            client.publish_json_message("note.create", &message).await
                .into_diagnostic()
                .wrap_err("Failed to publish Note creation message to LavinMQ")?;
                
            println!("Note creation request by '{}' sent to message queue", author);
        },
        
        NoteCommands::Update {
            id,
            content,
            name,
            summary,
            tags,
            properties
        } => {
            // Create a message describing the Note update
            let mut message = serde_json::json!({
                "action": "update_note",
                "id": id
            });
            
            // Add optional fields if provided
            if let Some(obj) = message.as_object_mut() {
                if let Some(new_content) = content {
                    obj.insert("content".to_string(), serde_json::Value::String(new_content.clone()));
                }
                
                if let Some(new_name) = name {
                    obj.insert("name".to_string(), serde_json::Value::String(new_name.clone()));
                }
                
                if let Some(new_summary) = summary {
                    obj.insert("summary".to_string(), serde_json::Value::String(new_summary.clone()));
                }
                
                if let Some(tags_str) = tags {
                    obj.insert("tags".to_string(), serde_json::Value::String(tags_str.clone()));
                }
                
                // Add custom properties if provided
                if let Some(props_json) = properties {
                    let custom_props: serde_json::Value = serde_json::from_str(props_json)
                        .into_diagnostic()
                        .wrap_err("Failed to parse custom properties JSON")?;
                        
                    obj.insert("properties".to_string(), custom_props);
                }
            }
            
            // Send to LavinMQ
            client.publish_json_message("note.update", &message).await
                .into_diagnostic()
                .wrap_err("Failed to publish Note update message to LavinMQ")?;
                
            println!("Note update request for ID '{}' sent to message queue", id);
        },
        
        NoteCommands::Delete { id, force } => {
            // Create a message requesting to delete a Note
            let message = serde_json::json!({
                "action": "delete_note",
                "id": id,
                "force": force
            });
            
            // Send to LavinMQ
            client.publish_json_message("note.delete", &message).await
                .into_diagnostic()
                .wrap_err("Failed to publish Note deletion message to LavinMQ")?;
                
            println!("Note deletion request for ID '{}' sent to message queue", id);
            if *force {
                println!("Forced deletion requested");
            }
        }
    }
    
    Ok(())
}

/// Handle Activity commands via messaging
async fn handle_activity_command_messaging(client: &LavinMQClient, command: &ActivityCommands) -> Result<()> {
    match command {
        ActivityCommands::Follow { actor, object } => {
            // Create a message describing the Follow activity
            let message = serde_json::json!({
                "action": "create_activity",
                "type": "Follow",
                "actor": actor,
                "object": object
            });
            
            // Send to LavinMQ
            client.publish_json_message("activity.follow", &message).await
                .into_diagnostic()
                .wrap_err("Failed to publish Follow activity message to LavinMQ")?;
                
            println!("'Follow' activity request from '{}' for object '{}' sent to message queue", actor, object);
        },
        
        ActivityCommands::Like { actor, object } => {
            // Create a message describing the Like activity
            let message = serde_json::json!({
                "action": "create_activity",
                "type": "Like",
                "actor": actor,
                "object": object
            });
            
            // Send to LavinMQ
            client.publish_json_message("activity.like", &message).await
                .into_diagnostic()
                .wrap_err("Failed to publish Like activity message to LavinMQ")?;
                
            println!("'Like' activity request from '{}' for object '{}' sent to message queue", actor, object);
        },
        
        ActivityCommands::Announce { actor, object, to, cc } => {
            // Create a message describing the Announce activity
            let mut message = serde_json::json!({
                "action": "create_activity",
                "type": "Announce",
                "actor": actor,
                "object": object
            });
            
            // Add optional fields if provided
            if let Some(obj) = message.as_object_mut() {
                if let Some(to_str) = to {
                    obj.insert("to".to_string(), serde_json::Value::String(to_str.clone()));
                }
                
                if let Some(cc_str) = cc {
                    obj.insert("cc".to_string(), serde_json::Value::String(cc_str.clone()));
                }
            }
            
            // Send to LavinMQ
            client.publish_json_message("activity.announce", &message).await
                .into_diagnostic()
                .wrap_err("Failed to publish Announce activity message to LavinMQ")?;
                
            println!("'Announce' activity request from '{}' for object '{}' sent to message queue", actor, object);
        },
        
        ActivityCommands::Delete { id, force } => {
            // Create a message requesting to delete an Activity
            let message = serde_json::json!({
                "action": "delete_activity",
                "id": id,
                "force": force
            });
            
            // Send to LavinMQ
            client.publish_json_message("activity.delete", &message).await
                .into_diagnostic()
                .wrap_err("Failed to publish Activity deletion message to LavinMQ")?;
                
            println!("Activity deletion request for ID '{}' sent to message queue", id);
            if *force {
                println!("Forced deletion requested");
            }
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

