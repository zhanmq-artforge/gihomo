use gihomo_core::{KernelStatus, Subscription, TrafficStats};

#[derive(Debug, Clone)]
pub enum AppEvent {
    ProxyStatusChanged(bool),
    TunStatusChanged(bool),
    KernelStatusChanged(KernelStatus),
    TrafficUpdated(TrafficStats),
    SubscriptionsChanged(Vec<Subscription>),
    ActiveSubscriptionChanged(Option<Subscription>),
    ProxyGroupsUpdated(Vec<gihomo_core::ProxyGroup>),
    ProxyModeChanged(String),
    RulesUpdated(Vec<gihomo_core::RuleItem>),
    GeoUpdated,
    Notification(String),
    ErrorOccurred(String),
}
