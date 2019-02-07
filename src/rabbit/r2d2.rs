use std::fmt::{self, Debug};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use failure;
use lapin_async::connection::ConnectionState;
use lapin_futures::channel::{BasicQosOptions, Channel, ConfirmSelectOptions};
use lapin_futures::client::{Client, ConnectionOptions, HeartbeatHandle};

use prelude::*;
use regex::Regex;
use tokio::net::tcp::TcpStream;
use tokio::timer::timeout::Timeout;

use super::error::*;
use config::Config;
use utils::log_error;

// large limits may force RabbitMQ to close connection
// (in case of socket buffer overflow)
const CONSUMER_PREFETCH_COUNT: u16 = 1;

#[derive(Clone)]
pub struct RabbitConnectionManager {
    client: Arc<Mutex<Client<TcpStream>>>,
    heartbeat_handle: Arc<Mutex<RabbitHeartbeatHandle>>,
    connection_timeout: Duration,
    connection_options: ConnectionOptions,
    address: SocketAddr,
}

impl Debug for RabbitConnectionManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str("RabbitConnectionManager")
    }
}

struct RabbitHeartbeatHandle(Option<HeartbeatHandle>);

impl RabbitHeartbeatHandle {
    pub fn new(handle: HeartbeatHandle) -> Self {
        RabbitHeartbeatHandle(Some(handle))
    }
}

impl Drop for RabbitHeartbeatHandle {
    fn drop(&mut self) {
        let handle = self.0.take();
        if let Some(h) = handle {
            h.stop();
        }
    }
}

impl RabbitConnectionManager {
    pub fn create(config: &Config) -> impl Future<Item = Self, Error = Error> {
        let connection_timeout = Duration::from_secs(config.rabbit.connection_timeout_secs as u64);
        RabbitConnectionManager::extract_options_and_address(config)
            .into_future()
            .and_then(move |(options, address)| {
                let options_clone = options.clone();
                Timeout::new(
                    RabbitConnectionManager::establish_client(address, options).map(move |(client, hearbeat_handle)| {
                        RabbitConnectionManager {
                            client: Arc::new(Mutex::new(client)),
                            heartbeat_handle: Arc::new(Mutex::new(hearbeat_handle)),
                            connection_options: options_clone,
                            connection_timeout,
                            address,
                        }
                    }),
                    connection_timeout,
                )
                .map_err(move |e| {
                    let e: failure::Error = e.into_inner().map(|e| e.into()).unwrap_or(format_err!("Timeout error"));
                    ectx!(err e, ErrorSource::Timeout, ErrorContext::ConnectionTimeout, ErrorKind::Internal => connection_timeout)
                })
            })
    }

    fn extract_options_and_address(config: &Config) -> Result<(ConnectionOptions, SocketAddr), Error> {
        let url = config.rabbit.url.clone();
        let url_clone = config.rabbit.url.clone();
        let regex = Regex::new(r#"@([a-zA-Z0-9-_\.]*:[0-9]*)"#).unwrap();
        let address = regex
            .captures(&config.rabbit.url)
            .and_then(|captures| captures.get(1))
            .map(|mtch| mtch.as_str().to_string())
            .ok_or(ectx!(err ErrorContext::RabbitUrl, ErrorKind::Internal => config))
            .and_then(|host_and_port| -> Result<SocketAddr, Error> {
                let url_clone = url.clone();
                let mut addrs_iter = host_and_port
                    .to_socket_addrs()
                    .map_err(ectx!(try ErrorContext::RabbitUrl, ErrorKind::Internal => url))?;
                let addr = addrs_iter
                    .next()
                    .ok_or(ectx!(try err ErrorContext::RabbitUrl, ErrorKind::Internal => url_clone))?;
                Ok(addr)
            })?;
        let options = config
            .rabbit
            .url
            .parse::<ConnectionOptions>()
            .map_err(move |e| ectx!(try err format_err!("{}", e), ErrorContext::RabbitUrl, ErrorKind::Internal => url_clone))?;
        Ok((options, address))
    }

    fn establish_client(
        address: SocketAddr,
        options: ConnectionOptions,
    ) -> impl Future<Item = (Client<TcpStream>, RabbitHeartbeatHandle), Error = Error> {
        let address_clone2 = address.clone();
        let address_clone3 = address.clone();
        info!("Connecting to rabbit at `{}`", address);
        TcpStream::connect(&address)
            .map_err(ectx!(ErrorSource::Io, ErrorContext::TcpConnection, ErrorKind::Internal => address_clone3))
            .and_then(move |stream| {
                info!("TCP connection established. Handshake with rabbit...");
                Client::connect(stream, options)
                    .map_err(ectx!(ErrorSource::Io, ErrorContext::RabbitConnection, ErrorKind::Internal => address_clone2))
            })
            .and_then(move |(client, mut heartbeat)| {
                info!("Connected to rabbit");
                let handle = heartbeat.handle();
                let client_clone = client.clone();
                tokio::spawn(heartbeat.map_err(move |e| {
                    let e: Error = ectx!(err e, ErrorContext::Heartbeat, ErrorKind::Internal);
                    log_error(&e);
                    let mut transport = client_clone.transport.lock().unwrap();
                    transport.conn.state = ConnectionState::Error;
                }));
                handle
                    .ok_or(ectx!(err ErrorContext::HeartbeatHandle, ErrorKind::Internal))
                    .map(move |handle| (client, RabbitHeartbeatHandle::new(handle)))
            })
    }

    pub fn get_channel(&self) -> Result<Channel<TcpStream>, Error> {
        trace!("Creating rabbit channel...");
        let cli = self.client.lock().unwrap();
        let ch = cli
            .create_confirm_channel(ConfirmSelectOptions::default())
            .wait()
            .map_err(ectx!(try ErrorSource::Io, ErrorContext::RabbitChannel, ErrorKind::Internal))?;
        let _ = ch
            .basic_qos(BasicQosOptions {
                prefetch_count: CONSUMER_PREFETCH_COUNT,
                global: true,
                ..Default::default()
            })
            .wait()
            .map_err(ectx!(try ErrorSource::Io, ErrorContext::RabbitChannel, ErrorKind::Internal))?;
        trace!("Rabbit channel is created");
        Ok(ch)
    }
}
