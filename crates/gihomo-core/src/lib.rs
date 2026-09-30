pub mod config;
pub mod error;
pub mod model;
pub mod parser;

pub use config::{
    generate_base_config, merge_subscription_config, parse_user_info_header,
    secure_controller_config,
};
pub use error::CoreError;
pub use model::{
    ConnectionItem, ConnectionMetadata, ConnectionsSnapshot, GeoDatabaseInfo, KernelStatus,
    LogMessage, ProxyGroup, ProxyNode, RuleItem, RuleProvider, Subscription, SubscriptionSource,
    SubscriptionUserInfo, TrafficStats, TunConfig,
};
pub use parser::{
    decode_base64_flexible, generate_profile_from_proxies, parse_share_links,
    parse_single_share_link, ParsedProxyNode,
};
