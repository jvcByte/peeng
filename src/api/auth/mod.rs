//! Authentication feature module.
//!
//! This module groups the authentication submodules used by the API:
//! - `dto`      — request/response data transfer objects
//! - `handlers` — Actix HTTP handlers for auth endpoints
//! - `routes`   — route wiring for this feature
pub mod dto;
pub mod handlers;
pub mod repository;
pub mod routes;
pub mod service;
