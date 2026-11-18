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
use serde_json::Value;

use super::App;
use super::Message;
use super::Tab;
use super::rpc::LookupKind;
use super::rpc::format_bus;

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
        "Rhombus",
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

fn field(value: &Value, key: &str) -> String {
    scalar(&value[key])
}

fn scalar(value: &Value) -> String {
    match value {
        Value::Null => "—".into(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => if *value { "Yes" } else { "No" }.into(),
        other => other.to_string(),
    }
}

fn uptime(value: &Value) -> String {
    match value.as_u64() {
        Some(seconds) if seconds >= 86_400 => {
            format!(
                "{}d {}h {}m",
                seconds / 86_400,
                seconds % 86_400 / 3_600,
                seconds % 3_600 / 60
            )
        }
        Some(seconds) if seconds >= 3_600 => {
            format!("{}h {}m", seconds / 3_600, seconds % 3_600 / 60)
        }
        Some(seconds) => format!("{}m {}s", seconds / 60, seconds % 60),
        None => "—".into(),
    }
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
                    text("RHOMBUS").size(22).font(BOLD),
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
            text("$BUS  /  SHIELDED")
                .font(MONO)
                .wrapping(Wrapping::WordOrGlyph)
                .size(11)
                .color(ACCENT),
            muted("Tor-only peer network"),
            muted(format!("Rhombus {}", env!("CARGO_PKG_VERSION"))),
        ]
        .spacing(12),
    )
    .padding(22)
    .width(230)
    .height(Length::Fill);

    let network = app.node.as_ref().map(|node| field(&node.chain, "network"));
    let connection = if app.node_busy && app.node.is_none() {
        "◌  CONNECTING".into()
    } else if app.node_connected && app.node.is_some() {
        format!(
            "●  {}",
            network.as_deref().unwrap_or("Connected").to_uppercase()
        )
    } else {
        "○  NOT CONNECTED".into()
    };
    let (title, subtitle) = match app.tab {
        Tab::Overview => (
            "Your node, at a glance",
            "Live insight into your Rhombus node.",
        ),
        Tab::Network => (
            "Connected through Tor",
            "Onion peers and authenticated outbound connections.",
        ),
        Tab::Mining => (
            "Mining & readiness",
            "Network readiness and RandomX proof of work.",
        ),
        Tab::Explorer => (
            "Explore the shielded chain",
            "Inspect public blocks and transaction data.",
        ),
        Tab::Wallet => (
            "Your shielded wallet",
            "Balances and receiving address from your local wallet daemon.",
        ),
        Tab::Settings => (
            "Connect your workspace",
            "Local connections to your Rhombus node and wallet.",
        ),
        Tab::About => (
            "Rhombus",
            "A desktop companion for a shielded monetary blockchain.",
        ),
    };
    let refresh = button(
        text(if app.node_busy || app.wallet_busy {
            "Refreshing…"
        } else {
            "Refresh"
        })
        .size(12),
    )
    .on_press_maybe(
        (!(app.node_busy || app.wallet_busy) && (app.node_connected || app.wallet_connected))
            .then_some(Message::Refresh),
    )
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
            "Start Rhombus, then connect with its local HTTP endpoint and node token.",
        );
    };
    let syncing = node.chain["syncing"].as_bool();
    let sync_label = match syncing {
        Some(true) => "Synchronizing",
        Some(false) => "Synchronized",
        None => "Unknown",
    };
    let readiness = match node.metrics["mining_ready"].as_bool() {
        Some(true) => "Ready for mining".into(),
        Some(false) => format!("Waiting: {}", field(&node.metrics, "mining_waits")),
        None => "Readiness unavailable".into(),
    };
    let cards = row![
        metric("CHAIN HEIGHT", field(&node.chain, "height"), sync_label),
        metric(
            "CONNECTED PEERS",
            field(&node.network, "connections"),
            "Over the Tor network"
        ),
        metric(
            "MEMPOOL",
            field(&node.mempool_info, "size"),
            "Pending transactions"
        ),
        metric(
            "UPTIME",
            uptime(&node.metrics["uptime_seconds"]),
            "Current node session"
        ),
    ]
    .spacing(14);
    let chain = panel(
        column![
            section("Chain state", "The latest snapshot reported by your node."),
            horizontal_rule(1),
            key_value("Network", field(&node.chain, "network")),
            key_value("Synchronization", sync_label.into()),
            key_value(
                "Target block interval",
                format!("{} seconds", field(&node.chain, "block_interval"))
            ),
            key_value("Active branch", field(&node.chain, "branch")),
            key_value("Mining readiness", readiness),
            muted("BEST BLOCK HASH"),
            text(field(&node.chain, "best_block_hash"))
                .font(MONO)
                .wrapping(Wrapping::WordOrGlyph)
                .size(12),
        ]
        .spacing(16),
    );
    let mut mempool = column![
        section("Mempool", "Transactions awaiting inclusion in a block."),
        muted(format!(
            "{} / {} transactions",
            field(&node.mempool_info, "size"),
            field(&node.mempool_info, "capacity")
        )),
    ]
    .spacing(12);
    if node.mempool.is_empty() {
        mempool = mempool.push(muted("The mempool is empty."));
    } else {
        for txid in node.mempool.iter().take(12) {
            mempool = mempool.push(
                button(
                    text(txid)
                        .font(MONO)
                        .wrapping(Wrapping::WordOrGlyph)
                        .size(12),
                )
                .on_press(Message::InspectTransaction(txid.clone()))
                .padding([10, 12])
                .width(Length::Fill)
                .style(secondary_style),
            );
        }
        if node.mempool.len() > 12 {
            mempool = mempool.push(muted(format!(
                "Showing 12 of {} pending transactions.",
                node.mempool.len()
            )));
        }
        mempool = mempool.push(muted("Select a transaction to inspect it in Explorer."));
    }
    column![cards, chain, panel(mempool)].spacing(18).into()
}

fn network_view(app: &App) -> Element<'_, Message> {
    let Some(node) = &app.node else {
        return empty(
            "Connect to inspect your peers",
            "Rhombus uses embedded Arti for onion connections. Connect your node in Settings.",
        );
    };
    let mut peers = column![section(
        "Peer connections",
        "Inbound addresses are advertised claims; outbound onion destinations are authenticated."
    )]
    .spacing(16);
    if node.peers.is_empty() {
        peers = peers.push(muted("No peers reported. Rhombus has no built-in public bootstrap peers; configure onion peers in the node."));
    }
    for peer in &node.peers {
        let address = peer["addr"]
            .as_str()
            .or_else(|| peer["target"].as_str())
            .unwrap_or("Address not advertised");
        let ready = peer["ready"].as_bool() == Some(true);
        let rtt = peer["rtt_micros"]
            .as_u64()
            .map(|micros| format!("{} ms RTT", micros / 1_000))
            .unwrap_or_else(|| "RTT unavailable".into());
        peers = peers.push(
            container(
                column![
                    row![
                        text(format!(
                            "{}  ·  Peer {}",
                            field(peer, "direction").to_uppercase(),
                            field(peer, "id")
                        ))
                        .size(11)
                        .font(BOLD)
                        .color(ACCENT),
                        Space::new().width(Length::Fill),
                        text(if ready { "CONNECTED" } else { "HANDSHAKING" })
                            .size(11)
                            .color(if ready { ACCENT } else { AMBER }),
                    ]
                    .spacing(12),
                    text(address)
                        .font(MONO)
                        .wrapping(Wrapping::WordOrGlyph)
                        .size(12),
                    muted(format!(
                        "Height {}   ·   {}   ·   Received {} bytes   ·   Sent {} bytes",
                        field(peer, "best_height"),
                        rtt,
                        field(peer, "received_bytes"),
                        field(peer, "sent_bytes")
                    )),
                ]
                .spacing(12),
            )
            .padding(16)
            .width(Length::Fill)
            .style(inset),
        );
    }
    column![
        row![
            metric(
                "INBOUND",
                field(&node.network, "connections_in"),
                "Accepted onion streams"
            ),
            metric(
                "OUTBOUND",
                field(&node.network, "connections_out"),
                "Authenticated destinations"
            ),
            metric(
                "HANDSHAKING",
                field(&node.network, "handshaking"),
                format!("Protocol {}", field(&node.network, "protocol_version"))
            ),
        ]
        .spacing(14),
        panel(peers),
    ]
    .spacing(18)
    .into()
}

fn mining(app: &App) -> Element<'_, Message> {
    let Some(node) = &app.node else {
        return empty(
            "Connect to view mining readiness",
            "Your node reports readiness, synchronization, and mining outcomes through its local RPC.",
        );
    };
    let ready = node.metrics["mining_ready"].as_bool();
    let readiness = match ready {
        Some(true) => "Ready".into(),
        Some(false) => field(&node.metrics, "mining_waits"),
        None => "Unavailable".into(),
    };
    let subsidy = node.mining["subsidy"]
        .as_str()
        .map(|raw| format!("{} BUS", format_bus(raw)))
        .unwrap_or_else(|| "—".into());
    column![
        panel(column![
            eyebrow("MINING READINESS"),
            text(readiness).size(26).font(BOLD).color(if ready == Some(true) { ACCENT } else { AMBER }),
            muted("Online mining waits for Tor, an authenticated outbound peer, completed sync, and five seconds of stable readiness."),
            muted("Readiness does not mean a miner is running. Configure mining in the Rhombus node."),
        ].spacing(12)),
        row![
            metric("MINED IN CHAIN", field(&node.metrics, "blocks_mined_in_chain"), "Locally mined, accepted blocks"),
            metric("MINED TOTAL", field(&node.metrics, "blocks_mined"), "All local mining outcomes"),
            metric("STALE BLOCKS", field(&node.metrics, "blocks_mined_stale"), "Local stale outcomes"),
        ].spacing(14),
        panel(column![
            section("Next block", "Parameters reported by the Rhombus node."),
            horizontal_rule(1),
            key_value("Next height", field(&node.mining, "next_height")),
            key_value("Block subsidy", subsidy),
            key_value("Target", field(&node.mining, "next_target")),
            key_value("Network work per second", field(&node.mining, "work_per_second")),
            key_value("Target interval", format!("{} seconds", field(&node.mining, "block_interval"))),
            key_value("Rejected local blocks", field(&node.metrics, "blocks_mined_rejected")),
            key_value("Blocked local attempts", field(&node.metrics, "blocks_mined_blocked")),
            key_value("Pending compact blocks", field(&node.metrics, "pending_compact_blocks")),
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
            "Block height or 64-character block hash"
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
            muted("Recipients and transferred values are shielded. Public data includes commitments, nullifiers, and public fees."),
        ].spacing(16)),
    ].spacing(18);
    if !app.node_connected {
        content = content.push(empty(
            "Connect your node to explore",
            "Block and transaction lookups use your local Rhombus node.",
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
            muted("Search a block by height or hash, or inspect a transaction by its hash. Address activity is visible only to a wallet with the corresponding viewing key."),
        ].spacing(12)));
    }
    content.into()
}

fn wallet(app: &App) -> Element<'_, Message> {
    let mut content = column![].spacing(18);
    if let Some(error) = &app.wallet_error {
        content = content.push(notice(format!("Wallet connection: {error}"), true));
    }
    let Some(wallet) = &app.wallet else {
        return content.push(empty("Connect your shielded wallet", "Run the Rhombus wallet daemon, then connect its local endpoint and wallet token in Settings.")).into();
    };
    let amount = |key| {
        wallet.balance[key]
            .as_str()
            .map(|raw| format!("{} BUS", format_bus(raw)))
            .unwrap_or_else(|| "—".into())
    };
    content = content.push(
        row![
            metric("SPENDABLE", amount("spendable"), "Available balance"),
            metric("PENDING", amount("pending"), "Awaiting confirmations"),
            metric("LOCKED", amount("locked"), "Reserved for operations"),
        ]
        .spacing(14),
    );
    content = content.push(panel(
        column![
            section(
                "Wallet state",
                "Wallet data is separate from the node's public chain data."
            ),
            horizontal_rule(1),
            key_value("Network", field(&wallet.info, "network")),
            key_value("Total balance", amount("total")),
            key_value("Scanned height", field(&wallet.info, "scanned_height")),
            key_value("Node height", field(&wallet.info, "node_height")),
            key_value("Synchronized", field(&wallet.info, "synced")),
            key_value("Unspent coins", field(&wallet.info, "unspent")),
            key_value(
                "Pending operations",
                field(&wallet.info, "pending_operations")
            ),
        ]
        .spacing(16),
    ));
    if let Some(error) = wallet.info["last_error"].as_str()
        && !error.is_empty()
    {
        content = content.push(notice(format!("Wallet scan: {error}"), true));
    }
    if let Some(address) = wallet.info["receive_address"].as_str() {
        content = content.push(panel(column![
            row![section("Receive $BUS", "Use the current address for a new payment request."), Space::new().width(Length::Fill),
                button(text("Copy address").size(13).font(BOLD)).on_press(Message::CopyAddress).padding([12, 16]).style(action_style),
            ].spacing(16).align_y(Vertical::Center),
            container(text(address).font(MONO).wrapping(Wrapping::WordOrGlyph).size(12)).padding(16).width(Length::Fill).style(inset),
            muted("Rhombus rotates this address after receiving a payment or preparing an outgoing transfer. Refresh before sharing it again."),
        ].spacing(16)));
    } else {
        content = content.push(notice(if wallet.info["address_rotation_exhausted"].as_bool() == Some(true) {
            "The wallet has exhausted its receiving address range. Create a fresh account through Rhombus to receive new payments."
        } else { "The wallet has not supplied a receiving address." }, false));
    }
    content.push(muted("This desktop view displays balances and the current receiving address. Sending and wallet recovery are available through Rhombus.")).into()
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
            "Rhombus node",
            "Metrics, Tor peers, mining readiness, and the block explorer."
        ),
        muted("LOCAL HTTP ENDPOINT"),
        text_input("http://127.0.0.1:18443", &app.settings.node_endpoint)
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
    let mut wallet = column![
        section(
            "Rhombus wallet",
            "A separate connection for shielded balances and receiving."
        ),
        muted("LOCAL WALLET HTTP ENDPOINT"),
        text_input("http://127.0.0.1:18447", &app.settings.wallet_endpoint)
            .on_input(Message::WalletEndpointChanged)
            .size(14)
            .padding(12),
        muted("WALLET TOKEN · wallet-rpc.token"),
        text_input("Paste your wallet token", &app.wallet_token)
            .secure(true)
            .on_input(Message::WalletTokenChanged)
            .size(14)
            .padding(12),
        row![
            button(
                text(if app.wallet_busy {
                    "Connecting…"
                } else {
                    "Connect wallet"
                })
                .size(13)
                .font(BOLD)
            )
            .on_press_maybe((!app.wallet_busy).then_some(Message::ConnectWallet))
            .padding([12, 18])
            .style(action_style),
            button(text("Disconnect").size(13))
                .on_press_maybe(
                    (app.wallet_connected || app.wallet_busy).then_some(Message::DisconnectWallet)
                )
                .padding([12, 18])
                .style(secondary_style),
        ]
        .spacing(10),
    ]
    .spacing(12);
    if let Some(error) = &app.wallet_error {
        wallet = wallet.push(notice(error, true));
    }
    if app.wallet_busy && app.wallet.is_none() {
        wallet = wallet.push(muted("Connecting to wallet…"));
    } else if app.wallet_connected && app.wallet.is_some() {
        wallet = wallet.push(text("● Wallet connected").color(ACCENT).size(13));
    }
    let mut content = column![
        panel(node),
        panel(wallet),
        panel(column![
            section("Connection preferences", "Save the endpoint addresses for your next session."),
            muted("Tokens stay in memory for this session. Node and wallet use separate tokens. Connections accept only HTTP on 127.0.0.1; peer traffic remains inside Rhombus's embedded Tor transport."),
            button(text("Save endpoint addresses").size(13).font(BOLD)).on_press(Message::SaveSettings).padding([12, 18]).style(secondary_style),
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
            text("◇  RHOMBUS").size(36).font(BOLD).color(ACCENT),
            text("Randomized Hidden-Output Monetary Blockchain Using Shielded-transfers.").size(19),
            muted("Rhombus brings node observability and a shielded wallet overview to the desktop. It connects to the existing Rhombus node and wallet daemons through authenticated local RPC."),
            button(text("View Rhombus on GitHub").size(13).font(BOLD)).on_press(Message::OpenProject).padding([12, 18]).style(action_style),
        ].spacing(20)),
        row![
            metric("PRIVATE TRANSFERS", "Halo2".into(), "Zero-knowledge proofs"),
            metric("COIN ENCRYPTION", "ML-KEM".into(), "Encrypted coin payloads"),
            metric("PEER TRANSPORT", "Tor".into(), "Embedded Arti"),
        ].spacing(14),
        panel(column![
            section("Experimental software", "Rhombus parameters and protocol remain provisional."),
            muted("ML-KEM protects coin payload encryption. It does not make the proof system or spending authorization fully post-quantum."),
            muted("Recipients and coin values are shielded; transaction fees and verification data remain public. Your wallet's viewing keys determine which incoming coins it can discover."),
            horizontal_rule(1),
            muted("Built on the Bonsai desktop interface. Rhombus protocol and daemon development: zkmagnus/Rhombus."),
            muted(format!("Rhombus {} · Rust / Iced", env!("CARGO_PKG_VERSION"))),
        ].spacing(16)),
    ].spacing(18).into()
}
