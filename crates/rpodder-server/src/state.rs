use std::sync::Arc;

use rpodder_db::Db;

/// Shared application state, available in all route handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
}
