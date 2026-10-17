//! Playback: the pure queue state machine lives in `queue`; `engine` is the bridge to the single
//! hidden `<audio>` element, `controller` owns playback state and the public transport API,
//! `session` saves/restores the queue and `media_keys` connects the Windows media overlay.
pub mod queue;

pub mod controller;
pub mod effects;
pub mod engine;
mod events;
mod loader;
pub mod media_keys;
pub mod policy;
mod queue_edit;
mod radio;
pub mod session;
