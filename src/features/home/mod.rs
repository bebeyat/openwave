//! Home screen (`/`): welcome and starter actions on first run, the user's own music afterwards.

pub mod cache;
pub mod discover;
pub mod logic;
pub mod page;
pub mod recommend;
pub mod row;
pub mod sections;
pub mod taste;
pub mod taste_cache;
pub mod welcome;

pub use page::Home;
