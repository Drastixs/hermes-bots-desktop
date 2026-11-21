//! Client for the Hermes Agent API.
//!
//! This crate only speaks the documented Hermes HTTP and WebSocket API:
//! the native PKCE sign-in (`/auth/native/*`), the REST routes under `/api/*`,
//! the gateway JSON-RPC socket `/api/ws`, and the Bot Screen RFB bridge
//! `/api/display/ws`. It never loads or scrapes the Hermes Webapp UI.

pub mod auth;
pub mod backoff;
pub mod gateway;
pub mod redact;
pub mod rest;
pub mod rfb;
pub mod types;
pub mod url_util;

pub use rest::{ApiError, RestClient};
