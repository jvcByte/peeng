//! Refresh tokens feature module.
//!
//! This module groups refresh-token related functionality. Currently it exposes
//! the repository responsible for persisting, looking up and revoking refresh
//! tokens. Additional handlers/services can be added under this module as the
//! feature grows.

pub mod repository;
