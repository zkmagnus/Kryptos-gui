//! Desktop frontend for the Kryptos node control service.

pub(crate) mod rpc;
mod settings;
pub(crate) mod view;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use iced::{Subscription, Task, clipboard};
use serde_json::Value;

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
            node_epoch: 0,
            lookup_epoch: 0,
        }
    }
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let (settings, settings_status) = Settings::load();
        (
            Self {
                settings,
                settings_status,
                ..Self::default()
            },
            Task::none(),
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
        Task::perform(rpc::fetch_node(self.node_credentials()), move |result| {
            Message::NodeLoaded(epoch, result)
        })
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectTab(tab) => {
                self.tab = tab;
                return iced::widget::operation::snap_to(
                    "page-content",
                    iced::widget::scrollable::RelativeOffset::START,
                );
            }
            Message::NodeEndpointChanged(value) => {
                self.disconnect_node();
                // Credentials are bound to an endpoint; never carry a token to a new one.
                self.node_token.clear();
                self.settings.node_endpoint = value;
                self.settings_status = None;
            }
            Message::NodeTokenChanged(value) => {
                self.disconnect_node();
                self.node_token = value;
            }
            Message::ConnectNode => {
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
                    Ok(()) => "Control address saved. The token stays in memory only.".into(),
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
        if self.node_connected {
            iced::time::every(Duration::from_secs(15)).map(|_| Message::Refresh)
        } else {
            Subscription::none()
        }
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
}
