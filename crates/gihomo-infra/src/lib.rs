//! Infrastructure adapters for external APIs, system autostart, storage, supervision, and proxy settings.

pub mod api;
pub mod autostart;
pub mod error;
pub mod kernel;
pub mod storage;
pub mod sysproxy;

pub use api::MihomoApiClient;
pub use autostart::{is_autostart_enabled, set_autostart};
pub use error::InfraError;
pub use kernel::KernelManager;
pub use storage::{RemoteSubscriptionDownload, StorageManager, APP_ID};
pub use sysproxy::SystemProxyManager;

/// Trims the process heap by returning unused memory to the operating system.
///
/// On Linux with glibc, this invokes `libc::malloc_trim(0)`. On other targets,
/// this is a safe no-op.
pub fn trim_memory() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: `malloc_trim(0)` is an async-signal-safe, thread-safe glibc extension.
    // It takes an integer padding (0 requests releasing all free memory possible),
    // mutates only allocator-internal free lists, and does not dereference user pointers.
    unsafe {
        libc::malloc_trim(0);
    }
}
