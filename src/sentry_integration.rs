use sentry;

#[derive(Debug, Deserialize, Clone)]
pub struct SentryConfig {
    pub dsn: String,
}

pub fn init(sentry_config: Option<&SentryConfig>) -> Option<sentry::internals::ClientInitGuard> {
    sentry_config.map(|config_sentry| {
        info!("initialization support with sentry");
        let result = sentry::init((
            config_sentry.dsn.clone(),
            sentry::ClientOptions {
                release: sentry_crate_release!(),
                ..Default::default()
            },
        ));
        sentry::integrations::panic::register_panic_handler();
        result
    })
}
