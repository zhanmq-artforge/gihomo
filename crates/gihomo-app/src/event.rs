//! Application reactive event definitions.

use gihomo_core::{KernelStatus, ProxyGroup, RuleItem, Subscription, TrafficStats};

/// Reactive event broadcasted across the application lifecycle.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AppEvent {
    /// System proxy mode changed (enabled or disabled).
    ProxyStatusChanged(bool),
    /// TUN mode status changed (enabled or disabled).
    TunStatusChanged(bool),
    /// Kernel process status changed (running, stopped, error).
    KernelStatusChanged(KernelStatus),
    /// Real-time traffic rate and cumulative totals updated.
    TrafficUpdated(TrafficStats),
    /// Subscription list updated.
    SubscriptionsChanged(Vec<Subscription>),
    /// Currently active subscription changed.
    ActiveSubscriptionChanged(Option<Subscription>),
    /// Proxy selector groups and node lists updated.
    ProxyGroupsUpdated(Vec<ProxyGroup>),
    /// Proxy routing mode changed (Rule, Global, Direct).
    ProxyModeChanged(String),
    /// Routing rules and rule providers updated.
    RulesUpdated(Vec<RuleItem>),
    /// Geo database files updated.
    GeoUpdated,
    /// Toast notification message to display to the user.
    Notification(String),
    /// Error notification to display to the user.
    ErrorOccurred(String),
}
