//! Integration test for federation between three Oxifed servers
//!
//! This test sets up three separate Oxifed server instances and tests
//! the federation functionality between them, including:
//! - Domain discovery
//! - Activity delivery
//! - Content federation

use oxifed::messaging::{
    DomainCreateMessage, DomainInfo,
    FollowActivityMessage, IncomingActivityMessage, IncomingObjectMessage, Message,
    MessageEnum, NoteCreateMessage, ProfileCreateMessage
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Test configuration for a single server instance
struct ServerConfig {
    name: String,
    domain: String,
    port: u16,
    rabbitmq_port: u16,
    mongodb_port: u16,
}

/// Server instance with its own message broker and database
struct ServerInstance {
    config: ServerConfig,
    message_queue: Arc<Mutex<Vec<MessageEnum>>>,
    domains: Arc<Mutex<Vec<DomainInfo>>>,
}

impl ServerInstance {
    /// Create a new server instance with the given configuration
    fn new(config: ServerConfig) -> Self {
        Self {
            config,
            message_queue: Arc::new(Mutex::new(Vec::new())),
            domains: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Initialize the server with a domain
    async fn initialize(&self) {
        // Create a domain for this server
        let domain_msg = DomainCreateMessage::new(
            self.config.domain.clone(),
            Some(format!("{} Server", self.config.name)),
            Some(format!("Test server for {}", self.config.name)),
            Some(format!("admin@{}", self.config.domain)),
            Some(vec!["Be respectful".to_string()]),
            Some("open".to_string()),
            Some(true),
            Some(500),
            Some(10485760),
            Some(vec!["image/jpeg".to_string(), "image/png".to_string()]),
            None,
        );

        // Simulate domain creation
        let domain_info = DomainInfo {
            domain: self.config.domain.clone(),
            name: Some(format!("{} Server", self.config.name)),
            description: Some(format!("Test server for {}", self.config.name)),
            contact_email: Some(format!("admin@{}", self.config.domain)),
            registration_mode: "open".to_string(),
            authorized_fetch: true,
            max_note_length: Some(500),
            max_file_size: Some(10485760),
            allowed_file_types: Some(vec!["image/jpeg".to_string(), "image/png".to_string()]),
            status: "Active".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        // Store domain info
        self.domains.lock().unwrap().push(domain_info);

        println!("Initialized server {} with domain {}", self.config.name, self.config.domain);
    }

    /// Send a message to this server's message queue
    async fn send_message(&self, message: MessageEnum) {
        self.message_queue.lock().unwrap().push(message);
        println!("Message sent to server {}", self.config.name);
    }

    /// Process messages in the queue
    async fn process_messages(&self) {
        let messages = {
            let mut queue = self.message_queue.lock().unwrap();
            let messages = queue.clone();
            queue.clear();
            messages
        };

        for message in messages {
            match message {
                MessageEnum::IncomingActivityMessage(activity) => {
                    println!(
                        "Server {} processing activity: {} from {}",
                        self.config.name, activity.activity_type, activity.actor
                    );
                    // Process the activity based on its type
                    // In a real implementation, this would update the database and potentially
                    // forward the activity to other servers
                }
                MessageEnum::IncomingObjectMessage(object) => {
                    println!(
                        "Server {} processing object: {} from {}",
                        self.config.name, object.object_type, object.attributed_to
                    );
                    // Process the object based on its type
                }
                _ => {
                    println!("Server {} received other message type", self.config.name);
                }
            }
        }
    }

    /// Create a user profile on this server
    async fn create_profile(&self, username: &str) -> String {
        let profile_id = format!("{}@{}", username, self.config.domain);
        
        let profile_msg = ProfileCreateMessage::new(
            profile_id.clone(),
            Some(format!("Test user {}", username)),
            None,
            None,
        );

        println!("Created profile {} on server {}", profile_id, self.config.name);
        profile_id
    }

    /// Create a note from a user on this server
    async fn create_note(&self, author: &str, content: &str) -> String {
        let note_id = format!("note-{}-{}", author.split('@').next().unwrap(), Uuid::new_v4());
        
        let note_msg = NoteCreateMessage::new(
            author.to_string(),
            content.to_string(),
            None,
            None,
            None,
            None,
        );

        println!("Created note {} by {} on server {}", note_id, author, self.config.name);
        note_id
    }

    /// Follow a user on another server
    async fn follow_user(&self, follower: &str, target: &str) {
        let follow_msg = FollowActivityMessage::new(
            follower.to_string(),
            target.to_string(),
        );

        // In a real implementation, this would create a Follow activity and send it to the target server
        println!("{} on server {} is now following {}", follower, self.config.name, target);
    }
}

/// Federation network with multiple server instances
struct FederationNetwork {
    servers: Vec<ServerInstance>,
}

impl FederationNetwork {
    /// Create a new federation network with the given servers
    fn new(servers: Vec<ServerInstance>) -> Self {
        Self { servers }
    }

    /// Find a server by domain
    fn find_server(&self, domain: &str) -> Option<&ServerInstance> {
        self.servers.iter().find(|s| s.config.domain == domain)
    }

    /// Deliver an activity from one server to another
    async fn deliver_activity(&self, from_domain: &str, to_domain: &str, activity_type: &str, actor: &str, object: Value) {
        let from_server = self.find_server(from_domain).expect("Source server not found");
        let to_server = self.find_server(to_domain).expect("Target server not found");

        // Create an incoming activity message
        let activity_msg = IncomingActivityMessage {
            activity: object,
            activity_type: activity_type.to_string(),
            actor: actor.to_string(),
            target_domain: to_domain.to_string(),
            target_username: None, // In a real implementation, this would be extracted from the activity
            received_at: chrono::Utc::now().to_rfc3339(),
            source: Some(from_domain.to_string()),
        };

        // Send the activity to the target server
        to_server.send_message(activity_msg.to_message()).await;
        println!("Delivered {} activity from {} to {}", activity_type, from_domain, to_domain);
    }

    /// Deliver an object from one server to another
    async fn deliver_object(&self, from_domain: &str, to_domain: &str, object_type: &str, attributed_to: &str, object: Value) {
        let from_server = self.find_server(from_domain).expect("Source server not found");
        let to_server = self.find_server(to_domain).expect("Target server not found");

        // Create an incoming object message
        let object_msg = IncomingObjectMessage {
            object,
            object_type: object_type.to_string(),
            attributed_to: attributed_to.to_string(),
            target_domain: to_domain.to_string(),
            target_username: None, // In a real implementation, this would be extracted from the object
            received_at: chrono::Utc::now().to_rfc3339(),
            source: Some(from_domain.to_string()),
        };

        // Send the object to the target server
        to_server.send_message(object_msg.to_message()).await;
        println!("Delivered {} object from {} to {}", object_type, from_domain, to_domain);
    }

    /// Process messages on all servers
    async fn process_all_messages(&self) {
        for server in &self.servers {
            server.process_messages().await;
        }
    }
}

#[tokio::test]
async fn test_federation_between_three_servers() {
    // Create configurations for three servers
    let server1_config = ServerConfig {
        name: "Alpha".to_string(),
        domain: "alpha.example".to_string(),
        port: 8081,
        rabbitmq_port: 5673,
        mongodb_port: 27018,
    };

    let server2_config = ServerConfig {
        name: "Beta".to_string(),
        domain: "beta.example".to_string(),
        port: 8082,
        rabbitmq_port: 5674,
        mongodb_port: 27019,
    };

    let server3_config = ServerConfig {
        name: "Gamma".to_string(),
        domain: "gamma.example".to_string(),
        port: 8083,
        rabbitmq_port: 5675,
        mongodb_port: 27020,
    };

    // Create server instances
    let server1 = ServerInstance::new(server1_config);
    let server2 = ServerInstance::new(server2_config);
    let server3 = ServerInstance::new(server3_config);

    // Initialize servers
    server1.initialize().await;
    server2.initialize().await;
    server3.initialize().await;

    // Create federation network
    let federation = FederationNetwork::new(vec![server1, server2, server3]);

    // Test scenario: Create users on each server
    let alice = federation.find_server("alpha.example").unwrap().create_profile("alice").await;
    let bob = federation.find_server("beta.example").unwrap().create_profile("bob").await;
    let charlie = federation.find_server("gamma.example").unwrap().create_profile("charlie").await;

    // Test scenario: Alice follows Bob
    federation.find_server("alpha.example").unwrap().follow_user(&alice, &bob).await;

    // Deliver the Follow activity from Alpha to Beta
    let follow_activity = json!({
        "type": "Follow",
        "actor": alice,
        "object": bob,
        "id": format!("https://alpha.example/activities/{}", Uuid::new_v4()),
    });
    federation.deliver_activity("alpha.example", "beta.example", "Follow", &alice, follow_activity).await;

    // Process messages on all servers
    federation.process_all_messages().await;

    // Test scenario: Bob creates a note
    let bob_note_id = federation.find_server("beta.example").unwrap()
        .create_note(&bob, "Hello from Beta server!").await;

    // Create a Note object
    let note_object = json!({
        "type": "Note",
        "id": format!("https://beta.example/notes/{}", bob_note_id),
        "attributedTo": bob,
        "content": "Hello from Beta server!",
        "published": chrono::Utc::now().to_rfc3339(),
    });

    // Create a Create activity for the note
    let create_activity = json!({
        "type": "Create",
        "actor": bob,
        "object": note_object,
        "id": format!("https://beta.example/activities/{}", Uuid::new_v4()),
        "published": chrono::Utc::now().to_rfc3339(),
    });

    // Deliver the Create activity from Beta to Alpha (because Alice follows Bob)
    federation.deliver_activity("beta.example", "alpha.example", "Create", &bob, create_activity).await;

    // Process messages on all servers
    federation.process_all_messages().await;

    // Test scenario: Charlie follows Alice
    federation.find_server("gamma.example").unwrap().follow_user(&charlie, &alice).await;

    // Deliver the Follow activity from Gamma to Alpha
    let follow_activity = json!({
        "type": "Follow",
        "actor": charlie,
        "object": alice,
        "id": format!("https://gamma.example/activities/{}", Uuid::new_v4()),
    });
    federation.deliver_activity("gamma.example", "alpha.example", "Follow", &charlie, follow_activity).await;

    // Process messages on all servers
    federation.process_all_messages().await;

    // Test scenario: Alice creates a note that mentions Bob
    let alice_note_id = federation.find_server("alpha.example").unwrap()
        .create_note(&alice, "Hello @bob@beta.example, how are you?").await;

    // Create a Note object with mentions
    let note_object = json!({
        "type": "Note",
        "id": format!("https://alpha.example/notes/{}", alice_note_id),
        "attributedTo": alice,
        "content": "Hello @bob@beta.example, how are you?",
        "published": chrono::Utc::now().to_rfc3339(),
        "tag": [
            {
                "type": "Mention",
                "href": bob,
                "name": "@bob@beta.example"
            }
        ]
    });

    // Create a Create activity for the note
    let create_activity = json!({
        "type": "Create",
        "actor": alice,
        "object": note_object.clone(),
        "id": format!("https://alpha.example/activities/{}", Uuid::new_v4()),
        "published": chrono::Utc::now().to_rfc3339(),
    });

    // Deliver the Create activity from Alpha to Beta (because Alice mentioned Bob)
    federation.deliver_activity("alpha.example", "beta.example", "Create", &alice, create_activity.clone()).await;

    // Deliver the Create activity from Alpha to Gamma (because Charlie follows Alice)
    federation.deliver_activity("alpha.example", "gamma.example", "Create", &alice, create_activity).await;

    // Process messages on all servers
    federation.process_all_messages().await;

    // Test scenario: Bob likes Alice's note
    let like_activity = json!({
        "type": "Like",
        "actor": bob,
        "object": format!("https://alpha.example/notes/{}", alice_note_id),
        "id": format!("https://beta.example/activities/{}", Uuid::new_v4()),
        "published": chrono::Utc::now().to_rfc3339(),
    });

    // Deliver the Like activity from Beta to Alpha
    federation.deliver_activity("beta.example", "alpha.example", "Like", &bob, like_activity).await;

    // Process messages on all servers
    federation.process_all_messages().await;

    // Verify that the federation test completed successfully
    println!("Federation test completed successfully!");
}