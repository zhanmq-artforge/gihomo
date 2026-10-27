//! Modal dialogs for adding and editing subscription resources.

pub mod add_subscription;
pub mod edit_subscription;

pub use add_subscription::show_add_subscription_dialog;
pub use edit_subscription::show_edit_subscription_dialog;
