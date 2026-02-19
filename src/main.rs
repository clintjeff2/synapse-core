mod config;
mod db;
mod error;
mod handlers;
mod services;
mod stellar;

use axum::{
    Router,
    routing::{get, put},
};
use db::pool_manager::PoolManager;
use services::FeatureFlagService;
use sqlx::migrate::Migrator;
use std::net::SocketAddr;
use std::path::Path;
use stellar::HorizonClient;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    db: sqlx::PgPool,
    pub pool_manager: PoolManager,
    pub horizon_client: HorizonClient,
    pub feature_flags: FeatureFlagService,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::Config::from_env()?;

    // Setup logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Database pool
    let pool = db::create_pool(&config).await?;

    // Initialize pool manager for multi-region failover
    let pool_manager = PoolManager::new(
        &config.database_url,
        config.database_replica_url.as_deref(),
    )
    .await?;
    
    if pool_manager.replica().is_some() {
        tracing::info!("Database replica configured - read queries will be routed to replica");
    } else {
        tracing::info!("No replica configured - all queries will use primary database");
    }

    // Run migrations
    let migrator = Migrator::new(Path::new("./migrations")).await?;
    migrator.run(&pool).await?;
    tracing::info!("Database migrations completed");

    // Initialize partition manager (runs every 24 hours)
    let partition_manager = db::partition::PartitionManager::new(pool.clone(), 24);
    partition_manager.start();
    tracing::info!("Partition manager started");

    // Initialize Stellar Horizon client
    let horizon_client = HorizonClient::new(config.stellar_horizon_url.clone());
    tracing::info!(
        "Stellar Horizon client initialized with URL: {}",
        config.stellar_horizon_url
    );

    // Initialize feature flags service
    let feature_flags = FeatureFlagService::new(pool.clone());
    feature_flags.refresh_cache().await?;
    feature_flags.start(1); // Refresh every 1 hour
    tracing::info!("Feature flags service initialized");

    // Build router with state
    let app_state = AppState {
        db: pool,
        pool_manager,
        horizon_client,
        feature_flags,
    };
    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/admin/flags", get(handlers::admin::get_flags))
        .route("/admin/flags/:name", put(handlers::admin::update_flag))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    tracing::info!("listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
