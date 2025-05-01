//! Messaging functionality for LavinMQ communication
//!
//! This module handles the communication with LavinMQ for the oxiadm tool

use lapin::{
    options::BasicPublishOptions

    , Connection, ConnectionProperties,
};
use lapin::protocol::basic::AMQPProperties;
use miette::{IntoDiagnostic, Result};
use oxifed::messaging::Message;
use serde::Serialize;
use serde_json::Value;
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
    pub async fn new(url: &str) -> Result<Self> {
        let connection = Connection::connect(
            url,
            ConnectionProperties::default()
                .with_connection_name("oxiadm".into())
        )
        .await
        .into_diagnostic()
        .map_err(|e| miette::miette!("Failed to connect to LavinMQ: {}", e))?;

        Ok(Self { connection })
    }

    /// Publish a message that implements the Message trait
    pub async fn publish_message<T: Message + Serialize>(&self, message: &T) -> Result<(), MessagingError> {
        let channel = self.connection.create_channel().await?;

        // Serialize the message to JSON
        let payload = serde_json::to_vec(message)?;

        // Publish the message
        channel
            .basic_publish(
                "",                    // exchange
                message.routing_key(), // routing key
                BasicPublishOptions::default(),
                &payload,
                AMQPProperties::default(),
            )
            .await?;

        Ok(())
    }

    /// Publish a JSON message (legacy method)
    pub async fn publish_json_message(&self, routing_key: &str, payload: &Value) -> Result<(), MessagingError> {
        let channel = self.connection.create_channel().await?;

        // Serialize the message to JSON
        let payload = serde_json::to_vec(payload)?;

        // Publish the message
        channel
            .basic_publish(
                "",            // exchange
                routing_key,   // routing key
                BasicPublishOptions::default(),
                &payload,
                AMQPProperties::default(),
            )
            .await?;

        Ok(())
    }
}
