use reqwest::{Client, Error};
use serde::Deserialize;
use std::collections::HashMap;
use std::iter::Cycle;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::vec::IntoIter;
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum ServerDiscoveryError {
    #[error("Failed send discovery request: {0:#}")]
    Network(reqwest::Error),
    #[error("steam returned an empty server list")]
    NoServers,
}

impl From<reqwest::Error> for ServerDiscoveryError {
    fn from(value: Error) -> Self {
        ServerDiscoveryError::Network(value)
    }
}

#[derive(Default, Clone, Debug)]
pub struct DiscoverOptions {
    web_client: Option<Client>,
    /// Explicit cell ID
    cell: Option<u8>,
}

impl DiscoverOptions {
    pub fn with_web_client(self, web_client: Client) -> Self {
        DiscoverOptions {
            web_client: Some(web_client),
            ..self
        }
    }

    pub fn with_cell(self, cell: u8) -> Self {
        DiscoverOptions {
            cell: Some(cell),
            ..self
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackedServer<T: Clone> {
    inner: T,
    connection_failures: Arc<AtomicU32>,
}

impl<T: Clone> TrackedServer<T> {
    pub fn server(&self) -> &T {
        &self.inner
    }

    pub fn track_connection_failure(&self) -> u32 {
        self.connection_failures.fetch_add(1, Ordering::Relaxed)
    }

    pub fn connection_failures(&self) -> u32 {
        self.connection_failures.load(Ordering::Relaxed)
    }
}

impl<T: Clone> From<T> for TrackedServer<T> {
    fn from(server: T) -> Self {
        Self {
            inner: server,
            connection_failures: Arc::new(AtomicU32::new(0)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerList {
    servers: Arc<Mutex<Cycle<IntoIter<TrackedServer<Server>>>>>,
}

impl ServerList {
    pub async fn discover() -> Result<ServerList, ServerDiscoveryError> {
        Self::discover_with(DiscoverOptions::default()).await
    }

    pub async fn discover_with(
        options: DiscoverOptions,
    ) -> Result<ServerList, ServerDiscoveryError> {
        let client = options.web_client.unwrap_or_default();

        let mut query = HashMap::new();
        query.insert("cmtype".to_string(), "websockets".to_string());
        query.insert("realm".to_string(), "steamglobal".to_string());

        if let Some(cell_id) = options.cell {
            query.insert("cellid".to_string(), cell_id.to_string());
        }

        let response: ServerListResponse = client
            .get("https://api.steampowered.com/ISteamDirectory/GetCMListForConnect/v1")
            .query(&query)
            .send()
            .await?
            .json()
            .await?;
        if response.response.server_list.is_empty() {
            return Err(ServerDiscoveryError::NoServers);
        }
        Ok(response.into())
    }

    /// Pick a WebSocket server from the server list, rotating them in a round-robin way for reconnects.
    ///
    /// # Returns
    /// A WebSocket URL to connect to, if the server list contains any servers.
    pub fn pick_ws(&self) -> TrackedServer<Server> {
        // SAFETY:
        // `lock` cannot panic as we cannot lock again within the same thread.
        // `unwrap` is safe as `discover_with` already checks for servers being present.
        let srv = self.servers.lock().unwrap().next().unwrap();
        debug!(addr = ?srv, "picked websocket server from list");
        srv
    }
}

impl From<ServerListResponse> for ServerList {
    fn from(value: ServerListResponse) -> Self {
        let mut servers = value.response.server_list;

        // Sort servers by load as reported by Steam
        servers.sort_by(|a, b| a.load.cmp(&b.load));

        let servers = servers
            .into_iter()
            .map(TrackedServer::from)
            .collect::<Vec<TrackedServer<Server>>>();

        ServerList {
            servers: Arc::new(Mutex::new(servers.into_iter().cycle())),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ServerListResponse {
    response: ServerListResponseInner,
}

#[derive(Debug, Deserialize)]
struct ServerListResponseInner {
    #[serde(rename = "serverlist")]
    server_list: Vec<Server>,
}

#[allow(unused)]
#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    endpoint: String,
    legacy_endpoint: String,
    r#type: String,
    dc: String,
    realm: String,
    load: u32,
    wtd_load: f32,
}

impl Server {
    pub fn url(&self) -> String {
        format!("wss://{}/cmsocket/", self.endpoint)
    }
}
