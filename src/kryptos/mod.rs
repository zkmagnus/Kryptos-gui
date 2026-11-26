//! Desktop frontend for the Kryptos node control service.

mod embedded;
pub(crate) mod rpc;
mod settings;
pub(crate) mod view;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use iced::{Subscription, Task, clipboard};
use serde_json::Value;

use embedded::{EmbeddedNode, EmbeddedSnapshot, NodeOptions};
use rpc::{Credentials, LookupKind, NodeSnapshot};
use settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Tab {
    #[default]
    Overview,
    Network,
    Mining,
    Explorer,
    Wallet,
    Settings,
    About,
}

impl Tab {
    pub const ALL: [Self; 7] = [
        Self::Overview,
        Self::Network,
        Self::Mining,
        Self::Explorer,
        Self::Wallet,
        Self::Settings,
        Self::About,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Network => "Tor network",
            Self::Mining => "Mining",
            Self::Explorer => "Explorer",
            Self::Wallet => "Wallet",
            Self::Settings => "Settings",
            Self::About => "About",
        }
    }
}

// Input messages may contain credentials: never derive Debug for this enum.
#[derive(Clone)]
pub(crate) enum Message {
    SelectTab(Tab),
    NodeEndpointChanged(String),
    NodeTokenChanged(String),
    ConnectNode,
    DisconnectNode,
    UseEmbeddedNode(bool),
    NetworkChanged(String),
    PeersChanged(String),
    StartEmbeddedNode,
    StopEmbeddedNode,
    EmbeddedStarted(u64, Result<(), String>),
    EmbeddedStopped(u64, Result<(), String>),
    EmbeddedLoaded(u64, Result<EmbeddedSnapshot, String>),
    CloseRequested,
    Refresh,
    LookupKindChanged(LookupKind),
    QueryChanged(String),
    Search,
    CopyExplorerResult,
    SaveSettings,
    OpenProject,
    NodeLoaded(u64, Result<NodeSnapshot, String>),
    LookupLoaded(u64, u64, Result<Value, String>),
    LinkOpened(Result<(), String>),
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Kryptos GUI event")
    }
}

pub(crate) struct App {
    pub tab: Tab,
    pub settings: Settings,
    pub node_token: String,
    pub node: Option<NodeSnapshot>,
    pub node_connected: bool,
    pub node_busy: bool,
    pub node_error: Option<String>,
    pub lookup_kind: LookupKind,
    pub query: String,
    pub lookup_result: Option<Value>,
    pub lookup_busy: bool,
    pub lookup_error: Option<String>,
    pub settings_status: Option<String>,
    pub last_updated: Option<String>,
    pub embedded_active: bool,
    pub embedded_busy: bool,
    pub embedded_stopping: bool,
    pub closing: bool,
    pub tor_status: Option<String>,
    pub onion_address: Option<String>,
    embedded: Option<EmbeddedNode>,
    lifecycle_epoch: u64,
    node_epoch: u64,
    lookup_epoch: u64,
}

impl Default for App {
    fn default() -> Self {
        Self {
            tab: Tab::Overview,
            settings: Settings::default(),
            node_token: String::new(),
            node: None,
            node_connected: false,
            node_busy: false,
            node_error: None,
            lookup_kind: LookupKind::Block,
            query: String::new(),
            lookup_result: None,
            lookup_busy: false,
            lookup_error: None,
            settings_status: None,
            last_updated: None,
            embedded_active: false,
            embedded_busy: false,
            embedded_stopping: false,
            closing: false,
            tor_status: None,
            onion_address: None,
            embedded: None,
            lifecycle_epoch: 0,
            node_epoch: 0,
            lookup_epoch: 0,
        }
    }
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let (settings, settings_status) = Settings::load();
        let mut app = Self {
            settings,
            settings_status,
            embedded: Some(EmbeddedNode::new()),
            ..Self::default()
        };
        let task = if app.settings.embedded_node {
            app.start_embedded()
        } else {
            Task::none()
        };
        (app, task)
    }

    fn start_embedded(&mut self) -> Task<Message> {
        if self.embedded_active
            || self.embedded_busy
            || self.closing
            || !self.settings.embedded_node
        {
            return Task::none();
        }
        let options = self.settings.validate().and_then(|()| {
            Ok(NodeOptions {
                network: self.settings.network.clone(),
                datadir: self.settings.chain_directory()?,
                peers: self.settings.peer_list(),
            })
        });
        let options = match options {
            Ok(options) => options,
            Err(error) => {
                self.node_error = Some(error);
                self.tab = Tab::Settings;
                return Task::none();
            }
        };
        let Some(embedded) = self.embedded.clone() else {
            return Task::none();
        };
        self.disconnect_node();
        self.node_token.clear();
        // Ownership begins before startup: even a timed-out start must be stopped.
        self.embedded_active = true;
        self.embedded_busy = true;
        self.embedded_stopping = false;
        self.tor_status = Some("Preparing cryptographic verifier and opening blockchain".into());
        self.lifecycle_epoch = self.lifecycle_epoch.wrapping_add(1);
        let epoch = self.lifecycle_epoch;
        let startup = embedded.start(options);
        Task::perform(startup, move |result| {
            Message::EmbeddedStarted(epoch, result)
        })
    }

    fn stop_embedded(&mut self) -> Task<Message> {
        self.disconnect_node();
        self.embedded_busy = true;
        self.embedded_stopping = true;
        self.tor_status = Some("Stopping node".into());
        self.lifecycle_epoch = self.lifecycle_epoch.wrapping_add(1);
        let epoch = self.lifecycle_epoch;
        // Invalidate queued startup before Iced polls the shutdown task.
        let shutdown = self.embedded.as_ref().map(EmbeddedNode::stop);
        Task::perform(
            async move {
                match shutdown {
                    Some(shutdown) => shutdown.await,
                    None => Ok(()),
                }
            },
            move |result| Message::EmbeddedStopped(epoch, result),
        )
    }

    fn node_credentials(&self) -> Credentials {
        Credentials {
            endpoint: self.settings.node_endpoint.clone(),
            token: self.node_token.clone(),
        }
    }

    fn invalidate_lookup(&mut self) {
        self.lookup_epoch = self.lookup_epoch.wrapping_add(1);
        self.lookup_busy = false;
        self.lookup_result = None;
        self.lookup_error = None;
    }

    fn disconnect_node(&mut self) {
        self.node_epoch = self.node_epoch.wrapping_add(1);
        self.node_connected = false;
        self.node_busy = false;
        self.node = None;
        self.node_error = None;
        self.last_updated = None;
        self.invalidate_lookup();
    }

    fn refresh_node(&mut self) -> Task<Message> {
        if !self.node_connected || self.node_busy {
            return Task::none();
        }
        self.node_busy = true;
        let epoch = self.node_epoch;
        if self.settings.embedded_node {
            let Some(embedded) = self.embedded.clone() else {
                self.node_busy = false;
                return Task::none();
            };
            return Task::perform(async move { embedded.snapshot().await }, move |result| {
                Message::EmbeddedLoaded(epoch, result)
            });
        }
        Task::perform(rpc::fetch_node(self.node_credentials()), move |result| {
            Message::NodeLoaded(epoch, result)
        })
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Pending I/O may complete during shutdown; it must not reconnect the UI.
        if self.closing
            && !matches!(
                message,
                Message::EmbeddedStopped(..) | Message::CloseRequested
            )
        {
            return Task::none();
        }
        match message {
            Message::CloseRequested => {
                if !self.closing {
                    self.closing = true;
                    return self.stop_embedded();
                }
            }
            Message::UseEmbeddedNode(enabled) => {
                if !self.embedded_active && !self.embedded_busy {
                    self.disconnect_node();
                    self.node_token.clear();
                    self.settings.embedded_node = enabled;
                    self.tor_status = None;
                    self.onion_address = None;
                }
            }
            Message::NetworkChanged(network) => {
                if !self.embedded_active && !self.embedded_busy {
                    self.settings.network = network;
                    self.settings_status = None;
                }
            }
            Message::PeersChanged(peers) => {
                if !self.embedded_active && !self.embedded_busy {
                    self.settings.peers = peers;
                    self.settings_status = None;
                }
            }
            Message::StartEmbeddedNode => return self.start_embedded(),
            Message::StopEmbeddedNode => {
                if self.embedded_active && !self.embedded_stopping {
                    return self.stop_embedded();
                }
            }
            Message::EmbeddedStarted(epoch, result) => {
                if epoch != self.lifecycle_epoch {
                    return Task::none();
                }
                self.embedded_busy = false;
                match result {
                    Ok(()) => {
                        self.node_connected = true;
                        return self.refresh_node();
                    }
                    Err(error) => {
                        self.node_error = Some(error);
                        self.tor_status = Some("Node startup failed; stop before retrying".into());
                        self.tab = Tab::Settings;
                    }
                }
            }
            Message::EmbeddedStopped(epoch, result) => {
                if epoch != self.lifecycle_epoch {
                    return Task::none();
                }
                self.embedded_busy = false;
                self.embedded_stopping = false;
                match result {
                    Ok(()) => {
                        self.embedded_active = false;
                        self.tor_status = None;
                        self.onion_address = None;
                        if self.closing {
                            return iced::exit();
                        }
                    }
                    Err(error) => {
                        self.closing = false;
                        self.node_error = Some(format!(
                            "Shutdown incomplete: {error}. Retry stopping the node."
                        ));
                        self.tab = Tab::Settings;
                    }
                }
            }
            Message::EmbeddedLoaded(epoch, result) => {
                if epoch != self.node_epoch {
                    return Task::none();
                }
                self.node_busy = false;
                match result {
                    Ok(snapshot) => {
                        self.node = Some(snapshot.node);
                        self.tor_status = Some(snapshot.tor_status);
                        self.onion_address = snapshot.onion_address;
                        self.node_error = None;
                        self.last_updated = Some(update_time());
                    }
                    Err(error) => {
                        // Keep polling for recovery, but do not display stale
                        // chain data as a healthy, connected embedded node.
                        self.node = None;
                        self.last_updated = None;
                        self.node_error = Some(error);
                    }
                }
            }
            Message::SelectTab(tab) => {
                self.tab = tab;
                return iced::widget::operation::snap_to(
                    "page-content",
                    iced::widget::scrollable::RelativeOffset::START,
                );
            }
            Message::NodeEndpointChanged(value) => {
                if self.settings.embedded_node {
                    return Task::none();
                }
                self.disconnect_node();
                // Credentials are bound to an endpoint; never carry a token to a new one.
                self.node_token.clear();
                self.settings.node_endpoint = value;
                self.settings_status = None;
            }
            Message::NodeTokenChanged(value) => {
                if self.settings.embedded_node {
                    return Task::none();
                }
                self.disconnect_node();
                self.node_token = value;
            }
            Message::ConnectNode => {
                if self.settings.embedded_node {
                    return Task::none();
                }
                self.disconnect_node();
                match rpc::Client::new(&self.node_credentials()) {
                    Ok(_) => {
                        self.node_connected = true;
                        return self.refresh_node();
                    }
                    Err(error) => self.node_error = Some(error),
                }
            }
            Message::DisconnectNode => {
                if self.settings.embedded_node {
                    return Task::none();
                }
                self.disconnect_node();
                self.node_token.clear();
            }
            Message::Refresh => return self.refresh_node(),
            Message::NodeLoaded(epoch, result) => {
                if epoch != self.node_epoch {
                    return Task::none();
                }
                self.node_busy = false;
                match result {
                    Ok(snapshot) => {
                        self.node = Some(snapshot);
                        self.node_error = None;
                        self.last_updated = Some(update_time());
                    }
                    Err(error) => {
                        self.disconnect_node();
                        self.node_error = Some(error);
                    }
                }
            }
            Message::LookupKindChanged(kind) => {
                self.lookup_kind = kind;
                self.invalidate_lookup();
            }
            Message::QueryChanged(value) => {
                self.query = value;
                self.invalidate_lookup();
            }
            Message::Search => {
                if !self.node_connected || self.node.is_none() {
                    self.lookup_error = Some("Connect to the node in Settings first.".into());
                } else if !self.lookup_busy {
                    self.invalidate_lookup();
                    self.lookup_busy = true;
                    let epoch = self.node_epoch;
                    let lookup_epoch = self.lookup_epoch;
                    if self.settings.embedded_node {
                        let Some(embedded) = self.embedded.clone() else {
                            self.lookup_busy = false;
                            return Task::none();
                        };
                        let kind = self.lookup_kind;
                        let query = self.query.clone();
                        return Task::perform(
                            async move { embedded.lookup(kind, query).await },
                            move |result| Message::LookupLoaded(epoch, lookup_epoch, result),
                        );
                    }
                    return Task::perform(
                        rpc::lookup(
                            self.node_credentials(),
                            self.lookup_kind,
                            self.query.clone(),
                        ),
                        move |result| Message::LookupLoaded(epoch, lookup_epoch, result),
                    );
                }
            }
            Message::LookupLoaded(epoch, lookup_epoch, result) => {
                if epoch != self.node_epoch || lookup_epoch != self.lookup_epoch {
                    return Task::none();
                }
                self.lookup_busy = false;
                match result {
                    Ok(value) => self.lookup_result = Some(value),
                    Err(error) => self.lookup_error = Some(error),
                }
            }
            Message::CopyExplorerResult => {
                if let Some(result) = &self.lookup_result
                    && let Ok(json) = serde_json::to_string_pretty(result)
                {
                    return clipboard::write(json);
                }
            }
            Message::SaveSettings => {
                self.settings_status = Some(match self.settings.save() {
                    Ok(()) => "Node preferences saved.".into(),
                    Err(error) => format!("Could not save settings: {error}"),
                });
            }
            Message::OpenProject => {
                return Task::perform(
                    async {
                        tokio::task::spawn_blocking(|| open::that(env!("CARGO_PKG_REPOSITORY")))
                            .await
                            .map_err(|_| "Could not open browser.".to_string())?
                            .map_err(|_| "Could not open browser.".to_string())
                    },
                    Message::LinkOpened,
                );
            }
            Message::LinkOpened(result) => {
                if let Err(error) = result {
                    self.settings_status = Some(error);
                }
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let refresh = if self.node_connected && !self.closing {
            iced::time::every(Duration::from_secs(if self.settings.embedded_node {
                3
            } else {
                15
            }))
            .map(|_| Message::Refresh)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            refresh,
            iced::window::close_requests().map(|_| Message::CloseRequested),
        ])
    }
}

fn update_time() -> String {
    // An explicit UTC label avoids silently using a different local timezone.
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!(
        "{:02}:{:02}:{:02} UTC",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_endpoint_clears_credentials_and_rejects_inflight_reply() {
        let mut app = App {
            settings: Settings {
                embedded_node: false,
                ..Settings::default()
            },
            node_token: "secret".into(),
            node_connected: true,
            ..App::default()
        };
        let old_epoch = app.node_epoch;
        let _ = app.update(Message::NodeEndpointChanged("127.0.0.1:18543".into()));
        let _ = app.update(Message::NodeLoaded(
            old_epoch,
            Err("obsolete failure".into()),
        ));
        assert!(app.node_token.is_empty());
        assert!(!app.node_connected);
        assert!(app.node_error.is_none());
    }

    #[test]
    fn editing_query_discards_out_of_order_search_result() {
        let mut app = App::default();
        let old_lookup = app.lookup_epoch;
        let _ = app.update(Message::QueryChanged("42".into()));
        let _ = app.update(Message::LookupLoaded(
            app.node_epoch,
            old_lookup,
            Ok(serde_json::json!({"height": 1})),
        ));
        assert!(app.lookup_result.is_none());
    }

    #[test]
    fn debug_does_not_reveal_credentials() {
        assert!(
            !format!("{:?}", Message::NodeTokenChanged("sensitive-token".into()))
                .contains("sensitive-token")
        );
    }

    #[test]
    fn stopping_discards_late_start_and_snapshot_callbacks() {
        let mut app = App {
            embedded_active: true,
            node_connected: true,
            ..App::default()
        };
        let old_start = app.lifecycle_epoch;
        let old_snapshot = app.node_epoch;
        let _ = app.update(Message::StopEmbeddedNode);
        let _ = app.update(Message::EmbeddedStarted(old_start, Ok(())));
        let _ = app.update(Message::EmbeddedLoaded(old_snapshot, Err("stale".into())));
        assert!(!app.node_connected);
        assert!(app.node_error.is_none());
        assert!(app.embedded_busy);
    }

    #[test]
    fn owned_node_blocks_network_and_mode_changes_until_stopped() {
        let mut app = App {
            embedded_active: true,
            ..App::default()
        };
        let _ = app.update(Message::NetworkChanged("test".into()));
        let _ = app.update(Message::UseEmbeddedNode(false));
        assert_eq!(app.settings.network, "main");
        assert!(app.settings.embedded_node);
        let _ = app.update(Message::EmbeddedStopped(app.lifecycle_epoch, Ok(())));
        let _ = app.update(Message::NetworkChanged("test".into()));
        assert_eq!(app.settings.network, "test");
    }

    #[test]
    fn failed_shutdown_keeps_window_and_node_ownership_for_retry() {
        let mut app = App {
            closing: true,
            embedded_active: true,
            ..App::default()
        };
        let _ = app.update(Message::EmbeddedStopped(
            app.lifecycle_epoch,
            Err("timeout".into()),
        ));
        assert!(!app.closing);
        assert!(app.embedded_active);
        assert!(
            app.node_error
                .as_ref()
                .unwrap()
                .contains("Shutdown incomplete")
        );
        assert_eq!(app.tab, Tab::Settings);
    }

    #[test]
    fn failed_embedded_refresh_hides_stale_chain_data_but_keeps_retrying() {
        let mut app = App {
            embedded_active: true,
            node_connected: true,
            last_updated: Some("old".into()),
            node: Some(NodeSnapshot {
                height: 1,
                hash: "00".repeat(32),
                peers: 1,
                mempool: 0,
                mined: 0,
                mined_in_chain: 0,
                syncing: false,
            }),
            ..App::default()
        };
        let _ = app.update(Message::EmbeddedLoaded(
            app.node_epoch,
            Err("stopped".into()),
        ));
        assert!(app.node.is_none());
        assert!(app.last_updated.is_none());
        assert!(app.node_connected);
        assert!(app.embedded_active);
    }

    #[test]
    fn stop_can_cancel_startup_before_it_completes() {
        let mut app = App {
            embedded_active: true,
            embedded_busy: true,
            ..App::default()
        };
        let _ = app.update(Message::StopEmbeddedNode);
        assert!(app.embedded_stopping);
        assert_eq!(app.lifecycle_epoch, 1);
    }
}
