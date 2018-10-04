extern crate futures;
#[macro_use]
extern crate failure;
extern crate diesel;
extern crate hyper;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate config as config_crate;
#[macro_use]
extern crate http_router;
extern crate base64;
extern crate hyper_tls;
extern crate jsonwebtoken;
extern crate regex;
#[macro_use]
extern crate validator_derive;
extern crate validator;
#[macro_use]
extern crate sentry;

#[macro_use]
mod macros;
mod api;
mod client;
mod config;
mod models;
mod prelude;
mod sentry_integration;
mod services;
mod utils;

use config::Config;

pub fn hello() {
    println!("Hello world");
}

pub fn print_config() {
    println!("Parsed config: {:?}", get_config());
}

pub fn start_server() {
    let config = get_config();
    // Prepare sentry integration
    let _sentry = sentry_integration::init(config.sentry.as_ref());
    api::start_server(config);
}

fn get_config() -> Config {
    config::Config::new().unwrap_or_else(|e| panic!("Error parsing config: {}", e))
}
