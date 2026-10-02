use gihomo_app::AppService;
use gihomo_ui::{prelude::*, ExitCode, GihomoApplication, APPLICATION_ID};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn init_logging() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("gihomo=debug,gihomo_ui=debug,gihomo_app=debug,gihomo_infra=debug,info")
    });

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn main() -> ExitCode {
    init_logging();

    info!(
        app_name = "Gihomo",
        app_id = APPLICATION_ID,
        version = env!("CARGO_PKG_VERSION"),
        "Starting Gihomo - Modern Native Mihomo Client"
    );

    // Initialize multi-threaded Tokio runtime and enter guard so background tasks can be spawned
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime");
    let _guard = rt.enter();

    gihomo_ui::i18n::preload_config();
    let app = GihomoApplication::new();

    // Register with D-Bus to determine if we are the primary instance or a remote (secondary) instance
    if let Err(e) = app.register(gio::Cancellable::NONE) {
        tracing::warn!("Failed to register application on D-Bus: {}", e);
    }

    if app.is_remote() {
        info!("Running as secondary instance, forwarding activation to primary instance");
        return app.run();
    }

    // --- Below is ONLY executed by the Primary Instance ---

    // Create a persistent local-only controller secret before starting background tasks.
    let service = rt
        .block_on(AppService::new(7890, 9090))
        .expect("Failed to initialize application service");

    // Initialize environment & background tasks
    let svc_clone = service.clone();
    rt.block_on(async move {
        svc_clone.init().await;
    });

    app.set_service(service.clone());

    // Listen for Unix SIGINT (Ctrl+C) and SIGTERM to initiate graceful shutdown
    tokio::spawn(async move {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigint = signal(SignalKind::interrupt()).ok();
        let mut sigterm = signal(SignalKind::terminate()).ok();

        tokio::select! {
            _ = async {
                if let Some(s) = sigint.as_mut() {
                    s.recv().await
                } else {
                    std::future::pending().await
                }
            } => {
                info!("Received SIGINT (Ctrl+C), initiating graceful shutdown");
            }
            _ = async {
                if let Some(s) = sigterm.as_mut() {
                    s.recv().await
                } else {
                    std::future::pending().await
                }
            } => {
                info!("Received SIGTERM, initiating graceful shutdown");
            }
        }

        glib::idle_add_once(|| {
            if let Some(app) = gio::Application::default() {
                use gio::prelude::*;
                app.quit();
            }
        });
    });

    let status = app.run();
    info!(exit_code = ?status, "Gihomo primary instance terminated gracefully");
    status
}
