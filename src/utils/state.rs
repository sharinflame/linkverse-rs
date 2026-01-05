use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use fred::clients::Client as RedisClient;
use fred::prelude::{self, ClientLike};
use once_cell::sync::Lazy;
use std::env;
use std::sync::Arc;
use thiserror::Error;
use tokio_postgres::{Config as PgConfig, NoTls};

pub static CONFIG: Lazy<Config> = Lazy::new(|| Config::from_env());

#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub cache_url: String,
    pub sessions_url: String,
    pub pubsub_url: String,
}

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    host: String,
    user: String,
    database: String,
    connections: u32,
    password: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub secret_auth_key: String,
    pub secret_refresh_key: String,
    pub signature_key: String,
    pub url: String,
    pub server_id: u8,
    pub total_servers: u8,
    pub cdn_secret_key: String,
    pub cdn_secret_key_n: String,
    pub vapid_secret: String,
    pub vapid_pub: String,
    pub brevo_api_key: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            secret_auth_key: env::var("SECRET_AUTH_KEY").expect("$SECRET_AUTH_KEY missing"),
            secret_refresh_key: env::var("SECRET_REFRESH_KEY")
                .expect("$SECRET_REFRESH_KEY missing"),
            signature_key: env::var("SIGNATURE_KEY").expect("$SIGNATURE_KEY missing"),
            url: env::var("URL").unwrap_or("localhost:8080".to_string()),
            server_id: env::var("SERVER_ID")
                .unwrap_or("0".to_string())
                .parse()
                .expect("SERVER_ID wrong type"),
            total_servers: env::var("TOTAL_SERVERS")
                .unwrap_or("1".to_string())
                .parse()
                .expect("TOTAL_SERVER wrong type"),
            cdn_secret_key: env::var("CDN_SECRET_KEY").expect("CDN_SECRET_KEY missing"),
            cdn_secret_key_n: env::var("CDN_SECRET_KEY_N").expect("CDN_SECRET_KEY missing"),
            vapid_secret: env::var("VAPID_SECRET").expect("VAPID_SECRET missing"),
            vapid_pub: env::var("VAPID_PUB").expect("VAPID_PUB missing"),
            brevo_api_key: env::var("BREVO_API_KEY").expect("BREVO_API_KEY missing"),
        }
    }
}

impl RedisConfig {
    pub fn from_env() -> Self {
        Self {
            cache_url: env::var("CACHE_REDIS_URL").expect("CACHE_REDIS_URL missing"),
            sessions_url: env::var("SESSIONS_REDIS_URL").expect("SESSIONS_REDIS_URL missing"),
            pubsub_url: env::var("PUBSUB_REDIS_URL").expect("PUBSUB_REDIS_URL missing"),
        }
    }
}

impl PostgresConfig {
    pub fn from_env() -> Self {
        Self {
            host: env::var("POSTGRES_HOST").expect("POSTGRES_HOST missing"),
            user: env::var("POSTGRES_USER").expect("POSTGRES_USER missing"),
            database: env::var("POSTGRES_DATABASE").expect("POSTGRES_DATABASE missing"),
            password: env::var("POSTGRES_PASSWORD").expect("POSTGRES_PASSWORD missing"),
            connections: env::var("POSTGRES_CONNECTIONS")
                .unwrap_or("1000".to_string())
                .parse()
                .expect("POSTGRES_CONNECTIONS wrong type"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub db_pool: Arc<Pool>,

    pub cache_redis: Arc<RedisClient>,
    pub sessions_redis: Arc<RedisClient>,
    pub pubsub_redis: Arc<RedisClient>,
}

#[derive(Error, Debug)]
pub enum AppStateError {
    #[error("SQL error: {0}")]
    Sql(#[from] tokio_postgres::Error),

    #[error("Redis error: {0}")]
    Redis(#[from] fred::error::Error),
}

impl AppState {
    pub async fn create_from_env() -> Result<AppState, AppStateError> {
        let postgres_config = PostgresConfig::from_env();
        let redis_config = RedisConfig::from_env();

        let mut pg_config = PgConfig::new();
        pg_config.host(&postgres_config.host);
        pg_config.user(&postgres_config.user);
        pg_config.password(&postgres_config.password);
        pg_config.dbname(&postgres_config.database);

        let mgr = Manager::from_config(
            pg_config,
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            },
        );
        let db_pool = Pool::builder(mgr)
            .max_size(postgres_config.connections as usize)
            .build()
            .unwrap();

        let cache_redis_config = prelude::Config::from_url(&redis_config.cache_url)?;
        let sessions_redis_config = prelude::Config::from_url(&redis_config.sessions_url)?;
        let pubsub_redis_config = prelude::Config::from_url(&redis_config.pubsub_url)?;

        let cache_redis = prelude::Builder::from_config(cache_redis_config).build()?;
        let sessions_redis = prelude::Builder::from_config(sessions_redis_config).build()?;
        let pubsub_redis = prelude::Builder::from_config(pubsub_redis_config).build()?;

        cache_redis.init().await?;
        sessions_redis.init().await?;
        pubsub_redis.init().await?;

        Ok(AppState {
            db_pool: Arc::new(db_pool),
            cache_redis: Arc::new(cache_redis),
            sessions_redis: Arc::new(sessions_redis),
            pubsub_redis: Arc::new(pubsub_redis),
        })
    }
}

pub type ArcAppState = Arc<AppState>;
