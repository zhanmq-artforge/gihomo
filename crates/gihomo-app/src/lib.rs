//! Application service layer orchestrating domain logic, background workers, and UI events.

pub mod error;
pub mod event;
pub mod service;

pub use error::AppError;
pub use event::AppEvent;
pub use service::AppService;
