//! ActivityPub protocol implementation based on W3C specification.
//! 
//! This crate provides types and deserialization for ActivityPub protocol,
//! which is a decentralized social networking protocol based on ActivityStreams 2.0.
//! 
//! See the [W3C ActivityPub Specification](https://www.w3.org/TR/activitypub/) for details.

use std::collections::HashMap;
use serde::{Deserialize, Serialize, Deserializer};
use serde_json::Value;
use chrono::{DateTime, Utc};
use url::Url;

/// Represents types of objects in ActivityPub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ObjectType {
    // Core types
    Object,
    Link,
    Activity,
    IntransitiveActivity,
    Collection,
    OrderedCollection,
    CollectionPage,
    OrderedCollectionPage,
    
    // Actor types
    Application,
    Group,
    Organization,
    Person,
    Service,
    
    // Object types
    Article,
    Audio,
    Document,
    Event,
    Image,
    Note,
    Page,
    Place,
    Profile,
    Relationship,
    Tombstone,
    Video,
    
    // Other types that may be defined by extensions
    #[serde(other)]
    Other,
}

/// Represents types of activities in ActivityPub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityType {
    // Base activities
    Accept,
    Add,
    Announce,
    Arrive,
    Block,
    Create,
    Delete,
    Dislike,
    Flag,
    Follow,
    Ignore,
    Invite,
    Join,
    Leave,
    Like,
    Listen,
    Move,
    Offer,
    Question,
    Reject,
    Read,
    Remove,
    TentativeReject,
    TentativeAccept,
    Travel,
    Undo,
    Update,
    View,
    
    // Other activities that may be defined by extensions
    #[serde(other)]
    Other,
}

/// Represents an object in ActivityPub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    /// The type of the object
    #[serde(rename = "type")]
    pub object_type: ObjectType,
    
    /// The identifier for this object
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Url>,
    
    /// A simple, human-readable, plain-text name for the object
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    
    /// A natural language summarization of the object
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    
    /// The content of the object
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    
    /// The URL of the object
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<Url>,
    
    /// When the object was published
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<DateTime<Utc>>,
    
    /// When the object was updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<DateTime<Utc>>,
    
    /// The author of the object
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributed_to: Option<ObjectOrLink>,
    
    /// Additional properties not defined in the specification
    #[serde(flatten)]
    pub additional_properties: HashMap<String, Value>,
}

/// Represents a link in ActivityPub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    /// The type of the link (always "Link")
    #[serde(rename = "type")]
    pub link_type: ObjectType,
    
    /// Target resource of the link
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<Url>,
    
    /// A human-readable name for the link
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    
    /// The MIME media type of the link
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    
    /// A hint about the language of the linked resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hreflang: Option<String>,
    
    /// Additional properties not defined in the specification
    #[serde(flatten)]
    pub additional_properties: HashMap<String, Value>,
}

/// Represents either an Object or a Link, or just a URL reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObjectOrLink {
    Object(Box<Object>),
    Link(Link),
    Url(Url),
}

impl ObjectOrLink {
    /// Returns true if this is an Object variant
    pub fn is_object(&self) -> bool {
        matches!(self, ObjectOrLink::Object(_))
    }

    /// Returns true if this is a Link variant
    pub fn is_link(&self) -> bool {
        matches!(self, ObjectOrLink::Link(_))
    }

    /// Returns true if this is a URL variant
    pub fn is_url(&self) -> bool {
        matches!(self, ObjectOrLink::Url(_))
    }

    /// Attempts to get the underlying URL, whether from a URL variant or from
    /// an Object or Link's ID or URL property
    pub fn get_url(&self) -> Option<&Url> {
        match self {
            ObjectOrLink::Object(obj) => obj.id.as_ref().or_else(|| obj.url.as_ref()),
            ObjectOrLink::Link(link) => link.href.as_ref(),
            ObjectOrLink::Url(url) => Some(url),
        }
    }
}

/// Represents an Activity in ActivityPub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    /// The type of the activity
    #[serde(rename = "type")]
    pub activity_type: ActivityType,

    /// The identifier for this activity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Url>,

    /// A simple, human-readable, plain-text name for the activity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// A natural language summarization of the activity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,

    /// The actor or person performing the activity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<ObjectOrLink>,

    /// The primary object of the activity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<ObjectOrLink>,

    /// The target of the activity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<ObjectOrLink>,

    /// When the activity was published
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<DateTime<Utc>>,

    /// When the activity was updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<DateTime<Utc>>,

    /// Additional properties not defined in the specification
    #[serde(flatten)]
    pub additional_properties: HashMap<String, Value>,
}

/// Represents a Collection in ActivityPub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    /// The type of the collection
    #[serde(rename = "type")]
    pub collection_type: ObjectType,

    /// The identifier for this collection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Url>,

    /// A simple, human-readable, plain-text name for the collection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The total number of items in the collection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_items: Option<usize>,

    /// The items in the collection
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ObjectOrLink>,

    /// Additional properties not defined in the specification
    #[serde(flatten)]
    pub additional_properties: HashMap<String, Value>,
}

/// A deserializer helper that can parse different ActivityPub entities.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ActivityPubEntity {
    Activity(Activity),
    Object(Object),
    Link(Link),
    Collection(Collection),
}

impl<'de> Deserialize<'de> for ActivityPubEntity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;

        // Check for the type property
        if let Some(type_value) = value.get("type") {
            let type_str = type_value.as_str().unwrap_or("");

            // Determine which entity to deserialize based on the type
            match type_str {
                "Create" | "Follow" | "Accept" | "Reject" | "Add" | "Remove" | "Like" | "Announce" | "Undo" | "Update" | "Delete" | "Block" | "Offer" | "Invite" => {
                    let activity: Activity = serde_json::from_value(value.clone())
                        .map_err(serde::de::Error::custom)?;
                    Ok(ActivityPubEntity::Activity(activity))
                },
                "Collection" | "OrderedCollection" | "CollectionPage" | "OrderedCollectionPage" => {
                    let collection: Collection = serde_json::from_value(value.clone())
                        .map_err(serde::de::Error::custom)?;
                    Ok(ActivityPubEntity::Collection(collection))
                },
                "Link" => {
                    let link: Link = serde_json::from_value(value.clone())
                        .map_err(serde::de::Error::custom)?;
                    Ok(ActivityPubEntity::Link(link))
                },
                // Default to Object for all other types
                _ => {
                    let object: Object = serde_json::from_value(value.clone())
                        .map_err(serde::de::Error::custom)?;
                    Ok(ActivityPubEntity::Object(object))
                }
            }
        } else {
            // If no type is specified, try to deserialize as an Object
            let object: Object = serde_json::from_value(value.clone())
                .map_err(serde::de::Error::custom)?;
            Ok(ActivityPubEntity::Object(object))
        }
    }
}

/// Helper method to parse JSON into an ActivityPub entity
pub fn parse_activitypub_json(json: &str) -> Result<ActivityPubEntity, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_activity() {
        let json = r#"
        {
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Create",
            "id": "https://example.com/activities/1",
            "actor": "https://example.com/users/alice",
            "object": {
                "type": "Note",
                "id": "https://example.com/notes/1",
                "content": "Hello, world!",
                "published": "2021-09-01T12:00:00Z"
            }
        }
        "#;

        let result = parse_activitypub_json(json).unwrap();

        if let ActivityPubEntity::Activity(activity) = result {
            assert_eq!(activity.activity_type, ActivityType::Create);
            assert_eq!(activity.id, Some(Url::parse("https://example.com/activities/1").unwrap()));

            // Check actor field - should be a URL string in this case
            if let Some(ObjectOrLink::Url(actor_url)) = &activity.actor {
                assert_eq!(actor_url.as_str(), "https://example.com/users/alice");
            } else {
                panic!("Actor should be a URL");
            }

            // Check object field - should be an Object
            if let Some(ObjectOrLink::Object(object)) = &activity.object {
                assert_eq!(object.object_type, ObjectType::Note);
                assert_eq!(object.id, Some(Url::parse("https://example.com/notes/1").unwrap()));
                assert_eq!(object.content, Some("Hello, world!".to_string()));
            } else {
                panic!("Object should be an Object type");
            }
        } else {
            panic!("Should be an Activity");
        }
    }

    #[test]
    fn test_parse_object_with_additional_properties() {
        let json = r#"
        {
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Person",
            "id": "https://example.com/users/bob",
            "name": "Bob",
            "preferredUsername": "bob123",
            "inbox": "https://example.com/users/bob/inbox",
            "outbox": "https://example.com/users/bob/outbox",
            "followers": "https://example.com/users/bob/followers"
        }
        "#;

        let result = parse_activitypub_json(json).unwrap();

        if let ActivityPubEntity::Object(object) = result {
            assert_eq!(object.object_type, ObjectType::Person);
            assert_eq!(object.id, Some(Url::parse("https://example.com/users/bob").unwrap()));
            assert_eq!(object.name, Some("Bob".to_string()));

            // Check additional properties
            assert!(object.additional_properties.contains_key("preferredUsername"));
            assert!(object.additional_properties.contains_key("inbox"));
            assert!(object.additional_properties.contains_key("outbox"));
            assert!(object.additional_properties.contains_key("followers"));

            if let Some(Value::String(username)) = object.additional_properties.get("preferredUsername") {
                assert_eq!(username, "bob123");
            } else {
                panic!("preferredUsername should be a string");
            }
        } else {
            panic!("Should be an Object");
        }
    }

    #[test]
    fn test_parse_collection() {
        let json = r#"
        {
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Collection",
            "id": "https://example.com/collections/public",
            "totalItems": 2,
            "items": [
                {
                    "type": "Note",
                    "id": "https://example.com/notes/1",
                    "content": "First note"
                },
                "https://example.com/notes/2"
            ]
        }
        "#;

        let result = parse_activitypub_json(json).unwrap();

        if let ActivityPubEntity::Collection(collection) = result {
            assert_eq!(collection.collection_type, ObjectType::Collection);
            assert_eq!(collection.id, Some(Url::parse("https://example.com/collections/public").unwrap()));
            assert_eq!(collection.total_items, Some(2));
            assert_eq!(collection.items.len(), 2);

            // Check first item - should be an Object (Note)
            match &collection.items[0] {
                ObjectOrLink::Object(object) => {
                    assert_eq!(object.object_type, ObjectType::Note);
                    assert_eq!(object.id, Some(Url::parse("https://example.com/notes/1").unwrap()));
                    assert_eq!(object.content, Some("First note".to_string()));
                },
                _ => panic!("First item should be an Object"),
            }

            // Check second item - should be a URL
            match &collection.items[1] {
                ObjectOrLink::Url(url) => {
                    assert_eq!(url.as_str(), "https://example.com/notes/2");
                },
                _ => panic!("Second item should be a URL"),
            }
        } else {
            panic!("Should be a Collection");
        }
    }
}
