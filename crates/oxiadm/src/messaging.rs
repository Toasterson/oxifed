//! Messaging functionality for LavinMQ communication
//!
//! This module handles the communication with LavinMQ for the oxiadm tool

use lapin::{
    options::{BasicPublishOptions, ExchangeDeclareOptions},
    publisher_confirm::Confirmation,
    BasicProperties, Connection, ConnectionProperties, ExchangeKind,
};
use miette::{Context, IntoDiagnostic, Result};
use oxifed::messaging::ProfileCreateMessage;
use thiserror::Error;

/// Messaging-related errors
#[derive(Error, Debug)]
pub enum MessagingError {
    /// LavinMQ connection error
    #[error("LavinMQ connection error: {0}")]
    ConnectionError(#[from] lapin::Error),

    /// JSON serialization error
    #[error("JSON serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Message confirmation error
    #[error("Message was not confirmed by broker")]
    ConfirmationError,
}

impl From<MessagingError> for miette::Error {
    fn from(err: MessagingError) -> Self {
        miette::Error::msg(err.to_string())
    }
}

/// LavinMQ client for profile operations
pub struct LavinMQClient {
    connection: Connection,
}

impl LavinMQClient {
    /// Create a new LavinMQ client
    pub async fn new(connection_string: &str) -> Result<Self> {
        let connection = Connection::connect(
            connection_string,
            ConnectionProperties::default(),
        )
        .await
        .into_diagnostic()
        .wrap_err("Failed to connect to LavinMQ")?;

        Ok(Self { connection })
    }

    /// Publish a profile creation message
    pub async fn publish_create_profile(
        &self,
        message: ProfileCreateMessage,
    ) -> Result<(), MessagingError> {
        // Create a channel
        let channel = self.connection.create_channel().await?;

        // Ensure the exchange exists
        channel
            .exchange_declare(
                "oxifed.create",
                ExchangeKind::Fanout,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                Default::default(),
            )
            .await?;

        // Serialize the message
        let payload = serde_json::to_vec(&message)?;

        // Publish the message
        let confirm = channel
            .basic_publish(
                "oxifed.create", // exchange name
                "",               // routing key (empty for fanout exchanges)
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default()
                    .with_content_type("application/json".into())
                    .with_delivery_mode(2), // Persistent
            )
            .await?
            .await?;

        // Check if the message was confirmed
        match confirm {
            Confirmation::Ack(_) => Ok(()),
            Confirmation::NotRequested => Ok(()),
            _ => Err(MessagingError::ConfirmationError),
        }
    }

    /// Publish a profile edit message
    pub async fn publish_edit_profile(
        &self,
        subject: &str,
        new_subject: Option<&str>,
        aliases: Option<&str>,
        links: Option<&str>,
        clear_aliases: bool,
        clear_links: bool,
    ) -> Result<(), MessagingError> {
        // Create a channel
        let channel = self.connection.create_channel().await?;

        // Ensure the exchange exists
        channel
            .exchange_declare(
                "oxifed.edit",
                ExchangeKind::Fanout,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                Default::default(),
            )
            .await?;

        // Create a message structure for edit
        let edit_message = oxifed::messaging::ProfileEditMessage {
            subject: subject.to_string(),
            new_subject: new_subject.map(ToString::to_string),
            aliases: aliases.map(ToString::to_string),
            links: links.map(ToString::to_string),
            clear_aliases,
            clear_links,
        };

        // Serialize the message
        let payload = serde_json::to_vec(&edit_message)?;

        // Publish the message
        let confirm = channel
            .basic_publish(
                "oxifed.edit",  // exchange name
                "",              // routing key (empty for fanout exchanges)
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default()
                    .with_content_type("application/json".into())
                    .with_delivery_mode(2), // Persistent
            )
            .await?
            .await?;

        // Check if the message was confirmed
        match confirm {
            Confirmation::Ack(_) => Ok(()),
            Confirmation::NotRequested => Ok(()),
            _ => Err(MessagingError::ConfirmationError),
        }
    }
}
