pub mod activities;
pub mod domains;
pub mod health;
pub mod keys;
pub mod notes;
pub mod persons;
pub mod users;

use axum::Router;
use axum::routing::{delete, get, post, put};

use crate::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        // Health check (no auth required)
        .route("/health", get(health::health_check))
        // Domains
        .route("/api/v1/domains", get(domains::list_domains))
        .route("/api/v1/domains", post(domains::create_domain))
        .route("/api/v1/domains/{name}", get(domains::get_domain))
        .route("/api/v1/domains/{name}", put(domains::update_domain))
        .route("/api/v1/domains/{name}", delete(domains::delete_domain))
        // Users
        .route("/api/v1/users", get(users::list_users))
        .route("/api/v1/users", post(users::create_user))
        .route("/api/v1/users/{username}", get(users::get_user))
        // Persons
        .route("/api/v1/persons", post(persons::create_person))
        .route("/api/v1/persons/{id}", put(persons::update_person))
        .route("/api/v1/persons/{id}", delete(persons::delete_person))
        // Notes
        .route("/api/v1/notes", post(notes::create_note))
        .route("/api/v1/notes/{id}", put(notes::update_note))
        .route("/api/v1/notes/{id}", delete(notes::delete_note))
        // Activities
        .route("/api/v1/activities/follow", post(activities::follow))
        .route("/api/v1/activities/like", post(activities::like))
        .route("/api/v1/activities/announce", post(activities::announce))
        // Keys
        .route("/api/v1/keys/generate", post(keys::generate_key))
}
