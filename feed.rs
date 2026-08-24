use tokio::sync::mpsc;
use futures_util::StreamExt;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::app::{Trade, Level, Liquidation, TickerStats};

// ═══════════════════════════════════════════════════════
// NIFY FEED ENGINE
// Binance USDT-M Futures — wss://fstream.binance.com/ws
// ═══════════════════════════════════════════════════════

const WS: &str = "wss://fstream.binance.com/ws";

#[derive(Debug)]
pub enum FeedMsg {
    Trade(Trade),
    Book { bids: Vec<Level>, asks: Vec<Level> },
    Liq(Liquidation),
    Ticker(TickerStats),
    Status { stream: &'static str, ok: bool },
}

pub fn spawn_feeds(tx: mpsc::UnboundedSender<FeedMsg>) {
    // Trade stream
    {
        let t = tx.clone();
        tokio::spawn(async move {
            loop {
                trade_stream(t.clone()).await;
                tokio::time::sleep(
                    tokio::time::Duration::from_secs(3)
                ).await;
            }
        });
    }
    // Depth stream
    {
        let t = tx.clone();
        tokio::spawn(async move {
            loop {
                depth_stream(t.clone()).await;
                tokio::time::sleep(
                    tokio::time::Duration::from_secs(3)
                ).await;
            }
        });
    }
    // Liquidation stream
    {
        let t = tx.clone();
        tokio::spawn(async move {
            loop {
                liq_stream(t.clone()).await;
                tokio::time::sleep(
                    tokio::time::Duration::from_secs(3)
                ).await;
            }
        });
    }
    // Ticker stream
    {
        let t = tx.clone();
        tokio::spawn(async move {
            loop {
                ticker_stream(t.clone()).await;
                tokio::time::sleep(
                    tokio::time::Duration::from_secs(3)
                ).await;
            }
        });
    }
}

// ── Safe parsers ─────────────────────────────────────

#[inline]
fn pf64(v: &serde_json::Value) -> f64 {
    match v {
        serde_json::Value::String(s) => {
            s.parse::<f64>().unwrap_or(0.0)
        }
        serde_json::Value::Number(n) => {
            n.as_f64().unwrap_or(0.0)
        }
        _ => 0.0,
    }
}

#[inline]
fn pu64(v: &serde_json::Value) -> u64 {
    match v {
        serde_json::Value::Number(n) => {
            n.as_u64().unwrap_or(0)
        }
        serde_json::Value::String(s) => {
            s.parse::<u64>().unwrap_or(0)
        }
        _ => 0,
    }
}

#[inline]
fn pstr<'a>(v: &'a serde_json::Value, d: &'a str) -> &'a str {
    v.as_str().unwrap_or(d)
}

fn to_text(msg: Message) -> Option<String> {
    match msg {
        Message::Text(t) => Some(t.to_string()),
        Message::Binary(b) => String::from_utf8(b).ok(),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════
// STREAM 1 — btcusdt@aggTrade
//
// Binance aggTrade payload:
// {
//   "e": "aggTrade",
//   "E": 123456789,   event time
//   "s": "BTCUSDT",
//   "a": 5933014,     agg trade id
//   "p": "0.001",     price (STRING)
//   "q": "100",       quantity (STRING)
//   "f": 100,         first trade id
//   "l": 105,         last trade id
//   "T": 123456785,   trade time
//   "m": true         is buyer maker?
//                     true  = SELL (taker sold)
//                     false = BUY  (taker bought)
// }
// ═══════════════════════════════════════════════════════
async fn trade_stream(tx: mpsc::UnboundedSender<FeedMsg>) {
    let url = format!("{}/btcusdt@aggTrade", WS);

    let ws = match connect_async(url.as_str()).await {
        Ok((ws, _)) => {
            let _ = tx.send(FeedMsg::Status {
                stream: "trade",
                ok: true,
            });
            ws
        }
        Err(e) => {
            eprintln!("[TRADE] connect error: {e}");
            let _ = tx.send(FeedMsg::Status {
                stream: "trade",
                ok: false,
            });
            return;
        }
    };

    let (_, mut rx) = ws.split();

    while let Some(result) = rx.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[TRADE] read error: {e}");
                break;
            }
        };

        if matches!(msg, Message::Close(_)) {
            break;
        }

        let text = match to_text(msg) {
            Some(t) => t,
            None => continue,
        };

        let j: serde_json::Value =
            match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };

        // Verify event type
        let etype = pstr(&j["e"], "");
        if !etype.is_empty() && etype != "aggTrade" {
            continue;
        }

        let price = pf64(&j["p"]);
        let qty   = pf64(&j["q"]);

        if price <= 0.0 || qty <= 0.0 {
            continue;
        }

        let value_usd      = price * qty;
        let is_buyer_maker = j["m"].as_bool().unwrap_or(false);
        let timestamp      = pu64(&j["T"]);

        let trade = Trade {
            price,
            qty,
            value_usd,
            is_buyer_maker,
            timestamp,
            is_whale: value_usd >= 100_000.0,
        };

        if tx.send(FeedMsg::Trade(trade)).is_err() {
            return;
        }
    }

    let _ = tx.send(FeedMsg::Status {
        stream: "trade",
        ok: false,
    });
}

// ═══════════════════════════════════════════════════════
// STREAM 2 — btcusdt@depth20@100ms
//
// Payload:
// {
//   "e": "depthUpdate",
//   "b": [["price","qty"], ...],  bids (desc order)
//   "a": [["price","qty"], ...],  asks (asc order)
// }
//
// bids[0] = BEST bid (highest price)
// asks[0] = BEST ask (lowest price)
// ═══════════════════════════════════════════════════════
async fn depth_stream(tx: mpsc::UnboundedSender<FeedMsg>) {
    let url = format!("{}/btcusdt@depth20@100ms", WS);

    let ws = match connect_async(url.as_str()).await {
        Ok((ws, _)) => {
            let _ = tx.send(FeedMsg::Status {
                stream: "depth",
                ok: true,
            });
            ws
        }
        Err(e) => {
            eprintln!("[DEPTH] connect error: {e}");
            let _ = tx.send(FeedMsg::Status {
                stream: "depth",
                ok: false,
            });
            return;
        }
    };

    let (_, mut rx) = ws.split();

    while let Some(result) = rx.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[DEPTH] read error: {e}");
                break;
            }
        };

        if matches!(msg, Message::Close(_)) {
            break;
        }

        let text = match to_text(msg) {
            Some(t) => t,
            None => continue,
        };

        let j: serde_json::Value =
            match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };

        let bids = parse_book_side(&j["b"], 15);
        let asks = parse_book_side(&j["a"], 15);

        if bids.is_empty() && asks.is_empty() {
            continue;
        }

        if tx.send(FeedMsg::Book { bids, asks }).is_err() {
            return;
        }
    }

    let _ = tx.send(FeedMsg::Status {
        stream: "depth",
        ok: false,
    });
}

fn parse_book_side(
    arr: &serde_json::Value,
    limit: usize,
) -> Vec<Level> {
    let mut levels = Vec::new();
    if let Some(entries) = arr.as_array() {
        for entry in entries.iter().take(limit) {
            if let Some(pair) = entry.as_array() {
                if pair.len() >= 2 {
                    let p = pf64(&pair[0]);
                    let q = pf64(&pair[1]);
                    if p > 0.0 {
                        levels.push(Level {
                            price: p,
                            qty: q,
                            value_usd: p * q,
                        });
                    }
                }
            }
        }
    }
    levels
}

// ═══════════════════════════════════════════════════════
// STREAM 3 — !forceOrder@arr (ALL coins liquidations)
//
// IMPORTANT: Yeh global stream hai
// Single event format:
// {
//   "e": "forceOrder",
//   "E": 1568014460893,
//   "o": {
//     "s": "BTCUSDT",
//     "S": "SELL",       SELL=long liq, BUY=short liq
//     "o": "LIMIT",
//     "f": "IOC",
//     "q": "0.014",      original quantity
//     "p": "9910",       order price
//     "ap": "9910",      average filled price
//     "X": "FILLED",
//     "l": "0.014",      last filled qty
//     "z": "0.014",      cumulative filled qty
//     "T": 1568014460893
//   }
// }
//
// Array format (multiple events):
// [{ same structure }, { same structure }]
// ═══════════════════════════════════════════════════════
async fn liq_stream(tx: mpsc::UnboundedSender<FeedMsg>) {
    // Try global stream first
    let url = format!("{}/!forceOrder@arr", WS);

    let ws = match connect_async(url.as_str()).await {
        Ok((ws, _)) => {
            eprintln!("[LIQ] Global stream connected OK");
            let _ = tx.send(FeedMsg::Status {
                stream: "liq",
                ok: true,
            });
            ws
        }
        Err(e) => {
            eprintln!("[LIQ] Global failed: {e}, trying BTC only");
            liq_stream_btc(tx).await;
            return;
        }
    };

    let (_, mut rx) = ws.split();

    while let Some(result) = rx.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[LIQ] read error: {e}");
                break;
            }
        };

        if matches!(msg, Message::Close(_)) {
            break;
        }

        let text = match to_text(msg) {
            Some(t) => t,
            None => continue,
        };

        let j: serde_json::Value =
            match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[LIQ] JSON parse error: {e}");
                    continue;
                }
            };

        eprintln!("[LIQ] Raw: {}", &text[..text.len().min(200)]);

        // Array of events
        if let Some(arr) = j.as_array() {
            for item in arr {
                if let Some(liq) = parse_liq_event(item) {
                    if tx.send(FeedMsg::Liq(liq)).is_err() {
                        return;
                    }
                }
            }
        } else {
            // Single event
            if let Some(liq) = parse_liq_event(&j) {
                if tx.send(FeedMsg::Liq(liq)).is_err() {
                    return;
                }
            }
        }
    }

    let _ = tx.send(FeedMsg::Status {
        stream: "liq",
        ok: false,
    });
}

async fn liq_stream_btc(tx: mpsc::UnboundedSender<FeedMsg>) {
    let url = format!("{}/btcusdt@forceOrder", WS);
    eprintln!("[LIQ] Using BTC-only stream");

    let ws = match connect_async(url.as_str()).await {
        Ok((ws, _)) => {
            let _ = tx.send(FeedMsg::Status {
                stream: "liq",
                ok: true,
            });
            ws
        }
        Err(e) => {
            eprintln!("[LIQ] BTC stream also failed: {e}");
            let _ = tx.send(FeedMsg::Status {
                stream: "liq",
                ok: false,
            });
            return;
        }
    };

    let (_, mut rx) = ws.split();

    while let Some(result) = rx.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(_) => break,
        };
        if matches!(msg, Message::Close(_)) {
            break;
        }
        let text = match to_text(msg) {
            Some(t) => t,
            None => continue,
        };
        let j: serde_json::Value =
            match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };

        eprintln!("[LIQ-BTC] Raw: {}",
            &text[..text.len().min(200)]);

        if let Some(liq) = parse_liq_event(&j) {
            if tx.send(FeedMsg::Liq(liq)).is_err() {
                return;
            }
        }
    }

    let _ = tx.send(FeedMsg::Status {
        stream: "liq",
        ok: false,
    });
}

fn parse_liq_event(j: &serde_json::Value) -> Option<Liquidation> {
    // Try nested "o" key first (standard forceOrder format)
    let order = if j["o"].is_object() {
        &j["o"]
    } else if j["s"].is_string() {
        // Data is at root level
        j
    } else {
        eprintln!("[LIQ] Unknown format: {:?}", j);
        return None;
    };

    let symbol = pstr(&order["s"], "UNKNOWN").to_string();
    let side   = pstr(&order["S"], "UNKNOWN").to_string();

    if side == "UNKNOWN" {
        return None;
    }

    // Use average filled price "ap", fallback to "p"
    let ap    = pf64(&order["ap"]);
    let price = if ap > 0.0 { ap } else { pf64(&order["p"]) };

    if price <= 0.0 {
        return None;
    }

    // Use cumulative filled qty "z", fallback to "q"
    let z   = pf64(&order["z"]);
    let qty = if z > 0.0 { z } else { pf64(&order["q"]) };

    let value_usd = price * qty;
    let timestamp = pu64(&order["T"]);

    eprintln!(
        "[LIQ] Parsed: {} {} price={:.2} qty={:.4} val=${:.0}",
        symbol, side, price, qty, value_usd
    );

    Some(Liquidation {
        symbol,
        side,
        price,
        qty,
        value_usd,
        timestamp,
    })
}

// ═══════════════════════════════════════════════════════
// STREAM 4 — btcusdt@ticker
//
// Binance Futures 24hr ticker payload:
// {
//   "e": "24hrTicker",
//   "E": 123456789,      event time
//   "s": "BTCUSDT",
//   "p": "0.0015",       price change (STRING)
//   "P": "0.0015",       price change % (STRING)
//   "w": "0.0018",       weighted avg price / VWAP
//   "c": "0.0025",       last price (current price)
//   "Q": "10",           last qty
//   "o": "0.0010",       open price
//   "h": "0.0025",       high price
//   "l": "0.0010",       low price
//   "v": "10000",        total traded base volume
//   "q": "18",           total traded quote volume
//   "O": 0,              stats open time
//   "C": 86400000,       stats close time
//   "F": 0,              first trade id
//   "L": 18150,          last trade id
//   "n": 18151           total trades
// }
// ═══════════════════════════════════════════════════════
async fn ticker_stream(tx: mpsc::UnboundedSender<FeedMsg>) {
    let url = format!("{}/btcusdt@ticker", WS);

    let ws = match connect_async(url.as_str()).await {
        Ok((ws, _)) => {
            eprintln!("[TICKER] Connected OK");
            let _ = tx.send(FeedMsg::Status {
                stream: "ticker",
                ok: true,
            });
            ws
        }
        Err(e) => {
            eprintln!("[TICKER] connect error: {e}");
            let _ = tx.send(FeedMsg::Status {
                stream: "ticker",
                ok: false,
            });
            return;
        }
    };

    let (_, mut rx) = ws.split();

    while let Some(result) = rx.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[TICKER] read error: {e}");
                break;
            }
        };

        if matches!(msg, Message::Close(_)) {
            break;
        }

        let text = match to_text(msg) {
            Some(t) => t,
            None => continue,
        };

        let j: serde_json::Value =
            match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };

        // Debug first few tickers
        eprintln!(
            "[TICKER] e={} c={} h={} l={} P={}",
            pstr(&j["e"], "?"),
            pstr(&j["c"], "?"),
            pstr(&j["h"], "?"),
            pstr(&j["l"], "?"),
            pstr(&j["P"], "?"),
        );

        let last_price = pf64(&j["c"]);
        if last_price <= 0.0 {
            eprintln!("[TICKER] Zero price, skipping");
            continue;
        }

        let stats = TickerStats {
            last_price,
            price_change:       pf64(&j["p"]),
            price_change_pct:   pf64(&j["P"]),
            high_24h:           pf64(&j["h"]),
            low_24h:            pf64(&j["l"]),
            volume_24h:         pf64(&j["v"]),
            quote_volume_24h:   pf64(&j["q"]),
            weighted_avg_price: pf64(&j["w"]),
        };

        eprintln!(
            "[TICKER] Sending price={:.2} chg={:.2}% h={:.2} l={:.2}",
            stats.last_price,
            stats.price_change_pct,
            stats.high_24h,
            stats.low_24h,
        );

        if tx.send(FeedMsg::Ticker(stats)).is_err() {
            return;
        }
    }

    let _ = tx.send(FeedMsg::Status {
        stream: "ticker",
        ok: false,
    });
}