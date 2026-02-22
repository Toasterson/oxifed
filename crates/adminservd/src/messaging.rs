use deadpool_lapin::Pool;
use futures::StreamExt;
use lapin::options::*;
use lapin::protocol::basic::AMQPProperties;
use lapin::types::FieldTable;
use oxifed::messaging::*;
use serde::Serialize;
use thiserror::Error;
use tokio::time::{Duration, timeout};
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Error, Debug)]
pub enum MessagingError {
    #[error("AMQP error: {0}")]
    Amqp(#[from] lapin::Error),

    #[error("Pool error: {0}")]
    Pool(#[from] deadpool_lapin::PoolError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("RPC timeout")]
    Timeout,

    #[error("RPC error: {0}")]
    RpcError(String),
}

impl From<MessagingError> for ApiError {
    fn from(err: MessagingError) -> Self {
        ApiError::Internal(err.to_string())
    }
}

/// Publish a message to the internal exchange
pub async fn publish_message<T: Message + Serialize>(
    pool: &Pool,
    message: &T,
) -> Result<(), MessagingError> {
    let conn = pool.get().await?;
    let channel = conn.create_channel().await?;

    let payload = serde_json::to_vec(&message.to_message())?;

    channel
        .basic_publish(
            EXCHANGE_INTERNAL_PUBLISH,
            "",
            BasicPublishOptions::default(),
            &payload,
            AMQPProperties::default(),
        )
        .await?;

    Ok(())
}

/// Initialize AMQP exchanges needed by adminservd
pub async fn init_exchanges(pool: &Pool) -> Result<(), MessagingError> {
    let conn = pool.get().await?;
    let channel = conn.create_channel().await?;

    // Declare the internal publish exchange
    channel
        .exchange_declare(
            EXCHANGE_INTERNAL_PUBLISH,
            lapin::ExchangeKind::Fanout,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Declare the RPC request exchange
    channel
        .exchange_declare(
            EXCHANGE_RPC_REQUEST,
            lapin::ExchangeKind::Direct,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    Ok(())
}

/// Send a domain RPC request and wait for a response
async fn send_domain_rpc(
    pool: &Pool,
    request: DomainRpcRequest,
) -> Result<DomainRpcResponse, MessagingError> {
    let conn = pool.get().await?;
    let channel = conn.create_channel().await?;

    // Create a temporary exclusive reply queue
    let reply_queue = channel
        .queue_declare(
            "",
            QueueDeclareOptions {
                exclusive: true,
                auto_delete: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?
        .name()
        .to_string();

    // Setup consumer for the reply queue
    let mut consumer = channel
        .basic_consume(
            &reply_queue,
            "",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let request_data = serde_json::to_vec(&request.to_message())?;
    let correlation_id = request.request_id.clone();

    let properties = AMQPProperties::default()
        .with_reply_to(reply_queue.into())
        .with_correlation_id(correlation_id.clone().into());

    channel
        .basic_publish(
            EXCHANGE_RPC_REQUEST,
            "domain",
            BasicPublishOptions::default(),
            &request_data,
            properties,
        )
        .await?;

    let response_timeout = Duration::from_secs(30);

    match timeout(response_timeout, async {
        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    if let Some(corr_id) = delivery.properties.correlation_id()
                        && corr_id.as_str() == correlation_id
                    {
                        if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                            tracing::warn!("Failed to ack RPC response: {}", e);
                        }

                        let message: MessageEnum = serde_json::from_slice(&delivery.data)?;
                        if let MessageEnum::DomainRpcResponse(response) = message {
                            return Ok(response);
                        }
                    }
                }
                Err(e) => {
                    return Err(MessagingError::Amqp(e));
                }
            }
        }
        Err(MessagingError::Timeout)
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err(MessagingError::Timeout),
    }
}

/// Send a user RPC request and wait for a response
async fn send_user_rpc(
    pool: &Pool,
    request: UserRpcRequest,
) -> Result<UserRpcResponse, MessagingError> {
    let conn = pool.get().await?;
    let channel = conn.create_channel().await?;

    let reply_queue = channel
        .queue_declare(
            "",
            QueueDeclareOptions {
                exclusive: true,
                auto_delete: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?
        .name()
        .to_string();

    let mut consumer = channel
        .basic_consume(
            &reply_queue,
            "",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let request_data = serde_json::to_vec(&request.to_message())?;
    let correlation_id = request.request_id.clone();

    let properties = AMQPProperties::default()
        .with_reply_to(reply_queue.into())
        .with_correlation_id(correlation_id.clone().into());

    channel
        .basic_publish(
            EXCHANGE_RPC_REQUEST,
            "user",
            BasicPublishOptions::default(),
            &request_data,
            properties,
        )
        .await?;

    let response_timeout = Duration::from_secs(30);

    match timeout(response_timeout, async {
        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    if let Some(corr_id) = delivery.properties.correlation_id()
                        && corr_id.as_str() == correlation_id
                    {
                        if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                            tracing::warn!("Failed to ack user RPC response: {}", e);
                        }

                        let message: MessageEnum = serde_json::from_slice(&delivery.data)?;
                        if let MessageEnum::UserRpcResponse(response) = message {
                            return Ok(response);
                        }
                    }
                }
                Err(e) => {
                    return Err(MessagingError::Amqp(e));
                }
            }
        }
        Err(MessagingError::Timeout)
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err(MessagingError::Timeout),
    }
}

/// List all domains via RPC
pub async fn list_domains(pool: &Pool) -> Result<Vec<DomainInfo>, MessagingError> {
    let request_id = Uuid::new_v4().to_string();
    let request = DomainRpcRequest::list_domains(request_id);
    let response = send_domain_rpc(pool, request).await?;

    match response.result {
        DomainRpcResult::DomainList { domains } => Ok(domains),
        DomainRpcResult::Error { message } => Err(MessagingError::RpcError(message)),
        _ => Err(MessagingError::RpcError("Unexpected response type".into())),
    }
}

/// Get details for a specific domain via RPC
pub async fn get_domain(pool: &Pool, domain: &str) -> Result<Option<DomainInfo>, MessagingError> {
    let request_id = Uuid::new_v4().to_string();
    let request = DomainRpcRequest::get_domain(request_id, domain.to_string());
    let response = send_domain_rpc(pool, request).await?;

    match response.result {
        DomainRpcResult::DomainDetails { domain } => Ok(*domain),
        DomainRpcResult::Error { message } => Err(MessagingError::RpcError(message)),
        _ => Err(MessagingError::RpcError("Unexpected response type".into())),
    }
}

/// List all users via RPC
pub async fn list_users(pool: &Pool) -> Result<Vec<UserInfo>, MessagingError> {
    let request_id = Uuid::new_v4().to_string();
    let request = UserRpcRequest::list_users(request_id);
    let response = send_user_rpc(pool, request).await?;

    match response.result {
        UserRpcResult::UserList { users } => Ok(users),
        UserRpcResult::Error { message } => Err(MessagingError::RpcError(message)),
        _ => Err(MessagingError::RpcError("Unexpected response type".into())),
    }
}

/// Get details for a specific user via RPC
pub async fn get_user(pool: &Pool, username: &str) -> Result<Option<UserInfo>, MessagingError> {
    let request_id = Uuid::new_v4().to_string();
    let request = UserRpcRequest::get_user(request_id, username.to_string());
    let response = send_user_rpc(pool, request).await?;

    match response.result {
        UserRpcResult::UserDetails { user } => Ok(*user),
        UserRpcResult::Error { message } => Err(MessagingError::RpcError(message)),
        _ => Err(MessagingError::RpcError("Unexpected response type".into())),
    }
}

/// Send a follow RPC request and wait for a response
async fn send_follow_rpc(
    pool: &Pool,
    request: FollowRpcRequest,
) -> Result<FollowRpcResponse, MessagingError> {
    let conn = pool.get().await?;
    let channel = conn.create_channel().await?;

    let reply_queue = channel
        .queue_declare(
            "",
            QueueDeclareOptions {
                exclusive: true,
                auto_delete: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?
        .name()
        .to_string();

    let mut consumer = channel
        .basic_consume(
            &reply_queue,
            "",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let request_data = serde_json::to_vec(&request.to_message())?;
    let correlation_id = request.request_id.clone();

    let properties = AMQPProperties::default()
        .with_reply_to(reply_queue.into())
        .with_correlation_id(correlation_id.clone().into());

    channel
        .basic_publish(
            EXCHANGE_RPC_REQUEST,
            "follow",
            BasicPublishOptions::default(),
            &request_data,
            properties,
        )
        .await?;

    let response_timeout = Duration::from_secs(30);

    match timeout(response_timeout, async {
        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    if let Some(corr_id) = delivery.properties.correlation_id()
                        && corr_id.as_str() == correlation_id
                    {
                        if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                            tracing::warn!("Failed to ack follow RPC response: {}", e);
                        }

                        let message: MessageEnum = serde_json::from_slice(&delivery.data)?;
                        if let MessageEnum::FollowRpcResponse(response) = message {
                            return Ok(response);
                        }
                    }
                }
                Err(e) => {
                    return Err(MessagingError::Amqp(e));
                }
            }
        }
        Err(MessagingError::Timeout)
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err(MessagingError::Timeout),
    }
}

/// List follows for an actor (who they follow) via RPC
pub async fn list_following(pool: &Pool, actor: &str) -> Result<Vec<FollowInfo>, MessagingError> {
    let request_id = Uuid::new_v4().to_string();
    let request = FollowRpcRequest::list_following(request_id, actor.to_string());
    let response = send_follow_rpc(pool, request).await?;

    match response.result {
        FollowRpcResult::FollowList { follows } => Ok(follows),
        FollowRpcResult::Error { message } => Err(MessagingError::RpcError(message)),
    }
}

/// List followers of an actor via RPC
pub async fn list_followers(pool: &Pool, actor: &str) -> Result<Vec<FollowInfo>, MessagingError> {
    let request_id = Uuid::new_v4().to_string();
    let request = FollowRpcRequest::list_followers(request_id, actor.to_string());
    let response = send_follow_rpc(pool, request).await?;

    match response.result {
        FollowRpcResult::FollowList { follows } => Ok(follows),
        FollowRpcResult::Error { message } => Err(MessagingError::RpcError(message)),
    }
}
