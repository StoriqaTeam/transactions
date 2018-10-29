use std::fmt::{self, Debug};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use failure;
use failure::Compat;
use futures::future::Either;
use lapin_async::connection::{ConnectingState, ConnectionState};
use lapin_futures::channel::{BasicQosOptions, Channel};
use lapin_futures::client::{Client, ConnectionOptions, HeartbeatHandle};
use prelude::*;
use r2d2::{CustomizeConnection, ManageConnection, Pool};
use regex::Regex;
use tokio::net::tcp::TcpStream;
use tokio::timer::timeout::Timeout;
use tokio_core::reactor::Core;

use super::error::*;
use config::Config;
use utils::log_error;

const CONSUMER_PREFETCH_COUNT: u16 = 1000;
pub type RabbitPool = Pool<RabbitConnectionManager>;

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

#[derive(Debug, Clone)]
pub struct ConnectionHooks;

impl CustomizeConnection<Channel<TcpStream>, Compat<Error>> for ConnectionHooks {
    #[allow(unused_variables)]
    fn on_acquire(&self, conn: &mut Channel<TcpStream>) -> Result<(), Compat<Error>> {
        trace!("Acquired rabbitmq channel");
        Ok(())
    }

    #[allow(unused_variables)]
    fn on_release(&self, conn: Channel<TcpStream>) {
        trace!("Released rabbitmq channel");
        thread::spawn(move || {
            // We ignore the error here, because the channel might be evicted from the pool
            // because it was already closed
            let _ = conn.close(0, "Released from pool").wait();
        });
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
                ).map_err(move |e| {
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

    fn repair_if_tcp_is_broken(&self) {
        if self.is_broken_conn() {
            let self_clone = self.clone();
            thread::spawn(move || {
                let _ = self_clone.repair_tcp().wait();
            });
        }
    }

    fn repair_tcp(&self) -> impl Future<Item = (), Error = Error> {
        {
            let cli = self.client.lock().unwrap();
            let mut transport = cli.transport.lock().unwrap();
            match transport.conn.state {
                ConnectionState::Connecting(_) => {
                    return Either::A(Err(ectx!(err ErrorContext::AlreadyConnecting, ErrorKind::Internal)).into_future())
                }
                _ => (),
            };
            info!("Resetting tcp connection to rabbit...");
            // setting state to connecting, so that we don't reenter here
            transport.conn.state = ConnectionState::Connecting(ConnectingState::Initial);
        };

        let self_client = self.client.clone();
        let self_client2 = self.client.clone();
        let self_hearbeat_handle = self.heartbeat_handle.clone();
        Either::B(
            RabbitConnectionManager::establish_client(self.address, self.connection_options.clone())
                .map(move |(client, hearbeat_handle)| {
                    {
                        let mut self_client = self_client.lock().unwrap();
                        *self_client = client;
                    }
                    {
                        let mut self_hearbeat_handle = self_hearbeat_handle.lock().unwrap();
                        *self_hearbeat_handle = hearbeat_handle;
                    }
                }).map_err(move |e| {
                    info!("Failed to reset tcp connection to rabbit.");
                    log_error(&e);
                    let cli = self_client2.lock().unwrap();
                    let mut transport = cli.transport.lock().unwrap();
                    transport.conn.state = ConnectionState::Error;
                    e
                }),
        )
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
            }).and_then(move |(client, mut heartbeat)| {
                info!("Connected to rabbit");
                let handle = heartbeat.handle();
                let client_clone = client.clone();
                thread::spawn(move || {
                    // Just using wait here doesn't work - says timer is stopped.
                    // On the other hand we need to isolate this to a new thread,
                    // i.e. need to use thread::spawn instead of tokio::spawn,
                    // because r2d2 uses it's own thread pool witout tokio =>
                    // good chance tokio::spawn won't work.
                    let mut core = Core::new().unwrap();
                    let f = heartbeat.map_err(move |e| {
                        let e: Error = ectx!(err e, ErrorContext::Heartbeat, ErrorKind::Internal);
                        log_error(&e);
                        let mut transport = client_clone.transport.lock().unwrap();
                        transport.conn.state = ConnectionState::Error;
                    });
                    let _ = core.run(f);
                });
                handle
                    .ok_or(ectx!(err ErrorContext::HeartbeatHandle, ErrorKind::Internal))
                    .map(move |handle| (client, RabbitHeartbeatHandle::new(handle)))
            })
    }

    fn is_broken_conn(&self) -> bool {
        let cli = self.client.lock().unwrap();
        let transport = cli.transport.lock().unwrap();
        match transport.conn.state {
            ConnectionState::Closing(_) | ConnectionState::Closed | ConnectionState::Error => true,
            _ => false,
        }
    }

    fn is_connecting_conn(&self) -> bool {
        let cli = self.client.lock().unwrap();
        let transport = cli.transport.lock().unwrap();
        match transport.conn.state {
            ConnectionState::Connecting(_) => true,
            _ => false,
        }
    }
}

impl ManageConnection for RabbitConnectionManager {
    type Connection = Channel<TcpStream>;
    type Error = Compat<Error>;
    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        trace!("Creating rabbit channel...");
        self.repair_if_tcp_is_broken();
        let cli = self.client.lock().unwrap();
        let ch = cli
            .create_channel()
            .wait()
            .map_err(ectx!(ErrorSource::Io, ErrorContext::RabbitChannel, ErrorKind::Internal))
            .map_err(|e: Error| e.compat())?;
        let _ = ch
            .basic_qos(BasicQosOptions {
                prefetch_count: CONSUMER_PREFETCH_COUNT,
                ..Default::default()
            }).wait()
            .map_err(ectx!(ErrorSource::Io, ErrorContext::RabbitChannel, ErrorKind::Internal))
            .map_err(|e: Error| e.compat())?;
        trace!("Rabbit channel is created");
        Ok(ch)
    }
    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        self.repair_if_tcp_is_broken();
        if self.is_broken_conn() {
            let e: Error = ectx!(err format_err!("Connection is broken"), ErrorKind::Internal);
            return Err(e.compat());
        }
        if self.is_connecting_conn() {
            let e: Error = ectx!(err format_err!("Connection is in process of connecting"), ErrorKind::Internal);
            return Err(e.compat());
        }
        if !conn.is_connected() {
            let e: Error = ectx!(err format_err!("Channel is not connected"), ErrorKind::Internal);
            return Err(e.compat());
        }
        trace!("Channel is ok");
        Ok(())
    }
    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        self.is_valid(conn).is_err()
    }
}
