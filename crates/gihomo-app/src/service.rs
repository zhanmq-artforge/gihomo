use crate::error::AppError;
use crate::event::AppEvent;
use chrono::Utc;
use gihomo_core::{
    generate_base_config, merge_subscription_config, KernelStatus, Subscription, SubscriptionSource,
};
use gihomo_infra::{
    KernelManager, MihomoApiClient, RemoteSubscriptionDownload, StorageManager, SystemProxyManager,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

#[derive(Clone)]
pub struct AppService {
    storage: StorageManager,
    api: MihomoApiClient,
    proxy: Arc<SystemProxyManager>,
    mixed_port: u16,
    controller_port: u16,
    secret: String,
    event_tx: tokio::sync::broadcast::Sender<AppEvent>,
    tun_enabled: Arc<AtomicBool>,
    active_sub_id: Arc<RwLock<Option<String>>>,
}

#[derive(Debug, Clone, Default)]
pub struct SubscriptionUpdateSummary {
    pub total: usize,
    pub updated: usize,
    pub failures: Vec<String>,
}

impl AppService {
    pub async fn new(mixed_port: u16, controller_port: u16) -> Result<Self, AppError> {
        let storage = StorageManager::new();
        storage.init_environment().await?;
        let secret = storage.load_or_create_controller_secret().await?;
        let api_url = format!("http://127.0.0.1:{}", controller_port);
        let api = MihomoApiClient::new(api_url, &secret);
        let proxy = Arc::new(SystemProxyManager::new()?);
        let (event_tx, _) = tokio::sync::broadcast::channel(256);

        Ok(Self {
            storage,
            api,
            proxy,
            mixed_port,
            controller_port,
            secret,
            event_tx,
            tun_enabled: Arc::new(AtomicBool::new(false)),
            active_sub_id: Arc::new(RwLock::new(None)),
        })
    }

    pub fn event_receiver(&self) -> tokio::sync::broadcast::Receiver<AppEvent> {
        self.event_tx.subscribe()
    }

    pub fn emit_event(&self, event: AppEvent) {
        let _ = self.event_tx.send(event);
    }

    pub async fn get_subscriptions(&self) -> Result<Vec<Subscription>, AppError> {
        self.storage
            .load_subscriptions()
            .await
            .map_err(|e| AppError::from(e.to_string()))
    }

    pub async fn get_active_subscription(&self) -> Option<Subscription> {
        let guard = self.active_sub_id.read().await;
        if let Some(id) = guard.as_ref() {
            if let Ok(subs) = self.storage.load_subscriptions().await {
                return subs.into_iter().find(|s| &s.id == id);
            }
        }
        None
    }

    pub async fn can_start_kernel(&self) -> bool {
        self.storage.active_config_path().exists() || self.get_active_subscription().await.is_some()
    }

    pub fn mihomo_dir(&self) -> PathBuf {
        self.storage.mihomo_dir()
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.storage.logs_dir()
    }

    pub async fn autostart_enabled(&self) -> Result<bool, AppError> {
        tokio::task::spawn_blocking(gihomo_infra::is_autostart_enabled)
            .await
            .map_err(|e| AppError::from(e.to_string()))
    }

    pub async fn set_autostart(&self, enabled: bool) -> Result<(), AppError> {
        tokio::task::spawn_blocking(move || gihomo_infra::set_autostart(enabled))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| AppError::from(e.to_string()))
    }

    pub fn is_proxy_enabled(&self) -> bool {
        self.proxy.is_enabled()
    }

    pub fn is_tun_enabled(&self) -> bool {
        self.tun_enabled.load(Ordering::SeqCst)
    }

    pub fn find_kernel_binary(&self) -> Option<PathBuf> {
        KernelManager::find_kernel_binary(&self.storage)
    }

    pub async fn check_tun_capabilities(&self) -> bool {
        if let Some(bin) = self.find_kernel_binary() {
            KernelManager::check_tun_capabilities(&bin).await
        } else {
            false
        }
    }

    pub async fn request_tun_permissions(&self) -> Result<(), AppError> {
        let bin = self
            .find_kernel_binary()
            .ok_or_else(|| "未找到 Mihomo 内核文件，请先安装或下载内核".to_string())?;
        KernelManager::request_tun_permissions(&bin)
            .await
            .map_err(|e| AppError::from(format!("{}", e)))
    }

    pub async fn get_kernel_version(&self) -> Option<String> {
        if let Some(bin) = self.find_kernel_binary() {
            KernelManager::get_kernel_version(&bin).await
        } else {
            None
        }
    }

    /// Initialize storage, wire system proxy events, and start background workers
    pub async fn init(&self) {
        // Load subscriptions and set active sub id
        if let Ok(subs) = self.storage.load_subscriptions().await {
            if let Some(active) = subs.iter().find(|s| s.is_active).cloned() {
                let mut guard = self.active_sub_id.write().await;
                *guard = Some(active.id.clone());
                self.emit_event(AppEvent::ActiveSubscriptionChanged(Some(active)));
            } else if let Some(first) = subs.first() {
                // If there are subscriptions but none active, auto activate first
                let first_id = first.id.clone();
                let _ = self.activate_subscription(&first_id).await;
            } else {
                self.emit_event(AppEvent::ActiveSubscriptionChanged(None));
            }
            self.emit_event(AppEvent::SubscriptionsChanged(subs));
        }

        // Send initial system proxy status
        let initial_proxy_status = self.proxy.is_enabled();
        self.emit_event(AppEvent::ProxyStatusChanged(initial_proxy_status));

        // Wire GNOME system proxy changes
        let tx = self.event_tx.clone();
        self.proxy.connect_mode_changed(move |enabled| {
            let _ = tx.send(AppEvent::ProxyStatusChanged(enabled));
        });

        // Spawn background monitoring task
        self.spawn_background_worker();
    }

    fn spawn_background_worker(&self) {
        let this = self.clone();

        // 1. Initial status check
        tokio::spawn({
            let this = this.clone();
            async move {
                if let Ok(status) =
                    KernelManager::check_status(&this.storage, this.controller_port, &this.secret)
                        .await
                {
                    this.emit_event(AppEvent::KernelStatusChanged(status));
                }
            }
        });

        // 2. WebSocket Traffic Streaming & Kernel Liveness Task
        tokio::spawn(async move {
            let (traffic_tx, traffic_rx) = async_channel::bounded(10);
            let is_running = Arc::new(AtomicBool::new(false));

            // Forward WebSocket traffic stats to global event channel
            let this_forwarder = this.clone();
            let is_running_forwarder = is_running.clone();
            tokio::spawn(async move {
                while let Ok(stats) = traffic_rx.recv().await {
                    this_forwarder.emit_event(AppEvent::TrafficUpdated(stats));
                    // If receiving traffic, kernel is confirmed Running
                    if !is_running_forwarder.swap(true, Ordering::SeqCst) {
                        this_forwarder
                            .emit_event(AppEvent::KernelStatusChanged(KernelStatus::Running));
                    }
                }
            });

            loop {
                debug!("Connecting to Mihomo WebSocket traffic stream...");
                let stream_res = this.api.stream_traffic(traffic_tx.clone()).await;

                if let Err(e) = stream_res {
                    debug!("WebSocket traffic stream disconnected: {}", e);
                }

                is_running.store(false, Ordering::SeqCst);

                // When stream disconnects (e.g. during config reload or sleep), check actual kernel status
                // Only broadcast non-running status if the process has genuinely terminated
                match KernelManager::check_status(&this.storage, this.controller_port, &this.secret)
                    .await
                {
                    Ok(status) => {
                        if status != KernelStatus::Running {
                            this.emit_event(AppEvent::KernelStatusChanged(status));
                        }
                    }
                    Err(e) => {
                        debug!("Kernel check status error: {}", e);
                    }
                }

                // Backoff before reconnecting to WebSocket
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });

        // 3. Low-frequency TUN status sync worker (every 10s)
        let this_tun = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                if let Ok(configs) = this_tun.api.get_configs().await {
                    if let Some(tun_obj) = configs.get("tun") {
                        if let Some(enable) = tun_obj.get("enable").and_then(|e| e.as_bool()) {
                            let prev = this_tun.tun_enabled.swap(enable, Ordering::SeqCst);
                            if prev != enable {
                                this_tun.emit_event(AppEvent::TunStatusChanged(enable));
                            }
                        }
                    }
                }
            }
        });
    }

    /// Add a new subscription from URL
    pub async fn add_url_subscription(
        &self,
        name: String,
        url: String,
    ) -> Result<Subscription, AppError> {
        info!("Adding remote subscription: {} ({})", name, url);
        let mut sub = Subscription::new_url(name, url.clone());

        match self.storage.download_remote_subscription(&url, None).await {
            Ok(RemoteSubscriptionDownload::Modified {
                content,
                etag,
                user_info,
            }) => {
                sub.etag = etag;
                sub.user_info = user_info;
                sub.updated_at = Some(Utc::now());

                self.storage
                    .save_subscription_content(&sub.id, &content)
                    .await
                    .map_err(|e| format!("保存订阅内容失败: {}", e))?;

                let mut subs = self.storage.load_subscriptions().await.unwrap_or_default();
                subs.push(sub.clone());
                self.storage
                    .save_subscriptions(&subs)
                    .await
                    .map_err(|e| e.to_string())?;

                self.emit_event(AppEvent::SubscriptionsChanged(subs.clone()));

                // If this is the only subscription, auto-activate it
                if subs.len() == 1 {
                    let _ = self.activate_subscription(&sub.id).await;
                }

                self.emit_event(AppEvent::Notification(format!(
                    "订阅 [{}] 添加成功",
                    sub.name
                )));

                Ok(sub)
            }
            Ok(RemoteSubscriptionDownload::NotModified { .. }) => {
                Err("拉取订阅失败: 远程服务返回内容为空".into())
            }
            Err(e) => Err(format!("拉取订阅失败: {}", e).into()),
        }
    }

    /// Add a new subscription from local file
    pub async fn add_local_subscription(
        &self,
        name: String,
        path: &Path,
    ) -> Result<Subscription, AppError> {
        info!("Adding local subscription: {} from {:?}", name, path);
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| format!("读取本地文件失败: {}", e))?;

        let sub = Subscription::new_local(name, path.to_string_lossy().to_string());
        self.storage
            .save_subscription_content(&sub.id, &content)
            .await
            .map_err(|e| format!("保存订阅内容失败: {}", e))?;

        let mut subs = self.storage.load_subscriptions().await.unwrap_or_default();
        subs.push(sub.clone());
        self.storage
            .save_subscriptions(&subs)
            .await
            .map_err(|e| e.to_string())?;

        self.emit_event(AppEvent::SubscriptionsChanged(subs.clone()));

        if subs.len() == 1 {
            let _ = self.activate_subscription(&sub.id).await;
        }

        self.emit_event(AppEvent::Notification(format!(
            "本地配置 [{}] 导入成功",
            sub.name
        )));

        Ok(sub)
    }

    /// Add a new subscription from share links (ss://, vmess://, vless://, trojan://, hy2://)
    pub async fn add_share_links_subscription(
        &self,
        name: String,
        links_text: String,
    ) -> Result<Subscription, AppError> {
        info!("Adding share links subscription: {}", name);
        let (nodes, errors) = gihomo_core::parse_share_links(&links_text);
        if nodes.is_empty() {
            let msg = if !errors.is_empty() {
                format!("未识别到有效节点: {}", errors.join("; "))
            } else {
                "未在输入中识别到有效的代理节点链接".to_string()
            };
            return Err(msg.into());
        }

        let content = gihomo_core::generate_profile_from_proxies(&nodes)
            .map_err(|e| format!("生成代理配置失败: {}", e))?;

        let sub = Subscription::new_share_links(name, links_text);
        self.storage
            .save_subscription_content(&sub.id, &content)
            .await
            .map_err(|e| format!("保存订阅内容失败: {}", e))?;

        let mut subs = self.storage.load_subscriptions().await.unwrap_or_default();
        subs.push(sub.clone());
        self.storage
            .save_subscriptions(&subs)
            .await
            .map_err(|e| e.to_string())?;

        self.emit_event(AppEvent::SubscriptionsChanged(subs.clone()));

        if subs.len() == 1 {
            let _ = self.activate_subscription(&sub.id).await;
        }

        self.emit_event(AppEvent::Notification(format!(
            "已成功导入 {} 个节点到 [{}]",
            nodes.len(),
            sub.name
        )));

        Ok(sub)
    }

    /// Delete a subscription by ID
    pub async fn delete_subscription(&self, id: &str) -> Result<(), AppError> {
        info!("Deleting subscription {}", id);
        let is_active = {
            let active_id = self.active_sub_id.read().await.clone();
            active_id.as_deref() == Some(id)
        };

        self.storage
            .delete_subscription(id)
            .await
            .map_err(|e| format!("删除订阅失败: {}", e))?;

        let subs = self.storage.load_subscriptions().await.unwrap_or_default();
        if is_active {
            if let Some(next_sub) = subs.first() {
                let _ = self.activate_subscription(&next_sub.id).await;
            } else {
                let mut guard = self.active_sub_id.write().await;
                *guard = None;
                self.emit_event(AppEvent::ActiveSubscriptionChanged(None));
                let tun_enabled = self.tun_enabled.load(Ordering::SeqCst);
                let base_yaml = generate_base_config(
                    self.mixed_port,
                    self.controller_port,
                    &self.secret,
                    tun_enabled,
                );
                if let Ok(config_path) = self.storage.write_active_config(&base_yaml).await {
                    let _ = self
                        .api
                        .reload_config(&config_path.to_string_lossy(), true)
                        .await;
                }
            }
        }

        self.emit_event(AppEvent::SubscriptionsChanged(subs));
        self.emit_event(AppEvent::Notification("订阅已删除".to_string()));

        Ok(())
    }

    /// Update an existing subscription
    pub async fn update_subscription(&self, id: &str) -> Result<(), AppError> {
        self.update_subscription_inner(id, true).await
    }

    pub async fn update_all_subscriptions(&self) -> Result<SubscriptionUpdateSummary, AppError> {
        let subscriptions = self.storage.load_subscriptions().await?;
        let mut summary = SubscriptionUpdateSummary {
            total: subscriptions.len(),
            ..Default::default()
        };

        for subscription in subscriptions {
            match self
                .update_subscription_inner(&subscription.id, false)
                .await
            {
                Ok(()) => summary.updated += 1,
                Err(error) => summary
                    .failures
                    .push(format!("{}: {}", subscription.name, error)),
            }
        }

        let subscriptions = self.storage.load_subscriptions().await?;
        self.emit_event(AppEvent::SubscriptionsChanged(subscriptions));
        Ok(summary)
    }

    async fn update_subscription_inner(&self, id: &str, emit_events: bool) -> Result<(), AppError> {
        let mut subs = self.storage.load_subscriptions().await?;
        let sub_idx = subs
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| "订阅未找到".to_string())?;

        let mut sub = subs[sub_idx].clone();
        match &sub.source {
            SubscriptionSource::Url(url) => {
                let download_res = self
                    .storage
                    .download_remote_subscription(url, sub.etag.as_deref())
                    .await
                    .map_err(|e| format!("更新订阅失败: {}", e))?;

                let is_modified = match download_res {
                    RemoteSubscriptionDownload::Modified {
                        content,
                        etag,
                        user_info,
                    } => {
                        sub.etag = etag;
                        if user_info.is_some() {
                            sub.user_info = user_info;
                        }
                        sub.updated_at = Some(Utc::now());

                        self.storage
                            .save_subscription_content(&sub.id, &content)
                            .await
                            .map_err(|e| format!("保存订阅失败: {}", e))?;
                        true
                    }
                    RemoteSubscriptionDownload::NotModified { etag, user_info } => {
                        if etag.is_some() {
                            sub.etag = etag;
                        }
                        if user_info.is_some() {
                            sub.user_info = user_info;
                        }
                        sub.updated_at = Some(Utc::now());
                        false
                    }
                };

                subs[sub_idx] = sub.clone();
                self.storage
                    .save_subscriptions(&subs)
                    .await
                    .map_err(|e| e.to_string())?;

                let is_active = sub.is_active;
                if is_active && is_modified {
                    let _ = self.activate_subscription(&sub.id).await;
                }

                if emit_events {
                    self.emit_event(AppEvent::SubscriptionsChanged(subs));
                    let msg = if is_modified {
                        format!("订阅 [{}] 更新完成", sub.name)
                    } else {
                        format!("订阅 [{}] 已是最新 (无变更)", sub.name)
                    };
                    self.emit_event(AppEvent::Notification(msg));
                }

                Ok(())
            }
            SubscriptionSource::LocalFile(path) => {
                let content = tokio::fs::read_to_string(path)
                    .await
                    .map_err(|e| format!("读取本地文件失败: {}", e))?;

                sub.updated_at = Some(Utc::now());
                self.storage
                    .save_subscription_content(&sub.id, &content)
                    .await
                    .map_err(|e| format!("保存订阅失败: {}", e))?;

                subs[sub_idx] = sub.clone();
                self.storage
                    .save_subscriptions(&subs)
                    .await
                    .map_err(|e| e.to_string())?;

                if sub.is_active {
                    let _ = self.activate_subscription(&sub.id).await;
                }

                if emit_events {
                    self.emit_event(AppEvent::SubscriptionsChanged(subs));
                    self.emit_event(AppEvent::Notification(format!(
                        "本地配置 [{}] 重新载入成功",
                        sub.name
                    )));
                }

                Ok(())
            }
            SubscriptionSource::ShareLinks(raw_links) => {
                let (nodes, _) = gihomo_core::parse_share_links(raw_links);
                if nodes.is_empty() {
                    return Err("未识别到有效的代理节点".into());
                }
                let content = gihomo_core::generate_profile_from_proxies(&nodes)
                    .map_err(|e| format!("生成代理配置失败: {}", e))?;

                sub.updated_at = Some(Utc::now());
                self.storage
                    .save_subscription_content(&sub.id, &content)
                    .await
                    .map_err(|e| format!("保存订阅失败: {}", e))?;

                subs[sub_idx] = sub.clone();
                self.storage
                    .save_subscriptions(&subs)
                    .await
                    .map_err(|e| e.to_string())?;

                if sub.is_active {
                    let _ = self.activate_subscription(&sub.id).await;
                }

                if emit_events {
                    self.emit_event(AppEvent::SubscriptionsChanged(subs));
                    self.emit_event(AppEvent::Notification(format!(
                        "节点配置 [{}] 重新载入成功",
                        sub.name
                    )));
                }

                Ok(())
            }
        }
    }

    pub async fn edit_subscription(
        &self,
        id: &str,
        name: String,
        source: SubscriptionSource,
    ) -> Result<(), AppError> {
        let mut subscriptions = self.storage.load_subscriptions().await?;
        let index = subscriptions
            .iter()
            .position(|subscription| subscription.id == id)
            .ok_or_else(|| AppError::from("订阅未找到"))?;
        let mut subscription = subscriptions[index].clone();

        if subscription.source != source {
            match &source {
                SubscriptionSource::Url(url) => {
                    let download_res = self.storage.download_remote_subscription(url, None).await?;
                    match download_res {
                        RemoteSubscriptionDownload::Modified {
                            content,
                            etag,
                            user_info,
                        } => {
                            self.storage.save_subscription_content(id, &content).await?;
                            subscription.etag = etag;
                            subscription.user_info = user_info;
                        }
                        RemoteSubscriptionDownload::NotModified { etag, user_info } => {
                            subscription.etag = etag;
                            subscription.user_info = user_info;
                        }
                    }
                }
                SubscriptionSource::LocalFile(path) => {
                    let content = tokio::fs::read_to_string(path)
                        .await
                        .map_err(|error| AppError::from(error.to_string()))?;
                    self.storage.save_subscription_content(id, &content).await?;
                    subscription.etag = None;
                    subscription.user_info = None;
                }
                SubscriptionSource::ShareLinks(raw_links) => {
                    let (nodes, _) = gihomo_core::parse_share_links(raw_links);
                    let content = gihomo_core::generate_profile_from_proxies(&nodes)
                        .map_err(|error| AppError::from(error.to_string()))?;
                    self.storage.save_subscription_content(id, &content).await?;
                    subscription.etag = None;
                    subscription.user_info = None;
                }
            }
            subscription.updated_at = Some(Utc::now());
            subscription.source = source;
        }

        subscription.name = name;
        subscriptions[index] = subscription.clone();
        self.storage.save_subscriptions(&subscriptions).await?;

        if subscription.is_active {
            self.activate_subscription(id).await?;
        } else {
            self.emit_event(AppEvent::SubscriptionsChanged(subscriptions));
        }
        self.emit_event(AppEvent::Notification(format!(
            "订阅 [{}] 已保存",
            subscription.name
        )));
        Ok(())
    }

    /// Activate a subscription: merge config, write to active config path, trigger reload
    pub async fn activate_subscription(&self, id: &str) -> Result<(), AppError> {
        info!("Activating subscription {}", id);
        let sub_content = self
            .storage
            .read_subscription_content(id)
            .await
            .map_err(|e| format!("读取订阅配置失败: {}", e))?;

        let tun_enabled = self.tun_enabled.load(Ordering::SeqCst);
        let base_yaml = generate_base_config(
            self.mixed_port,
            self.controller_port,
            &self.secret,
            tun_enabled,
        );

        let merged_yaml = merge_subscription_config(
            &base_yaml,
            &sub_content,
            tun_enabled,
            self.mixed_port,
            self.controller_port,
            &self.secret,
        )
        .map_err(|e| format!("配置合成失败: {}", e))?;

        // Write to ~/.local/share/art.artforge.Gihomo/mihomo/config.yaml
        let _ = self
            .storage
            .write_active_config(&merged_yaml)
            .await
            .map_err(|e| format!("写入活动配置失败: {}", e))?;

        // Update is_active flags in metadata
        let mut subs = self.storage.load_subscriptions().await.unwrap_or_default();
        let mut active_name = String::new();
        for s in &mut subs {
            if s.id == id {
                s.is_active = true;
                active_name = s.name.clone();
            } else {
                s.is_active = false;
            }
        }
        self.storage
            .save_subscriptions(&subs)
            .await
            .map_err(|e| e.to_string())?;

        {
            let mut guard = self.active_sub_id.write().await;
            *guard = Some(id.to_string());
        }

        // Trigger Mihomo reload if running
        let kernel_status =
            KernelManager::check_status(&self.storage, self.controller_port, &self.secret)
                .await
                .unwrap_or(KernelStatus::Stopped);

        if kernel_status == KernelStatus::Running {
            let config_path = self.storage.active_config_path();
            let config_path_str = config_path.to_str().unwrap_or("config.yaml");
            if let Err(e) = self.api.reload_config(config_path_str, true).await {
                warn!("Mihomo API reload failed: {}, falling back to restart", e);
                let _ =
                    KernelManager::restart(&self.storage, self.controller_port, &self.secret).await;
            }
            let _ = self.fetch_proxies().await;
            self.emit_event(AppEvent::KernelStatusChanged(KernelStatus::Running));
        }

        let active_sub = subs.iter().find(|s| s.id == id).cloned();
        self.emit_event(AppEvent::SubscriptionsChanged(subs));
        self.emit_event(AppEvent::ActiveSubscriptionChanged(active_sub));
        self.emit_event(AppEvent::Notification(format!(
            "已激活订阅：{}",
            active_name
        )));

        Ok(())
    }

    /// Toggle GNOME system proxy
    pub fn toggle_proxy(&self, enable: bool) -> Result<(), AppError> {
        info!("Toggling system proxy: {}", enable);
        if enable {
            self.proxy
                .enable("127.0.0.1", self.mixed_port)
                .map_err(|e| format!("启用系统代理失败: {}", e))?;
        } else {
            self.proxy
                .disable()
                .map_err(|e| format!("关闭系统代理失败: {}", e))?;
        }
        Ok(())
    }

    /// Hot toggle TUN mode via REST API and sync config file
    pub async fn set_tun(&self, enable: bool) -> Result<(), AppError> {
        info!("Hot toggling TUN mode: {}", enable);
        self.tun_enabled.store(enable, Ordering::SeqCst);

        // Update active configuration on disk if active subscription exists
        let active_id = self.active_sub_id.read().await.clone();
        if let Some(id) = active_id {
            if let Ok(sub_content) = self.storage.read_subscription_content(&id).await {
                let base_yaml = generate_base_config(
                    self.mixed_port,
                    self.controller_port,
                    &self.secret,
                    enable,
                );
                if let Ok(merged_yaml) = merge_subscription_config(
                    &base_yaml,
                    &sub_content,
                    enable,
                    self.mixed_port,
                    self.controller_port,
                    &self.secret,
                ) {
                    let _ = self.storage.write_active_config(&merged_yaml).await;
                }
            }
        }

        // Hot patch running Mihomo instance via REST API
        let kernel_status =
            KernelManager::check_status(&self.storage, self.controller_port, &self.secret)
                .await
                .unwrap_or(KernelStatus::Stopped);

        if kernel_status == KernelStatus::Running {
            let mut patch = serde_json::Map::new();
            let mut tun_map = serde_json::Map::new();
            tun_map.insert("enable".to_string(), serde_json::Value::Bool(enable));
            patch.insert("tun".to_string(), serde_json::Value::Object(tun_map));

            self.api
                .patch_configs(serde_json::Value::Object(patch))
                .await
                .map_err(|e| format!("热修改 TUN 模式失败: {}", e))?;
        }

        self.emit_event(AppEvent::TunStatusChanged(enable));
        self.emit_event(AppEvent::Notification(if enable {
            "TUN 模式已开启".to_string()
        } else {
            "TUN 模式已关闭".to_string()
        }));
        Ok(())
    }

    /// Check current native Mihomo kernel status directly
    pub async fn check_kernel_status(&self) -> Result<KernelStatus, AppError> {
        KernelManager::check_status(&self.storage, self.controller_port, &self.secret)
            .await
            .map_err(|e| AppError::from(e.to_string()))
    }

    /// Start native Mihomo kernel
    pub async fn start_kernel(&self) -> Result<(), AppError> {
        let status = KernelManager::check_status(&self.storage, self.controller_port, &self.secret)
            .await
            .unwrap_or(KernelStatus::Stopped);
        if status == KernelStatus::Running {
            info!("Mihomo kernel is already running, skipping start");
            self.emit_event(AppEvent::KernelStatusChanged(KernelStatus::Running));
            let _ = self.fetch_proxies().await;
            return Ok(());
        }
        KernelManager::start(&self.storage, self.controller_port, &self.secret)
            .await
            .map_err(|e| format!("启动内核失败: {}", e))?;
        self.emit_event(AppEvent::Notification("Mihomo 内核已启动".to_string()));
        self.emit_event(AppEvent::KernelStatusChanged(KernelStatus::Running));
        let _ = self.fetch_proxies().await;
        Ok(())
    }

    /// Restart native Mihomo kernel
    pub async fn restart_kernel(&self) -> Result<(), AppError> {
        KernelManager::restart(&self.storage, self.controller_port, &self.secret)
            .await
            .map_err(|e| format!("重启内核失败: {}", e))?;
        self.emit_event(AppEvent::Notification("Mihomo 内核已重启".to_string()));
        self.emit_event(AppEvent::KernelStatusChanged(KernelStatus::Running));
        let _ = self.fetch_proxies().await;
        Ok(())
    }

    /// Stop native Mihomo kernel
    pub async fn stop_kernel(&self) -> Result<(), AppError> {
        KernelManager::stop(&self.storage)
            .await
            .map_err(|e| format!("停止内核失败: {}", e))?;
        self.emit_event(AppEvent::Notification("Mihomo 内核已停止".to_string()));
        self.emit_event(AppEvent::KernelStatusChanged(KernelStatus::Stopped));
        Ok(())
    }

    /// Graceful shutdown: resets system proxy to direct mode and terminates kernel
    pub async fn shutdown(&self) {
        info!("Initiating graceful AppService shutdown...");
        if self.proxy.is_enabled() {
            info!("System proxy is currently active; disabling to restore direct network access on exit");
            if let Err(e) = self.proxy.disable() {
                warn!("Failed to disable system proxy during shutdown: {}", e);
            }
        }
        let _ = KernelManager::stop(&self.storage).await;
        info!("AppService shutdown cleanup complete");
    }

    /// Fetch current proxy groups from Mihomo
    pub async fn fetch_proxies(&self) -> Result<Vec<gihomo_core::ProxyGroup>, AppError> {
        let groups = self
            .api
            .get_proxy_groups()
            .await
            .map_err(|e| format!("获取节点列表失败: {}", e))?;
        self.emit_event(AppEvent::ProxyGroupsUpdated(groups.clone()));
        Ok(groups)
    }

    /// Select a proxy node inside a group
    pub async fn select_proxy_node(&self, group: &str, node: &str) -> Result<(), AppError> {
        info!("Selecting node [{}] in group [{}]", node, group);
        self.api
            .select_proxy(group, node)
            .await
            .map_err(|e| format!("切换节点失败: {}", e))?;

        self.emit_event(AppEvent::Notification(format!(
            "已切换 [{}] -> {}",
            group, node
        )));

        // Refresh groups
        let _ = self.fetch_proxies().await;
        Ok(())
    }

    /// Test group delay (ping all nodes in group)
    pub async fn test_group_delay(&self, group: &str) -> Result<(), AppError> {
        info!("Testing delay for group [{}]", group);
        self.emit_event(AppEvent::Notification(format!(
            "正在测试 [{}] 延迟...",
            group
        )));

        self.api
            .test_group_delay(group, "http://www.gstatic.com/generate_204", 2500)
            .await
            .map_err(|e| format!("测速失败: {}", e))?;

        let _ = self.fetch_proxies().await;
        self.emit_event(AppEvent::Notification(format!("[{}] 测速完成", group)));
        Ok(())
    }

    /// Change proxy mode (rule / global / direct)
    pub async fn set_proxy_mode(&self, mode: &str) -> Result<(), AppError> {
        info!("Setting proxy mode to [{}]", mode);
        self.api
            .set_mode(mode)
            .await
            .map_err(|e| format!("更改模式失败: {}", e))?;

        self.emit_event(AppEvent::ProxyModeChanged(mode.to_string()));
        self.emit_event(AppEvent::Notification(format!("已切换至 [{}] 模式", mode)));
        Ok(())
    }

    /// Query current proxy mode from Mihomo (rule / global / direct)
    pub async fn get_proxy_mode(&self) -> Result<String, AppError> {
        self.api.get_mode().await.map_err(AppError::from)
    }

    /// Test individual node delay
    pub async fn test_node_delay(&self, _group: &str, node: &str) -> Result<u32, AppError> {
        info!("Testing delay for node [{}]", node);
        let delay = self
            .api
            .test_delay(node, "http://www.gstatic.com/generate_204", 2500)
            .await
            .map_err(|e| format!("测速失败: {}", e))?;

        let _ = self.fetch_proxies().await;
        Ok(delay)
    }

    /// Fetch all routing rules
    pub async fn fetch_rules(&self) -> Result<Vec<gihomo_core::RuleItem>, AppError> {
        let rules = self
            .api
            .get_rules()
            .await
            .map_err(|e| format!("获取分流规则失败: {}", e))?;
        self.emit_event(AppEvent::RulesUpdated(rules.clone()));
        Ok(rules)
    }

    /// Fetch all rule providers
    pub async fn fetch_rule_providers(&self) -> Result<Vec<gihomo_core::RuleProvider>, AppError> {
        self.api
            .get_rule_providers()
            .await
            .map_err(|e| AppError::from(format!("获取规则集失败: {}", e)))
    }

    /// Update a rule provider
    pub async fn update_rule_provider(&self, name: &str) -> Result<(), AppError> {
        info!("Updating rule provider [{}]", name);
        self.api
            .update_rule_provider(name)
            .await
            .map_err(|e| format!("更新规则集失败: {}", e))?;
        self.emit_event(AppEvent::Notification(format!(
            "规则集 [{}] 已触发更新",
            name
        )));
        let _ = self.fetch_rules().await;
        Ok(())
    }

    /// Retrieve local Geo database status
    pub async fn get_geo_databases(&self) -> Vec<gihomo_core::GeoDatabaseInfo> {
        self.storage.get_geo_databases().await
    }

    /// Update GeoIP and GeoSite databases from upstream
    pub async fn update_geo_databases(&self) -> Result<(), AppError> {
        info!("Starting Geo database update...");
        self.emit_event(AppEvent::Notification("正在更新 Geo 数据库...".to_string()));

        let geoip_dest = self.storage.geoip_path();
        let geosite_dest = self.storage.geosite_path();

        // 1. Download GeoIP (geoip.metadb)
        let geoip_urls = [
            "https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geoip.metadb",
            "https://ghproxy.net/https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geoip.metadb",
        ];
        let mut geoip_ok = false;
        for url in &geoip_urls {
            if self
                .storage
                .download_file_to(url, &geoip_dest)
                .await
                .is_ok()
            {
                geoip_ok = true;
                break;
            }
        }
        if !geoip_ok {
            return Err("下载 GeoIP 数据库失败，请检查网络连接".into());
        }

        // 2. Download GeoSite (geosite.dat)
        let geosite_urls = [
            "https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geosite.dat",
            "https://ghproxy.net/https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geosite.dat",
        ];
        let mut geosite_ok = false;
        for url in &geosite_urls {
            if self
                .storage
                .download_file_to(url, &geosite_dest)
                .await
                .is_ok()
            {
                geosite_ok = true;
                break;
            }
        }
        if !geosite_ok {
            return Err("下载 GeoSite 数据库失败，请检查网络连接".into());
        }

        // 3. Trigger Mihomo reload if running
        let kernel_status =
            KernelManager::check_status(&self.storage, self.controller_port, &self.secret)
                .await
                .unwrap_or(KernelStatus::Stopped);
        if kernel_status == KernelStatus::Running {
            let config_path = self.storage.active_config_path();
            let config_path_str = config_path.to_str().unwrap_or("config.yaml");
            let _ = self.api.reload_config(config_path_str, true).await;
        }

        self.emit_event(AppEvent::GeoUpdated);
        self.emit_event(AppEvent::Notification("Geo 数据库更新完成".to_string()));
        Ok(())
    }

    pub async fn get_connections(&self) -> Result<gihomo_core::ConnectionsSnapshot, AppError> {
        self.api
            .get_connections()
            .await
            .map_err(|e| AppError::from(e.to_string()))
    }

    pub async fn close_connection(&self, id: &str) -> Result<(), AppError> {
        self.api
            .close_connection(id)
            .await
            .map_err(|e| AppError::from(e.to_string()))
    }

    pub async fn close_all_connections(&self) -> Result<(), AppError> {
        self.api
            .close_all_connections()
            .await
            .map_err(|e| AppError::from(e.to_string()))
    }

    pub async fn stream_logs(
        &self,
        level: &str,
        sender: async_channel::Sender<gihomo_core::LogMessage>,
    ) -> Result<(), AppError> {
        self.api
            .stream_logs(level, sender)
            .await
            .map_err(|e| AppError::from(e.to_string()))
    }

    pub async fn get_recent_kernel_logs(&self, max_lines: usize) -> Vec<gihomo_core::LogMessage> {
        self.storage.read_recent_kernel_logs(max_lines).await
    }
}
