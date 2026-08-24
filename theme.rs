use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, BorderType};

pub const BG: Color                  = Color::Black;
pub const TEXT_PRIMARY: Color        = Color::White;
pub const TEXT_SECONDARY: Color      = Color::Gray;
pub const TEXT_DIM: Color            = Color::DarkGray;
pub const BORDER: Color              = Color::DarkGray;
pub const HEADER: Color              = Color::White;
pub const BID_GREEN: Color           = Color::Green;
pub const ASK_RED: Color             = Color::Red;
pub const WHALE_ORANGE: Color        = Color::Yellow;
pub const LIQUIDATION_MAGENTA: Color = Color::Magenta;
pub const POSITIVE: Color            = Color::Green;
pub const NEGATIVE: Color            = Color::Red;
pub const NEUTRAL: Color             = Color::White;
pub const SIGNAL_ENTRY: Color        = Color::Cyan;
pub const SIGNAL_TP: Color           = Color::Green;
pub const SIGNAL_SL: Color           = Color::Red;

pub fn nify_block(title: &str) -> Block<'_> {
    Block::default()
        .title(format!(" {} ", title))
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(BG))
}

pub fn footer_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(BG))
}

pub fn bid_style() -> Style {
    Style::default().fg(BID_GREEN).bg(BG)
}

pub fn ask_style() -> Style {
    Style::default().fg(ASK_RED).bg(BG)
}

pub fn whale_style() -> Style {
    Style::default()
        .fg(WHALE_ORANGE)
        .bg(BG)
        .add_modifier(Modifier::BOLD)
}

pub fn liq_style() -> Style {
    Style::default()
        .fg(LIQUIDATION_MAGENTA)
        .bg(BG)
        .add_modifier(Modifier::BOLD)
}

pub fn dim_style() -> Style {
    Style::default().fg(TEXT_DIM).bg(BG)
}

pub fn change_color(v: f64) -> Color {
    if v > 0.0      { POSITIVE }
    else if v < 0.0 { NEGATIVE }
    else            { NEUTRAL }
}

pub fn fmt_usd_short(v: f64) -> String {
    let abs = v.abs();
    let sign = if v < 0.0 { "-" } else { "" };
    if abs >= 1_000_000_000.0 {
        format!("{}${:.2}B", sign, abs / 1_000_000_000.0)
    } else if abs >= 1_000_000.0 {
        format!("{}${:.2}M", sign, abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{}${:.1}K", sign, abs / 1_000.0)
    } else {
        format!("{}${:.0}", sign, abs)
    }
}

pub fn fmt_usd_full(v: f64) -> String {
    if v >= 1_000_000.0 {
        format!("${:.2}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
        format!("${:.0}", v)
    } else {
        format!("${:.2}", v)
    }
}

pub fn depth_bar(value: f64, max: f64, width: usize) -> String {
    if max <= 0.0 {
        return String::new();
    }
    let f = ((value / max) * width as f64).round() as usize;
    "█".repeat(f.min(width))
}