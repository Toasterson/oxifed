mod db;
mod messaging;

use clap::{Parser, Subcommand};
use messaging::LavinMQClient;
use miette::{Context, IntoDiagnostic, Result};
use oxifed::messaging::ProfileCreateMessage;
use oxifed::webfinger::JrdResource;
use std::collections::HashMap;

/// Oxifed Admin CLI tool for managing profiles
#[derive(Parser)]
#[command(name = "oxiadm")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// MongoDB connection string (for backward compatibility)
    #[arg(long, env = "MONGODB_URI", default_value = "mongodb://root:password@localhost:27017")]
    mongo_uri: String,

    /// MongoDB database name (for backward compatibility)
    #[arg(long, env = "MONGODB_DBNAME", default_value = "domainservd")]
    db_name: String,
    
    /// LavinMQ connection string
    #[arg(long, env = "AMQP_URL", default_value = "amqp://guest:guest@localhost:5672")]
    amqp_url: String,
    
    /// Use direct MongoDB instead of messaging
    #[arg(long)]
    direct_db: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new profile
    Create {
        /// Subject of the profile (e.g. user@example.com or full acct:user@example.com)
        subject: String,

        /// Aliases for the profile (comma separated)
        #[arg(long)]
        aliases: Option<String>,

        /// Add links to the profile (format: rel,href[,title][;rel2,href2,...])
        #[arg(long)]
        links: Option<String>,
    },

    /// Edit an existing profile
    Edit {
        /// Original subject of the profile to edit (with or without acct: prefix)
        subject: String,

        /// New subject for the profile (with or without acct: prefix)
        #[arg(long)]
        new_subject: Option<String>,

        /// New aliases for the profile (comma separated)
        #[arg(long)]
        aliases: Option<String>,

        /// Add links to the profile (format: rel,href[,title][;rel2,href2,...])
        #[arg(long)]
        links: Option<String>,

        /// Remove all aliases
        #[arg(long)]
        clear_aliases: bool,

        /// Remove all links
        #[arg(long)]
        clear_links: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let cli = Cli::parse();

    if cli.direct_db {
        // Use legacy direct DB operations
        let mongo_client = db::MongoClient::new(&cli.mongo_uri, &cli.db_name).await?;
        handle_command_db(&mongo_client, &cli.command).await?;
    } else {
        // Use messaging via LavinMQ
        let lavin_client = LavinMQClient::new(&cli.amqp_url).await?;
        handle_command_messaging(&lavin_client, &cli.command).await?;
    }

    Ok(())
}

/// Handle commands using messaging
async fn handle_command_messaging(client: &LavinMQClient, command: &Commands) -> Result<()> {
    match command {
        Commands::Create {
            subject,
            aliases,
            links,
        } => {
            // Format the subject with appropriate prefix
            let formatted_subject = format_subject(subject);
            
            // Create message for LavinMQ
            let message = ProfileCreateMessage {
                name: formatted_subject.clone(),
                subject: formatted_subject.clone(),
                aliases: aliases.clone(),
                links: links.clone(),
            };
            
            // Send to LavinMQ
            client.publish_create_profile(message).await
                .into_diagnostic()
                .wrap_err("Failed to publish message to LavinMQ")?;
                
            println!("Profile creation request with subject '{}' sent to message queue", formatted_subject);
        },
        Commands::Edit {
            subject,
            new_subject,
            aliases,
            links,
            clear_aliases,
            clear_links,
        } => {
            // Format the subject with appropriate prefix
            let formatted_subject = format_subject(subject);
            
            // Format the new subject if provided
            let formatted_new_subject = new_subject.as_ref().map(|s| format_subject(s));
            
            // Send to LavinMQ
            client.publish_edit_profile(
                &formatted_subject,
                formatted_new_subject.as_deref(),
                aliases.as_deref(),
                links.as_deref(),
                *clear_aliases,
                *clear_links,
            ).await
                .into_diagnostic()
                .wrap_err("Failed to publish edit message to LavinMQ")?;
                
            println!("Profile edit request for subject '{}' sent to message queue", formatted_subject);
            if let Some(new_subj) = &formatted_new_subject {
                println!("Requested subject change to '{}'", new_subj);
            }
        }
    }
    
    Ok(())
}

/// Handle commands using direct DB operations (legacy mode)
async fn handle_command_db(client: &db::MongoClient, command: &Commands) -> Result<()> {
    match command {
        Commands::Create {
            subject,
            aliases,
            links,
        } => {
            create_profile_db(client, subject, aliases, links).await?;
        },
        Commands::Edit {
            subject,
            new_subject,
            aliases,
            links,
            clear_aliases,
            clear_links,
        } => {
            edit_profile_db(
                client, 
                subject, 
                new_subject, 
                aliases, 
                links,
                *clear_aliases,
                *clear_links,
            ).await?;
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

/// Legacy DB operation: create profile
async fn create_profile_db(
    mongo_client: &db::MongoClient,
    subject: &str,
    aliases: &Option<String>,
    links: &Option<String>,
) -> Result<()> {
    // Format the subject with appropriate prefix
    let formatted_subject = format_subject(subject);
    
    // Create new profile
    let mut resource = JrdResource {
        subject: Some(formatted_subject.clone()),
        aliases: None,
        properties: Some(HashMap::from([
            ("name".to_string(), serde_json::to_value(&formatted_subject).into_diagnostic()?),
        ])),
        links: None,
    };

    // Process aliases if provided
    if let Some(aliases_str) = aliases {
        let aliases_vec: Vec<String> = aliases_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        if !aliases_vec.is_empty() {
            resource.aliases = Some(aliases_vec);
        }
    }

    // Process links if provided
    if let Some(links_str) = links {
        let links_vec = parse_links(links_str)?;
        if !links_vec.is_empty() {
            resource.links = Some(links_vec);
        }
    }

    // Save to MongoDB directly
    mongo_client.create_profile(resource).await?;

    println!("Created profile with subject '{}' directly in database", formatted_subject);
    Ok(())
}

/// Legacy DB operation: edit profile
async fn edit_profile_db(
    mongo_client: &db::MongoClient,
    subject: &str,
    new_subject: &Option<String>,
    aliases: &Option<String>,
    links: &Option<String>,
    clear_aliases: bool,
    clear_links: bool,
) -> Result<()> {
    // Get existing profile
    let mut resource = mongo_client.get_profile(subject).await?;

    // Update subject if provided
    if let Some(new_subj) = new_subject {
        let formatted_new_subject = format_subject(new_subj);
        resource.subject = Some(formatted_new_subject);
    }

    // Handle aliases
    if clear_aliases {
        resource.aliases = None;
    } else if let Some(aliases_str) = aliases {
        let aliases_vec: Vec<String> = aliases_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        if !aliases_vec.is_empty() {
            resource.aliases = Some(aliases_vec);
        }
    }

    // Handle links
    if clear_links {
        resource.links = None;
    } else if let Some(links_str) = links {
        let new_links = parse_links(links_str)?;
        
        if !new_links.is_empty() {
            if let Some(existing_links) = &mut resource.links {
                // Add new links to existing ones
                existing_links.extend(new_links);
            } else {
                // Set new links
                resource.links = Some(new_links);
            }
        }
    }

    // Display original subject from the resource
    let original_subject = resource.subject.as_ref()
        .ok_or_else(|| miette::miette!("Existing profile has no subject"))?;

    // Update in MongoDB directly
    mongo_client.update_profile(subject, resource.clone()).await?;

    println!("Updated profile with subject '{}' directly in database", original_subject);
    if let Some(new_subj) = new_subject {
        let formatted_new_subject = format_subject(new_subj);
        println!("Subject changed to '{}'", formatted_new_subject);
    }
    Ok(())
}

fn parse_links(links_str: &str) -> Result<Vec<oxifed::webfinger::Link>> {
    let mut result = Vec::new();

    for link_str in links_str.split(';') {
        let parts: Vec<&str> = link_str.split(',').collect();
        if parts.len() < 2 {
            return Err(miette::miette!("Invalid link format: '{}'", link_str));
        }

        let rel = parts[0].trim().to_string();
        let href = parts[1].trim().to_string();
        
        let title = if parts.len() > 2 { 
            Some(parts[2].trim().to_string()) 
        } else { 
            None 
        };

        let link = oxifed::webfinger::Link {
            rel,
            href: Some(href),
            type_: None,
            titles: title.map(|t| {
                let mut map = HashMap::new();
                map.insert("en".to_string(), t);
                map
            }),
            properties: None,
        };

        result.push(link);
    }

    Ok(result)
}
