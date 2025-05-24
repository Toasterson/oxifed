//! Messaging functionality for LavinMQ communication
//!
//! This module handles the communication with LavinMQ for the oxiadm tool

use lapin::protocol::basic::AMQPProperties;
use lapin::{Connection, ConnectionProperties, options::{BasicPublishOptions, BasicConsumeOptions, QueueDeclareOptions}};
use miette::{IntoDiagnostic, Result};
use oxifed::messaging::{EXCHANGE_INTERNAL_PUBLISH, EXCHANGE_RPC_REQUEST};
use oxifed::messaging::{Message, DomainRpcRequest, DomainRpcResponse, MessageEnum};
use serde::Serialize;
use thiserror::Error;
use tokio::time::{timeout, Duration};
use futures::StreamExt;
use uuid::Uuid;
use std::sync::Arc;

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
    connection: Arc<Connection>,
}

/// RPC client for domain queries
pub struct RpcClient {
    connection: Arc<Connection>,
    reply_queue: String,
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

        // Initialize the exchange
        let channel = connection
            .create_channel()
            .await
            .into_diagnostic()
            .map_err(|e| miette::miette!("Failed to create channel: {}", e))?;

        // Declare the exchange if it doesn't exist
        channel
            .exchange_declare(
                EXCHANGE_INTERNAL_PUBLISH,
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

        Ok(Self { connection: Arc::new(connection) })
    }

    /// Publish a message that implements the Message trait
    pub async fn publish_message<T: Message + Serialize>(
        &self,
        message: &T,
    ) -> Result<(), MessagingError> {
        let channel = self.connection.create_channel().await?;

        // Serialize the message to JSON
        let payload = serde_json::to_vec(&message.to_message())?;

        // Publish the message to the exchange
        channel
            .basic_publish(
                EXCHANGE_INTERNAL_PUBLISH, // exchange
                "",
                BasicPublishOptions::default(),
                &payload,
                AMQPProperties::default(),
            )
            .await?;

        Ok(())
    }

    /// Create an RPC client for domain queries
    pub async fn create_rpc_client(&self) -> Result<RpcClient, MessagingError> {
        RpcClient::new(Arc::clone(&self.connection)).await
    }
}

impl RpcClient {
    /// Create a new RPC client
    async fn new(connection: Arc<Connection>) -> Result<Self, MessagingError> {
        let channel = connection.create_channel().await?;
        
        // Create a temporary exclusive queue for receiving replies
        let reply_queue = channel.queue_declare(
            "",
            QueueDeclareOptions {
                exclusive: true,
                auto_delete: true,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
        ).await?.name().to_string();

        Ok(Self {
            connection,
            reply_queue,
        })
    }

    /// Send an RPC request and wait for response
    /// 
    /// Note: RPC requests are wrapped in MessageEnum before sending to ensure
    /// compatibility with the server-side parsing. The server expects all messages
    /// to be wrapped in MessageEnum, so we use `request.to_message()` here.
    pub async fn send_rpc_request(&self, request: DomainRpcRequest) -> Result<DomainRpcResponse, MessagingError> {
        let channel = self.connection.create_channel().await?;
        
        // Setup consumer for the reply queue
        let mut consumer = channel.basic_consume(
            &self.reply_queue,
            "",
            BasicConsumeOptions::default(),
            lapin::types::FieldTable::default(),
        ).await?;

        // Serialize the request (wrapped in MessageEnum for server compatibility)
        let request_data = serde_json::to_vec(&request.to_message())?;
        let correlation_id = request.request_id.clone();

        // Publish the request
        let properties = AMQPProperties::default()
            .with_reply_to(self.reply_queue.clone().into())
            .with_correlation_id(correlation_id.clone().into());

        channel.basic_publish(
            EXCHANGE_RPC_REQUEST,
            "domain", // routing key for domain requests
            BasicPublishOptions::default(),
            &request_data,
            properties,
        ).await?;

        // Wait for response with timeout
        let response_timeout = Duration::from_secs(30);
        
        match timeout(response_timeout, async {
            while let Some(delivery) = consumer.next().await {
                match delivery {
                    Ok(delivery) => {
                        if let Some(corr_id) = delivery.properties.correlation_id() {
                            if corr_id.as_str() == correlation_id {
                                // Found our response
                                if let Err(e) = delivery.ack(lapin::options::BasicAckOptions::default()).await {
                                    tracing::warn!("Failed to ack RPC response: {}", e);
                                }
                                
                                // Parse response (also wrapped in MessageEnum)
                                let message: MessageEnum = serde_json::from_slice(&delivery.data)?;
                                if let MessageEnum::DomainRpcResponse(response) = message {
                                    return Ok(response);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        return Err(MessagingError::ConnectionError(e));
                    }
                }
            }
            Err(MessagingError::ConfirmationError)
        }).await {
            Ok(result) => result,
            Err(_) => Err(MessagingError::ConfirmationError), // Timeout
        }
    }

    /// List all domains
    pub async fn list_domains(&self) -> Result<Vec<oxifed::messaging::DomainInfo>, MessagingError> {
        let request_id = Uuid::new_v4().to_string();
        let request = DomainRpcRequest::list_domains(request_id);
        
        let response = self.send_rpc_request(request).await?;
        
        match response.result {
            oxifed::messaging::DomainRpcResult::DomainList { domains } => Ok(domains),
            oxifed::messaging::DomainRpcResult::Error { message: _ } => {
                Err(MessagingError::ConfirmationError) // Convert to appropriate error
            }
            _ => Err(MessagingError::ConfirmationError),
        }
    }

    /// Get details for a specific domain
    pub async fn get_domain(&self, domain: &str) -> Result<Option<oxifed::messaging::DomainInfo>, MessagingError> {
        let request_id = Uuid::new_v4().to_string();
        let request = DomainRpcRequest::get_domain(request_id, domain.to_string());
        
        let response = self.send_rpc_request(request).await?;
        
        match response.result {
            oxifed::messaging::DomainRpcResult::DomainDetails { domain } => Ok(domain),
            oxifed::messaging::DomainRpcResult::Error { message: _ } => {
                Err(MessagingError::ConfirmationError) // Convert to appropriate error
            }
            _ => Err(MessagingError::ConfirmationError),
        }
    }
}
