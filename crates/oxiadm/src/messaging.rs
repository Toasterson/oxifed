//! Messaging functionality for LavinMQ communication
//!
//! This module handles the communication with LavinMQ for the oxiadm tool

use lapin::protocol::basic::AMQPProperties;
use lapin::{Connection, ConnectionProperties, options::BasicPublishOptions};
use miette::{IntoDiagnostic, Result};
use oxifed::messaging::Message;
use serde::Serialize;
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

/// LavinMQ client for publishing ActivityPub operations to message broker
pub struct LavinMQClient {
    connection: Connection,
}

impl LavinMQClient {
    /// Create a new LavinMQ client and initialize exchanges
    pub async fn new(url: &str) -> Result<Self> {
        let connection = Connection::connect(
            url,
            ConnectionProperties::default().with_connection_name("oxiadm".into()),
        )
        .await
        .into_diagnostic()
        .map_err(|e| miette::miette!("Failed to connect to LavinMQ: {}", e))?;

        // Initialize the oxifed.activities exchange
        let channel = connection
            .create_channel()
            .await
            .into_diagnostic()
            .map_err(|e| miette::miette!("Failed to create channel: {}", e))?;

        // Declare the oxifed.activities exchange if it doesn't exist
        channel
            .exchange_declare(
                "oxifed.activities",
                lapin::ExchangeKind::Fanout,
                lapin::options::ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                lapin::types::FieldTable::default(),
            )
            .await
            .into_diagnostic()
            .map_err(|e| miette::miette!("Failed to declare exchange: {}", e))?;

        Ok(Self { connection })
    }

    /// Publish a message that implements the Message trait
    pub async fn publish_message<T: Message + Serialize>(
        &self,
        message: &T,
    ) -> Result<(), MessagingError> {
        let channel = self.connection.create_channel().await?;

        // Serialize the message to JSON
        let payload = serde_json::to_vec(message)?;

        // Publish the message to the oxifed.publish exchange
        channel
            .basic_publish(
                "oxifed.activities",   // exchange
                message.routing_key(), // routing key
                BasicPublishOptions::default(),
                &payload,
                AMQPProperties::default(),
            )
            .await?;

        Ok(())
    }
}
