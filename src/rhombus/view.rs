use iced::Background;
use iced::Border;
use iced::Color;
use iced::Element;
use iced::Font;
use iced::Length;
use iced::Theme;
use iced::alignment::Vertical;
use iced::font::Weight;
use iced::widget::Column;
use iced::widget::Container;
use iced::widget::Space;
use iced::widget::button;
use iced::widget::column;
use iced::widget::container;
use iced::widget::row;
use iced::widget::rule::horizontal as horizontal_rule;
use iced::widget::scrollable;
use iced::widget::text;
use iced::widget::text::Wrapping;
use iced::widget::text_input;

use super::App;
use super::Message;
use super::Tab;
use super::rpc::LookupKind;

const BG: Color = Color::from_rgb8(16, 20, 17);
const PANEL: Color = Color::from_rgb8(24, 31, 26);
const INSET: Color = Color::from_rgb8(18, 24, 20);
const LINE: Color = Color::from_rgb8(48, 59, 50);
const TEXT: Color = Color::from_rgb8(232, 238, 232);
const MUTED: Color = Color::from_rgb8(158, 172, 160);
const ACCENT: Color = Color::from_rgb8(157, 214, 165);
const AMBER: Color = Color::from_rgb8(241, 199, 121);
const ERROR: Color = Color::from_rgb8(255, 155, 145);
const BOLD: Font = Font {
    weight: Weight::Bold,
    ..Font::DEFAULT
};
const MONO: Font = Font::MONOSPACE;

pub(crate) fn theme(_: &App) -> Theme {
    Theme::custom(
        "Kryptos",
        iced::theme::Palette {
            background: BG,
            text: TEXT,
            primary: ACCENT,
            success: ACCENT,
            danger: ERROR,
            warning: AMBER,
        },
    )
}

fn surface(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(PANEL)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 14.0.into(),
        },
        ..Default::default()
    }
}

fn inset(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(INSET)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 9.0.into(),
        },
        ..Default::default()
    }
}

fn action_style(_: &Theme, status: button::Status) -> button::Style {
    let disabled = matches!(status, button::Status::Disabled);
    let hovered = matches!(status, button::Status::Hovered);
    button::Style {
        background: Some(Background::Color(if disabled {
            LINE
        } else if hovered {
            Color::from_rgb8(179, 231, 186)
        } else {
            ACCENT
        })),
        text_color: if disabled { MUTED } else { BG },
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn secondary_style(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered) {
                LINE
            } else {
                INSET
            },
        )),
        text_color: if matches!(status, button::Status::Disabled) {
            MUTED.scale_alpha(0.5)
        } else {
            TEXT
        },
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

fn panel<'a>(content: impl Into<Element<'a, Message>>) -> Container<'a, Message> {
    container(content)
        .padding(22)
        .width(Length::Fill)
        .style(surface)
}

fn eyebrow(label: &str) -> iced::widget::Text<'_> {
    text(label).size(11).color(ACCENT).font(BOLD)
}

fn muted<'a>(label: impl Into<String>) -> iced::widget::Text<'a> {
    text(label.into()).size(13).color(MUTED)
}

fn section<'a>(title: &'a str, detail: &'a str) -> Column<'a, Message> {
    column![text(title).size(20).font(BOLD), muted(detail)].spacing(5)
}

fn metric<'a>(label: &'a str, value: String, detail: impl Into<String>) -> Element<'a, Message> {
    container(
        column![
            text(label).size(11).color(MUTED).font(BOLD),
            text(value)
                .wrapping(Wrapping::WordOrGlyph)
                .size(28)
                .color(ACCENT)
                .font(BOLD),
            muted(detail),
        ]
        .spacing(10),
    )
    .padding(19)
    .width(Length::FillPortion(1))
    .style(surface)
    .into()
}

fn key_value<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    row![
        text(label)
            .size(13)
            .color(MUTED)
            .width(Length::FillPortion(2)),
        text(value)
            .size(13)
            .font(MONO)
            .wrapping(Wrapping::WordOrGlyph)
            .width(Length::FillPortion(3)),
    ]
    .spacing(20)
    .into()
}

fn notice<'a>(message: impl Into<String>, error: bool) -> Element<'a, Message> {
    container(
        text(message.into())
            .size(13)
            .color(if error { ERROR } else { AMBER }),
    )
    .padding(14)
    .width(Length::Fill)
    .style(inset)
    .into()
}

fn empty<'a>(title: &'a str, description: &'a str) -> Element<'a, Message> {
    panel(
        column![
            text("◇").size(44).color(ACCENT),
            text(title).size(23).font(BOLD),
            muted(description),
            Space::new().height(8),
            button(text("Open connection settings").font(BOLD).size(13))
                .on_press(Message::SelectTab(Tab::Settings))
                .style(action_style)
                .padding([12, 16]),
        ]
        .spacing(12),
    )
    .into()
}

pub(crate) fn view(app: &App) -> Element<'_, Message> {
    let mut navigation = column![].spacing(7);
    for (index, tab) in Tab::ALL.into_iter().enumerate() {
        let selected = app.tab == tab;
        navigation = navigation.push(
            button(
                row![
                    text(format!("{:02}", index + 1))
                        .size(12)
                        .font(MONO)
                        .wrapping(Wrapping::WordOrGlyph),
                    text(tab.label())
                        .size(14)
                        .font(if selected { BOLD } else { Font::DEFAULT }),
                ]
                .spacing(16)
                .align_y(Vertical::Center),
            )
            .on_press(Message::SelectTab(tab))
            .width(Length::Fill)
            .padding([14, 16])
            .style(move |_, status| button::Style {
                background: Some(Background::Color(if selected {
                    Color::from_rgb8(39, 57, 43)
                } else if matches!(status, button::Status::Hovered) {
                    PANEL
                } else {
                    BG
                })),
                text_color: if selected { ACCENT } else { MUTED },
                border: Border {
                    color: if selected {
                        Color::from_rgb8(63, 87, 68)
                    } else {
                        BG
                    },
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            }),
        );
    }

    let sidebar = container(
        column![
            row![
                text("◇").size(46).color(ACCENT),
                column![
                    text("KRYPTOS").size(22).font(BOLD),
                    text("SHIELDED BLOCKCHAIN").size(10).color(ACCENT),
                ]
                .spacing(3),
            ]
            .spacing(12)
            .align_y(Vertical::Center),
            Space::new().height(28),
            navigation,
            Space::new().height(Length::Fill),
            horizontal_rule(1),
            text("$K  /  SHIELDED")
                .font(MONO)
                .wrapping(Wrapping::WordOrGlyph)
                .size(11)
                .color(ACCENT),
            muted("Tor-only peer network"),
            muted(format!("Kryptos {}", env!("CARGO_PKG_VERSION"))),
        ]
        .spacing(12),
    )
    .padding(22)
    .width(230)
    .height(Length::Fill);

    let connection = if app.node_busy && app.node.is_none() {
        "◌  CONNECTING"
    } else if app.node_connected && app.node.is_some() {
        "●  NODE CONNECTED"
    } else {
        "○  NOT CONNECTED"
    };
    let (title, subtitle) = match app.tab {
        Tab::Overview => (
            "Your node, at a glance",
            "Live insight into your Kryptos node.",
        ),
        Tab::Network => (
            "Connected through Tor",
            "Peer count reported by the node control service.",
        ),
        Tab::Mining => (
            "Mining & readiness",
            "Local mined-block counts reported by the node.",
        ),
        Tab::Explorer => (
            "Explore the shielded chain",
            "Look up block hashes and transaction status.",
        ),
        Tab::Wallet => (
            "Your shielded wallet",
            "Wallet commands remain available in the Kryptos CLI.",
        ),
        Tab::Settings => (
            "Connect your workspace",
            "Connect to the local Kryptos control socket.",
        ),
        Tab::About => (
            "Kryptos",
            "A desktop companion for a shielded monetary blockchain.",
        ),
    };
    let refresh = button(
        text(if app.node_busy {
            "Refreshing…"
        } else {
            "Refresh"
        })
        .size(12),
    )
    .on_press_maybe((!app.node_busy && app.node_connected).then_some(Message::Refresh))
    .padding([10, 14])
    .style(secondary_style);
    let header = row![
        column![
            eyebrow(app.tab.label()),
            text(title).size(30).font(BOLD),
            muted(subtitle),
        ]
        .spacing(8)
        .width(Length::Fill),
        column![
            text(connection)
                .size(11)
                .color(if app.node_connected && app.node.is_some() {
                    ACCENT
                } else {
                    MUTED
                })
                .font(BOLD),
            refresh,
        ]
        .spacing(12)
        .align_x(iced::Alignment::End),
    ]
    .spacing(18)
    .align_y(Vertical::Center);

    let content = match app.tab {
        Tab::Overview => overview(app),
        Tab::Network => network_view(app),
        Tab::Mining => mining(app),
        Tab::Explorer => explorer(app),
        Tab::Wallet => wallet(app),
        Tab::Settings => settings(app),
        Tab::About => about(),
    };
    let mut body = column![header].spacing(24);
    if let Some(error) = &app.node_error
        && !matches!(app.tab, Tab::Settings | Tab::Wallet | Tab::About)
    {
        body = body.push(notice(format!("Node connection: {error}"), true));
    }
    body = body.push(content);
    if let Some(updated) = &app.last_updated
        && matches!(app.tab, Tab::Overview | Tab::Network | Tab::Mining)
    {
        body = body.push(muted(format!(
            "Last node update {updated} · Refreshes every 15 seconds"
        )));
    }
    row![
        sidebar,
        container(scrollable(container(body).padding(28).width(Length::Fill)).id("page-content"))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                border: Border {
                    color: LINE,
                    width: 1.0,
                    ..Default::default()
                },
                ..Default::default()
            }),
    ]
    .height(Length::Fill)
    .into()
}

fn overview(app: &App) -> Element<'_, Message> {
    let Some(node) = &app.node else {
        return empty(
            "Your node is one connection away",
            "Start Kryptos, then connect with its local control address and node token.",
        );
    };
    let sync_label = if node.syncing {
        "Synchronizing"
    } else {
        "Synchronized"
    };
    let cards = row![
        metric("CHAIN HEIGHT", node.height.to_string(), sync_label),
        metric(
            "CONNECTED PEERS",
            node.peers.to_string(),
            "Over the Tor network"
        ),
        metric("MEMPOOL", node.mempool.to_string(), "Pending transactions"),
        metric(
            "MINED IN CHAIN",
            node.mined_in_chain.to_string(),
            "Blocks mined by this node"
        ),
    ]
    .spacing(14);
    let chain = panel(
        column![
            section(
                "Chain state",
                "The latest status from the node control service."
            ),
            horizontal_rule(1),
            key_value("Height", node.height.to_string()),
            key_value("Synchronization", sync_label.to_owned()),
            key_value("Connected peers", node.peers.to_string()),
            key_value("Transactions in mempool", node.mempool.to_string()),
            muted("BEST BLOCK HASH"),
            text(node.hash.clone())
                .font(MONO)
                .wrapping(Wrapping::WordOrGlyph)
                .size(12),
        ]
        .spacing(16),
    );
    let mempool = panel(
        column![
            section("Mempool", "Transactions awaiting inclusion in a block."),
            muted(format!("{} transactions currently pooled.", node.mempool)),
            muted("The control service reports the count, but does not expose a transaction list."),
        ]
        .spacing(12),
    );
    column![cards, chain, mempool].spacing(18).into()
}

fn network_view(app: &App) -> Element<'_, Message> {
    let Some(node) = &app.node else {
        return empty(
            "Connect to inspect your peers",
            "Kryptos uses embedded Arti for onion connections. Connect your node in Settings.",
        );
    };
    let peer_summary = if node.peers == 0 {
        "No authenticated peers are connected. Configure onion peers on the node.".to_owned()
    } else {
        format!("{} authenticated peers are connected over Tor.", node.peers)
    };
    column![
        row![
            metric(
                "AUTHENTICATED PEERS",
                node.peers.to_string(),
                "Current connected peer count"
            ),
            metric("CHAIN HEIGHT", node.height.to_string(), "Latest node tip"),
            metric(
                "SYNC",
                if node.syncing { "Active" } else { "Idle" }.into(),
                "Synchronization state"
            ),
        ]
        .spacing(14),
        panel(
            column![
                section(
                    "Peer connections",
                    "Detailed peer addresses and RTT are not part of the current control protocol."
                ),
                muted(peer_summary),
            ]
            .spacing(12)
        ),
    ]
    .spacing(18)
    .into()
}

fn mining(app: &App) -> Element<'_, Message> {
    let Some(node) = &app.node else {
        return empty(
            "Connect to view node status",
            "The node control service reports its height, peer count, and local mined-block count.",
        );
    };
    column![
        panel(column![
            eyebrow("NODE MINING"),
            text("Mining readiness is not exposed by control").size(22).font(BOLD).color(AMBER),
            muted("Online mining waits for Tor, an authenticated outbound peer, completed sync, and a stable ready period."),
            muted("Configure mining with the Kryptos CLI. This view shows the block counts included in node status."),
        ].spacing(12)),
        row![
            metric("MINED IN CHAIN", node.mined_in_chain.to_string(), "Locally mined, accepted blocks"),
            metric("MINED TOTAL", node.mined.to_string(), "Blocks mined since startup"),
            metric("CHAIN HEIGHT", node.height.to_string(), if node.syncing { "Synchronizing" } else { "Synchronized" }),
        ].spacing(14),
        panel(column![
            section("Current node status", "The line control protocol exposes a compact status snapshot."),
            horizontal_rule(1),
            key_value("Connected peers", node.peers.to_string()),
            key_value("Mempool transactions", node.mempool.to_string()),
            key_value("Synchronization", if node.syncing { "Active" } else { "Idle" }.into()),
        ].spacing(16)),
    ].spacing(18).into()
}

fn explorer(app: &App) -> Element<'_, Message> {
    let block = app.lookup_kind == LookupKind::Block;
    let kinds = row![
        button(text("Blocks").size(13))
            .on_press(Message::LookupKindChanged(LookupKind::Block))
            .padding([10, 18])
            .style(if block { action_style } else { secondary_style }),
        button(text("Transactions").size(13))
            .on_press(Message::LookupKindChanged(LookupKind::Transaction))
            .padding([10, 18])
            .style(if block { secondary_style } else { action_style }),
    ]
    .spacing(10);
    let can_search = app.node_connected && !app.lookup_busy && !app.query.trim().is_empty();
    let input = text_input(
        if block {
            "Block height"
        } else {
            "64-character transaction hash"
        },
        &app.query,
    )
    .on_input(Message::QueryChanged)
    .on_submit_maybe(can_search.then_some(Message::Search))
    .size(14)
    .padding(13);
    let mut content = column![
        panel(column![
            kinds,
            row![input, button(text(if app.lookup_busy { "Searching…" } else { "Search" }).size(13).font(BOLD))
                .on_press_maybe(can_search.then_some(Message::Search)).padding([14, 20]).style(action_style)].spacing(12),
            muted("Block lookup accepts a height. Transaction lookup shows pool or confirmation status; the control service does not return transaction contents."),
        ].spacing(16)),
    ].spacing(18);
    if !app.node_connected {
        content = content.push(empty(
            "Connect your node to explore",
            "Block and transaction lookups use your local Kryptos node.",
        ));
    }
    if let Some(error) = &app.lookup_error {
        content = content.push(notice(error, true));
    }
    if let Some(result) = &app.lookup_result {
        let json = serde_json::to_string_pretty(result)
            .unwrap_or_else(|_| "Could not display response".into());
        let truncated = json.chars().count() > 32_000;
        let display = if truncated {
            json.chars().take(32_000).collect()
        } else {
            json
        };
        let mut details = column![
            row![
                section(
                    "Public chain data",
                    "Response from your authenticated local node."
                ),
                Space::new().width(Length::Fill),
                button(text("Copy JSON").size(12))
                    .on_press(Message::CopyExplorerResult)
                    .padding([10, 14])
                    .style(secondary_style),
            ]
            .spacing(16)
            .align_y(Vertical::Center),
            container(
                text(display)
                    .size(12)
                    .font(MONO)
                    .wrapping(Wrapping::WordOrGlyph)
            )
            .padding(16)
            .width(Length::Fill)
            .style(inset),
        ]
        .spacing(16);
        if truncated {
            details = details.push(muted(
                "Preview limited to 32,000 characters. Copy JSON includes the full response.",
            ));
        }
        content = content.push(panel(details));
    } else if app.node_connected && app.lookup_error.is_none() {
        content = content.push(panel(column![
            text("A private chain. Public verification.").size(22).font(BOLD),
            muted("Search a block by height or check a transaction ID. Shielded recipient activity is visible only to the wallet with its viewing key."),
        ].spacing(12)));
    }
    content.into()
}

fn wallet(_app: &App) -> Element<'_, Message> {
    panel(column![
        section("Wallet tools", "Wallet databases are opened directly by the Kryptos CLI."),
        horizontal_rule(1),
        muted("The removed wallet daemon is not needed for wallet commands."),
        container(
            text("kryptos balance --network main --wallet wallet.redb --rpc 127.0.0.1:18440 --rpc-token-file /path/to/rpc.token\nkryptos address --network main --wallet wallet.redb")
                .font(MONO)
                .wrapping(Wrapping::WordOrGlyph)
                .size(12),
        )
        .padding(16)
        .width(Length::Fill)
        .style(inset),
        muted("Use the same network and node token when connecting through the control socket. Keep the wallet file private and do not open it in two processes at once."),
    ].spacing(16)).into()
}

fn settings(app: &App) -> Element<'_, Message> {
    let node_button = button(
        text(if app.node_busy {
            "Connecting…"
        } else {
            "Connect node"
        })
        .size(13)
        .font(BOLD),
    )
    .on_press_maybe((!app.node_busy).then_some(Message::ConnectNode))
    .padding([12, 18])
    .style(action_style);
    let node_disconnect = button(text("Disconnect").size(13))
        .on_press_maybe((app.node_connected || app.node_busy).then_some(Message::DisconnectNode))
        .padding([12, 18])
        .style(secondary_style);
    let mut node = column![
        section(
            "Kryptos node",
            "Read status, block hashes by height, and transaction status over the control protocol."
        ),
        muted("LOCAL CONTROL ADDRESS"),
        text_input("127.0.0.1:18440", &app.settings.node_endpoint)
            .on_input(Message::NodeEndpointChanged)
            .size(14)
            .padding(12),
        muted("NODE TOKEN · rpc.token"),
        text_input("Paste your node token", &app.node_token)
            .secure(true)
            .on_input(Message::NodeTokenChanged)
            .size(14)
            .padding(12),
        row![node_button, node_disconnect].spacing(10),
    ]
    .spacing(12);
    if let Some(error) = &app.node_error {
        node = node.push(notice(error, true));
    }
    if app.node_busy && app.node.is_none() {
        node = node.push(muted("Connecting to node…"));
    } else if app.node_connected && app.node.is_some() {
        node = node.push(text("● Node connected").color(ACCENT).size(13));
    }
    let mut content = column![
        panel(node),
        panel(column![
            section("Connection preferences", "Save the local control address for your next session."),
            muted("The node token stays in memory for this session. The GUI accepts only an IPv4 loopback address; peer traffic remains inside Kryptos's embedded Tor transport."),
            button(text("Save control address").size(13).font(BOLD)).on_press(Message::SaveSettings).padding([12, 18]).style(secondary_style),
        ].spacing(14)),
    ].spacing(18);
    if let Some(status) = &app.settings_status {
        content = content.push(muted(status));
    }
    content.into()
}

fn about() -> Element<'static, Message> {
    column![
        panel(column![
            text("◇  KRYPTOS").size(36).font(BOLD).color(ACCENT),
            text("Desktop dashboard for the Kryptos blockchain.").size(19),
            muted("Kryptos brings a basic node status and chain lookup view to the desktop. It connects through the node's authenticated local control service."),
            button(text("View Kryptos on GitHub").size(13).font(BOLD)).on_press(Message::OpenProject).padding([12, 18]).style(action_style),
        ].spacing(20)),
        row![
            metric("PRIVATE TRANSFERS", "Halo2".into(), "Zero-knowledge proofs"),
            metric("COIN ENCRYPTION", "ML-KEM".into(), "Encrypted coin payloads"),
            metric("PEER TRANSPORT", "Tor".into(), "Embedded Arti"),
        ].spacing(14),
        panel(column![
            section("Experimental software", "Kryptos parameters and protocol remain provisional."),
            muted("ML-KEM protects coin payload encryption. It does not make the proof system or spending authorization fully post-quantum."),
            muted("Recipients and coin values are shielded; transaction fees and verification data remain public. Your wallet's viewing keys determine which incoming coins it can discover."),
            horizontal_rule(1),
            muted("Built on the Bonsai desktop interface. Kryptos protocol and daemon development: zkmagnus/Kryptos."),
            muted(format!("Kryptos {} · Rust / Iced", env!("CARGO_PKG_VERSION"))),
        ].spacing(16)),
    ].spacing(18).into()
}
