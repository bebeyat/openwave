//! The search screen (`/search?q=`). The top search field lives in the shell and drives `q`.

pub mod idle;
pub mod logic;
pub mod page;
pub mod results;

pub use page::Search;
