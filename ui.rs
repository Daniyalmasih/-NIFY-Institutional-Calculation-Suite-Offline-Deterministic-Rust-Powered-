use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, BorderType,
    Paragraph, Row, Table, Cell, Sparkline, Clear,
};
use ratatui::layout::{Layout, Constraint, Direction};

use crate::app::{App, SignalType};
use crate::theme;

// ═══════════════════════════════════════════════════════
// NIFY UI — Bloomberg Terminal Layout
// Premium lock overlay on Signal + Trades + Liquidations
// ═══════════════════════════════════════════════════════

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),   // Header + sparkline
            Constraint::Min(15),     // Body
            Constraint::Length(13),  // Signal panel
            Constraint::Length(3),   // Footer
        ])
        .split(area);

    draw_header(frame, app, main[0]);
    draw_body(frame, app, main[1]);
    draw_signal_panel(frame, app, main[2]);
    draw_footer(frame, app, main[3]);
}

// ═══════════════════════════════════════════════════════
// HEADER
// ═══════════════════════════════════════════════════════

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" NIFY TERMINAL v1.0 — BTCUSDT PERPETUAL ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Color::White))
        .style(Style::default().bg(theme::BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let h_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(65),
            Constraint::Percentage(35),
        ])
        .split(inner);

    let pct  = app.ticker.price_change_pct;
    let sign = if pct >= 0.0 { "+" } else { "" };
    let pc   = theme::change_color(pct);

    let price_str = if app.ticker.last_price > 0.0 {
        format!("{:.2}", app.ticker.last_price)
    } else {
        "CONNECTING...".to_string()
    };

    let h_str = if app.ticker.high_24h > 0.0 {
        format!("{:.2}", app.ticker.high_24h)
    } else { "---".to_string() };

    let l_str = if app.ticker.low_24h > 0.0 {
        format!("{:.2}", app.ticker.low_24h)
    } else { "---".to_string() };

    let vol = theme::fmt_usd_short(app.ticker.quote_volume_24h);

    let conn = format!(
        "T:{} B:{} L:{} K:{}",
        dot(app.ws_trade_ok),
        dot(app.ws_depth_ok),
        dot(app.ws_liq_ok),
        dot(app.ws_ticker_ok),
    );

    let line1 = Line::from(vec![
        Span::styled(
            " BTCUSDT-PERP ",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{} ", price_str),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}{:.2}% ", sign, pct),
            Style::default().fg(pc).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(theme::BORDER)),
        Span::styled(
            format!("H:{} L:{} ", h_str, l_str),
            Style::default().fg(theme::TEXT_SECONDARY),
        ),
    ]);

    let line2 = Line::from(vec![
        Span::styled(" Vol:", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("{} ", vol),
            Style::default().fg(theme::TEXT_SECONDARY),
        ),
        Span::styled("│ Sprd:", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("{:.1}bp ", app.spread_bps()),
            Style::default().fg(theme::TEXT_SECONDARY),
        ),
        Span::styled("│ ", Style::default().fg(theme::BORDER)),
        Span::styled(conn, Style::default().fg(theme::TEXT_DIM)),
    ]);

    frame.render_widget(
        Paragraph::new(vec![line1, line2])
            .style(Style::default().bg(theme::BG)),
        h_cols[0],
    );

    // Sparkline
    let spark_data: Vec<u64> = app
        .price_history
        .iter()
        .map(|&p| p as u64)
        .collect();

    if !spark_data.is_empty() {
        let spark_color = if app.ticker.price_change_pct >= 0.0 {
            theme::BID_GREEN
        } else {
            theme::ASK_RED
        };

        let spark = Sparkline::default()
            .block(
                Block::default()
                    .title(" PRICE TREND ")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::LEFT)
                    .border_style(Style::default().fg(theme::BORDER)),
            )
            .data(&spark_data)
            .style(Style::default().fg(spark_color).bg(theme::BG));

        frame.render_widget(spark, h_cols[1]);
    }
}

fn dot(ok: bool) -> &'static str {
    if ok { "●" } else { "○" }
}

// ═══════════════════════════════════════════════════════
// BODY — 3 columns
// ═══════════════════════════════════════════════════════

fn draw_body(frame: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(38),
            Constraint::Percentage(32),
        ])
        .split(area);

    draw_orderbook(frame, app, cols[0]);
    draw_trades_locked(frame, app, cols[1]);      // 🔒 LOCKED
    draw_liquidations_locked(frame, app, cols[2]); // 🔒 LOCKED
}

// ═══════════════════════════════════════════════════════
// ORDER BOOK — Free (no lock)
// ═══════════════════════════════════════════════════════

fn draw_orderbook(frame: &mut Frame, app: &App, area: Rect) {
    let block = theme::nify_block("ORDER BOOK");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 5 { return; }

    let avail     = inner.height as usize;
    let side_rows = ((avail.saturating_sub(3)) / 2).min(12);

    let max_depth: f64 = app
        .asks.iter().chain(app.bids.iter())
        .map(|l| l.value_usd)
        .fold(1.0_f64, f64::max);

    let mut rows: Vec<Row> = Vec::new();

    rows.push(Row::new(vec![
        Cell::from("PRICE").style(
            Style::default().fg(theme::TEXT_DIM).add_modifier(Modifier::BOLD)
        ),
        Cell::from("QTY").style(
            Style::default().fg(theme::TEXT_DIM).add_modifier(Modifier::BOLD)
        ),
        Cell::from("DEPTH").style(
            Style::default().fg(theme::TEXT_DIM).add_modifier(Modifier::BOLD)
        ),
    ]));

    // ASKS — highest at top, best ask nearest spread
    let ask_slice: Vec<_> = app.asks.iter().take(side_rows).collect();
    for level in ask_slice.iter().rev() {
        let bar = theme::depth_bar(level.value_usd, max_depth, 6);
        rows.push(Row::new(vec![
            Cell::from(format!("{:.2}", level.price)).style(theme::ask_style()),
            Cell::from(format!("{:.3}", level.qty)).style(theme::ask_style()),
            Cell::from(format!("{} {}", theme::fmt_usd_short(level.value_usd), bar))
                .style(theme::ask_style()),
        ]));
    }

    // Spread
    let spread_dollar = if !app.bids.is_empty() && !app.asks.is_empty() {
        app.asks[0].price - app.bids[0].price
    } else { 0.0 };

    rows.push(Row::new(vec![
        Cell::from(format!("── ${:.2} SPREAD", spread_dollar))
            .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Cell::from(""),
        Cell::from(""),
    ]));

    // BIDS — best bid first (GREEN)
    for level in app.bids.iter().take(side_rows) {
        let bar = theme::depth_bar(level.value_usd, max_depth, 6);
        rows.push(Row::new(vec![
            Cell::from(format!("{:.2}", level.price)).style(theme::bid_style()),
            Cell::from(format!("{:.3}", level.qty)).style(theme::bid_style()),
            Cell::from(format!("{} {}", theme::fmt_usd_short(level.value_usd), bar))
                .style(theme::bid_style()),
        ]));
    }

    // Imbalance
    let imb = app.book_imbalance();
    let (imb_label, imb_c) = if imb > 0.60 {
        (format!("▲ BID {:.0}%", imb * 100.0), theme::BID_GREEN)
    } else if imb < 0.40 {
        (format!("▼ ASK {:.0}%", (1.0 - imb) * 100.0), theme::ASK_RED)
    } else {
        (format!("● BAL {:.0}/{:.0}", imb*100.0, (1.0-imb)*100.0), theme::TEXT_DIM)
    };

    rows.push(Row::new(vec![
        Cell::from(imb_label)
            .style(Style::default().fg(imb_c).add_modifier(Modifier::BOLD)),
        Cell::from(""),
        Cell::from(""),
    ]));

    frame.render_widget(
        Table::new(rows, [
            Constraint::Percentage(36),
            Constraint::Percentage(24),
            Constraint::Percentage(40),
        ]).style(Style::default().bg(theme::BG)),
        inner,
    );
}

// ═══════════════════════════════════════════════════════
// TRADES TAPE — 🔒 LOCKED PREMIUM
// ═══════════════════════════════════════════════════════

fn draw_trades_locked(frame: &mut Frame, app: &App, area: Rect) {
    // Step 1: Draw blurred/dim real data behind
    draw_trades_blurred(frame, app, area);

    // Step 2: Draw lock overlay on top
    draw_lock_overlay(
        frame,
        area,
        "TRADES TAPE",
        "🔒 PREMIUM",
        &[
            "Real-time trade flow",
            "Whale detection ($100K+)",
            "Buy/Sell pressure",
            "Volume analysis",
        ],
        Color::Yellow,
    );
}

fn draw_trades_blurred(frame: &mut Frame, app: &App, area: Rect) {
    // Draw dim version of trades (blurred effect)
    let block = Block::default()
        .title(" TRADES TAPE ")
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default().bg(theme::BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Show dim/blurred data rows
    let mut rows: Vec<Row> = Vec::new();

    // Fake blurred header
    rows.push(Row::new(vec![
        Cell::from("████").style(Style::default().fg(Color::DarkGray)),
        Cell::from("████████").style(Style::default().fg(Color::DarkGray)),
        Cell::from("██████").style(Style::default().fg(Color::DarkGray)),
        Cell::from("████████").style(Style::default().fg(Color::DarkGray)),
    ]));

    // Generate fake blurred rows
    let fake_rows = [
        ("████", "██████.██", "██████", "$███,███"),
        ("████", "██████.██", "██████", "$███,███"),
        ("████", "██████.██", "██████", "$███,███"),
        ("████", "██████.██", "██████", "$██,███"),
        ("████", "██████.██", "██████", "$██,███"),
        ("████", "██████.██", "██████", "$███,███"),
        ("████", "██████.██", "██████", "$██,███"),
        ("████", "██████.██", "██████", "$███,███"),
    ];

    for (s, p, q, v) in fake_rows.iter() {
        rows.push(Row::new(vec![
            Cell::from(*s).style(Style::default().fg(Color::DarkGray)),
            Cell::from(*p).style(Style::default().fg(Color::DarkGray)),
            Cell::from(*q).style(Style::default().fg(Color::DarkGray)),
            Cell::from(*v).style(Style::default().fg(Color::DarkGray)),
        ]));
    }

    frame.render_widget(
        Table::new(rows, [
            Constraint::Percentage(16),
            Constraint::Percentage(28),
            Constraint::Percentage(20),
            Constraint::Percentage(36),
        ]).style(Style::default().bg(theme::BG)),
        inner,
    );
}

// ═══════════════════════════════════════════════════════
// LIQUIDATIONS — 🔒 LOCKED PREMIUM
// ═══════════════════════════════════════════════════════

fn draw_liquidations_locked(
    frame: &mut Frame,
    app: &App,
    area: Rect,
) {
    // Step 1: Draw dim data behind
    draw_liquidations_blurred(frame, area);

    // Step 2: Lock overlay
    draw_lock_overlay(
        frame,
        area,
        "LIQUIDATIONS",
        "🔒 PREMIUM",
        &[
            "Real-time liq events",
            "Long & Short tracking",
            "Multi-coin coverage",
            "Whale liq alerts $500K+",
        ],
        Color::Magenta,
    );
}

fn draw_liquidations_blurred(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(format!(" LIQS  L:██  S:██  $███K "))
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default().bg(theme::BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let fake = [
        ("███ ☠ LONG ", "██████.█", "$███,███"),
        ("███ ☠ SHORT", "██████.█", "$██,███"),
        ("███ ☠ LONG ", "██████.█", "$████,███"),
        ("███ ☠ LONG ", "██████.█", "$██,███"),
        ("███ ☠ SHORT", "██████.█", "$███,███"),
        ("███ ☠ LONG ", "██████.█", "$██,███"),
    ];

    let mut rows: Vec<Row> = Vec::new();

    rows.push(Row::new(vec![
        Cell::from("████████").style(Style::default().fg(Color::DarkGray)),
        Cell::from("████████").style(Style::default().fg(Color::DarkGray)),
        Cell::from("████████").style(Style::default().fg(Color::DarkGray)),
    ]));

    for (t, p, v) in fake.iter() {
        rows.push(Row::new(vec![
            Cell::from(*t).style(Style::default().fg(Color::DarkGray)),
            Cell::from(*p).style(Style::default().fg(Color::DarkGray)),
            Cell::from(*v).style(Style::default().fg(Color::DarkGray)),
        ]));
    }

    frame.render_widget(
        Table::new(rows, [
            Constraint::Percentage(42),
            Constraint::Percentage(28),
            Constraint::Percentage(30),
        ]).style(Style::default().bg(theme::BG)),
        inner,
    );
}

// ═══════════════════════════════════════════════════════
// SIGNAL PANEL — 🔒 LOCKED PREMIUM
// ═══════════════════════════════════════════════════════

fn draw_signal_panel(frame: &mut Frame, app: &App, area: Rect) {
    // Draw blurred signal behind
    draw_signal_blurred(frame, app, area);

    // Lock overlay on LEFT half only (signal card)
    // Right half (analytics) stays visible — free teaser
    let block = theme::nify_block("SIGNAL  ──  AUTO ANALYSIS ENGINE");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(46),
            Constraint::Percentage(54),
        ])
        .split(inner);

    // LEFT: Signal locked
    draw_lock_overlay(
        frame,
        halves[0],
        "SIGNAL ENGINE",
        "🔒 PREMIUM",
        &[
            "LONG / SHORT signal",
            "Entry price",
            "Stop Loss (SL)",
            "Take Profit 1/2/3",
            "Risk:Reward ratio",
            "Confidence score",
        ],
        Color::Cyan,
    );

    // RIGHT: Analytics FREE (teaser to entice upgrade)
    draw_analytics_free(frame, app, halves[1]);
}

fn draw_signal_blurred(frame: &mut Frame, _app: &App, area: Rect) {
    let block = Block::default()
        .title(" SIGNAL  ──  AUTO ANALYSIS ENGINE ")
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default().bg(theme::BG));

    frame.render_widget(block, area);
}

// ═══════════════════════════════════════════════════════
// LOCK OVERLAY — Reusable premium lock widget
// ═══════════════════════════════════════════════════════

fn draw_lock_overlay(
    frame: &mut Frame,
    area: Rect,
    panel_name: &str,
    badge: &str,
    features: &[&str],
    accent: Color,
) {
    // Center a popup within the area
    let popup = centered_rect(70, 75, area);

    // Clear the background of popup area
    frame.render_widget(Clear, popup);

    // Popup block with colored border
    let popup_block = Block::default()
        .title(format!(" {} ", badge))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(
            Style::default()
                .fg(accent)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(Color::Black));

    let popup_inner = popup_block.inner(popup);
    frame.render_widget(popup_block, popup);

    // Content inside popup
    let mut lines: Vec<Line> = Vec::new();

    // Lock icon + panel name
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(vec![
        Span::styled(
            "  🔒 ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            panel_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    lines.push(Line::from(Span::raw("")));

    // Divider
    lines.push(Line::from(Span::styled(
        "  ─────────────────────────",
        Style::default().fg(Color::DarkGray),
    )));

    lines.push(Line::from(Span::raw("")));

    // "This feature includes:" heading
    lines.push(Line::from(Span::styled(
        "  This feature includes:",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));

    lines.push(Line::from(Span::raw("")));

    // Feature list with checkmarks
    for feat in features.iter() {
        lines.push(Line::from(vec![
            Span::styled(
                "  ✓ ",
                Style::default()
                    .fg(accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                *feat,
                Style::default().fg(Color::Gray),
            ),
        ]));
    }

    lines.push(Line::from(Span::raw("")));

    // Divider
    lines.push(Line::from(Span::styled(
        "  ─────────────────────────",
        Style::default().fg(Color::DarkGray),
    )));

    lines.push(Line::from(Span::raw("")));

    // Upgrade CTA
    lines.push(Line::from(vec![
        Span::styled(
            "  ► ",
            Style::default()
                .fg(accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Upgrade to NIFY PRO",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled(
            "    nify.pro/upgrade",
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    lines.push(Line::from(Span::raw("")));

    // Blink effect message
    lines.push(Line::from(vec![
        Span::styled(
            "  [ LOCKED ]",
            Style::default()
                .fg(accent)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
    ]));

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(Color::Black)),
        popup_inner,
    );
}

// ═══════════════════════════════════════════════════════
// ANALYTICS — RIGHT side (FREE teaser)
// Yeh free hai — user ko lagey ki kuch toh mila
// ═══════════════════════════════════════════════════════

fn draw_analytics_free(
    frame: &mut Frame,
    app: &App,
    area: Rect,
) {
    let imb = app.book_imbalance();
    let bp  = app.buy_pct();

    let imb_c = if imb > 0.55 { theme::BID_GREEN }
        else if imb < 0.45 { theme::ASK_RED }
        else { theme::TEXT_SECONDARY };

    let bp_c = if bp > 55.0 { theme::BID_GREEN }
        else if bp < 45.0 { theme::ASK_RED }
        else { theme::TEXT_SECONDARY };

    let vwap = app.ticker.weighted_avg_price;
    let curr = app.ticker.last_price;
    let vwap_str = if vwap > 0.0 && curr > 0.0 {
        let d = (curr - vwap) / vwap * 100.0;
        let s = if d >= 0.0 { "+" } else { "" };
        format!("{:.2} ({}{:.2}%)", vwap, s, d)
    } else {
        "---".to_string()
    };

    let analytics = vec![
        Line::from(Span::styled(
            "  ── MARKET ANALYTICS ───────",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Book Imbal  : ", theme::dim_style()),
            Span::styled(
                format!("{:.0}% B / {:.0}% A", imb*100.0, (1.0-imb)*100.0),
                Style::default().fg(imb_c).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Buy Vol %   : ", theme::dim_style()),
            Span::styled(
                format!("{:.1}%", bp),
                Style::default().fg(bp_c).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Bid Depth   : ", theme::dim_style()),
            Span::styled(
                theme::fmt_usd_full(app.total_bid_depth()),
                Style::default().fg(theme::BID_GREEN),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Ask Depth   : ", theme::dim_style()),
            Span::styled(
                theme::fmt_usd_full(app.total_ask_depth()),
                Style::default().fg(theme::ASK_RED),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Whale Trades: ", theme::dim_style()),
            Span::styled(
                format!("🔒 PRO"),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Liqs (L/S)  : ", theme::dim_style()),
            Span::styled(
                format!("🔒 PRO"),
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Liq Value   : ", theme::dim_style()),
            Span::styled(
                "🔒 PRO",
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(vec![
            Span::styled("  VWAP        : ", theme::dim_style()),
            Span::styled(
                vwap_str,
                Style::default().fg(theme::TEXT_SECONDARY),
            ),
        ]),
        Line::from(vec![
            Span::styled("  24h Range   : ", theme::dim_style()),
            Span::styled(
                format!("{:.2} – {:.2}",
                    app.ticker.low_24h, app.ticker.high_24h),
                Style::default().fg(theme::TEXT_SECONDARY),
            ),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(analytics)
            .style(Style::default().bg(theme::BG)),
        area,
    );
}

// ═══════════════════════════════════════════════════════
// FOOTER
// ═══════════════════════════════════════════════════════

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let line = Line::from(vec![
        Span::styled(
            " [Q/ESC] Quit ",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(theme::BORDER)),
        Span::styled(
            "[R] Reset ",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(theme::BORDER)),
        Span::styled(
            "Binance USDT-M Futures ",
            theme::dim_style(),
        ),
        Span::styled("│ ", Style::default().fg(theme::BORDER)),
        Span::styled(
            format!("Trades:{}  Liqs:{} ", app.trades.len(), app.liquidations.len()),
            theme::dim_style(),
        ),
        Span::styled("│ ", Style::default().fg(theme::BORDER)),
        Span::styled(
            " 🔒 NIFY PRO — Unlock All Features ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
    ]);

    frame.render_widget(
        Paragraph::new(line).block(theme::footer_block()),
        area,
    );
}

// ═══════════════════════════════════════════════════════
// HELPERS
// ═══════════════════════════════════════════════════════

// Confidence bar
fn conf_bar(conf: f64) -> String {
    let filled = ((conf / 100.0) * 10.0).round() as usize;
    let empty  = 10usize.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

// Centered popup rect helper
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}