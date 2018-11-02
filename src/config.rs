use std::env;

use sentry_integration::SentryConfig;

use config_crate::{Config as RawConfig, ConfigError, Environment, File};
use models::*;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: Server,
    pub database: Database,
    pub client: Client,
    pub cpu_pool: CpuPool,
    pub rabbit: Rabbit,
    pub auth: Auth,
    pub fee_price: FeePrice,
    pub system: System,
    pub sentry: Option<SentryConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Client {
    pub dns_threads: usize,
    pub keys_url: String,
    pub blockchain_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub host: String,
    pub port: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeePrice {
    pub bitcoin: u64,
    pub ethereum: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Database {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CpuPool {
    pub size: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Auth {
    pub keys_token: AuthenticationToken,
    pub keys_user_id: UserId,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Rabbit {
    pub url: String,
    pub thread_pool_size: usize,
    pub connection_timeout_secs: usize,
    pub connection_pool_size: usize,
    pub restart_subscription_secs: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct System {
    pub system_user_id: UserId,
    pub btc_liquidity_account_id: AccountId,
    pub eth_liquidity_account_id: AccountId,
    pub stq_liquidity_account_id: AccountId,
    pub btc_fees_account_id: AccountId,
    pub eth_fees_account_id: AccountId,
    pub stq_fees_account_id: AccountId,
    pub keys_system_user_id: UserId,
    pub keys_system_user_token: AuthenticationToken,
}

impl Config {
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = RawConfig::new();
        s.merge(File::with_name("config/base"))?;

        // Merge development.toml if RUN_MODE variable is not set
        let env = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        s.merge(File::with_name(&format!("config/{}", env)).required(false))?;
        s.merge(File::with_name("config/secret.toml").required(false))?;

        s.merge(Environment::with_prefix("STQ_TRANSACTIONS"))?;
        s.try_into()
    }
}
