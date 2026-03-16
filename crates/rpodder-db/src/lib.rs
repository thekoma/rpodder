//! Database layer for rpodder.
//!
//! Provides implementations of the repository traits defined in rpodder-core
//! for both PostgreSQL and SQLite via sqlx.

pub mod postgres;
pub mod sqlite;
