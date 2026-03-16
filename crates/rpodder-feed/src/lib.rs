//! Feed fetching and parsing for rpodder.
//!
//! Fetches RSS/Atom feeds via HTTP with conditional GET support,
//! parses them with feed-rs, and maps the results to rpodder domain types.

mod fetcher;
mod parser;

pub use fetcher::{FeedFetcher, FetchResult};
pub use parser::parse_feed;
