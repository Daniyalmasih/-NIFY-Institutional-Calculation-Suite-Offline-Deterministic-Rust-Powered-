use std::collections::VecDeque;

// ═══════════════════════════════════════════════════════
// NIFY APP STATE
// ═══════════════════════════════════════════════════════

const MAX_TRADES: usize       = 200;
const MAX_LIQS: usize         = 100;
const MAX_PRICE_HIST: usize   = 300;

#[derive(Clone, Debug)]
pub struct Trade {
    pub price: f64,
    pub qty: f64,
    pub value_usd: f64,
    pub is_buyer_maker: bool,
    pub timestamp: u64,
    pub is_whale: bool,
}

#[derive(Clone, Debug)]
pub struct Level {
    pub price: f64,
    pub qty: f64,
    pub value_usd: f64,
}

#[derive(Clone, Debug)]
pub struct Liquidation {
    pub symbol: String,
    pub side: String,
    pub price: f64,
    pub qty: f64,
    pub value_usd: f64,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Default)]
pub struct TickerStats {
    pub last_price: f64,
    pub price_change: f64,
    pub price_change_pct: f64,
    pub high_24h: f64,
    pub low_24h: f64,
    pub volume_24h: f64,
    pub quote_volume_24h: f64,
    pub weighted_avg_price: f64,
}

#[derive(Clone, Debug)]
pub struct TradingSignal {
    pub signal_type: SignalType,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit_1: f64,
    pub take_profit_2: f64,
    pub take_profit_3: f64,
    pub risk_reward: f64,
    pub confidence: f64,
    pub reasoning: String,
    pub timestamp: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SignalType {
    Long,
    Short,
    Neutral,
}

impl std::fmt::Display for SignalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalType::Long    => write!(f, "LONG  ▲"),
            SignalType::Short   => write!(f, "SHORT ▼"),
            SignalType::Neutral => write!(f, "NEUTRAL"),
        }
    }
}

pub struct App {
    pub symbol: String,
    pub running: bool,

    // Market data
    pub ticker: TickerStats,
    pub trades: VecDeque<Trade>,
    pub bids: Vec<Level>,   // index 0 = best bid (highest)
    pub asks: Vec<Level>,   // index 0 = best ask (lowest)
    pub liquidations: VecDeque<Liquidation>,

    // Price history for signal + sparkline
    pub price_history: VecDeque<f64>,

    // Session counters
    pub buy_volume: f64,
    pub sell_volume: f64,
    pub whale_count: u32,
    pub liq_count_long: u32,
    pub liq_count_short: u32,
    pub total_liq_value: f64,
    pub cvd: f64,   // Cumulative Volume Delta

    // Signal
    pub signal: Option<TradingSignal>,
    pub signal_tick: u64,

    // Alert state (liq > $500k flash)
    pub liq_alert_ticks: u32,

    // WebSocket status
    pub ws_trade_ok: bool,
    pub ws_depth_ok: bool,
    pub ws_liq_ok: bool,
    pub ws_ticker_ok: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            symbol: "BTCUSDT".to_string(),
            running: true,
            ticker: TickerStats::default(),
            trades: VecDeque::with_capacity(MAX_TRADES),
            bids: Vec::with_capacity(20),
            asks: Vec::with_capacity(20),
            liquidations: VecDeque::with_capacity(MAX_LIQS),
            price_history: VecDeque::with_capacity(MAX_PRICE_HIST),
            buy_volume: 0.0,
            sell_volume: 0.0,
            whale_count: 0,
            liq_count_long: 0,
            liq_count_short: 0,
            total_liq_value: 0.0,
            cvd: 0.0,
            signal: None,
            signal_tick: 0,
            liq_alert_ticks: 0,
            ws_trade_ok: false,
            ws_depth_ok: false,
            ws_liq_ok: false,
            ws_ticker_ok: false,
        }
    }

    pub fn add_trade(&mut self, t: Trade) {
        // CVD update
        if t.is_buyer_maker {
            // Taker sold → sell pressure
            self.sell_volume += t.value_usd;
            self.cvd -= t.value_usd;
        } else {
            // Taker bought → buy pressure
            self.buy_volume += t.value_usd;
            self.cvd += t.value_usd;
        }

        if t.is_whale {
            self.whale_count += 1;
        }

        self.price_history.push_back(t.price);
        if self.price_history.len() > MAX_PRICE_HIST {
            self.price_history.pop_front();
        }

        self.trades.push_front(t);
        if self.trades.len() > MAX_TRADES {
            self.trades.pop_back();
        }

        self.signal_tick += 1;
        if self.signal_tick % 30 == 0 {
            self.compute_signal();
        }
    }

    pub fn add_liq(&mut self, liq: Liquidation) {
        // side="SELL" → exchange selling → LONG was liquidated
        // side="BUY"  → exchange buying  → SHORT was liquidated
        if liq.side == "SELL" {
            self.liq_count_long += 1;
        } else {
            self.liq_count_short += 1;
        }

        self.total_liq_value += liq.value_usd;

        // Flash alert for big liquidations
        if liq.value_usd >= 500_000.0 {
            self.liq_alert_ticks = 60; // ~2 seconds at 30fps
        }

        self.liquidations.push_front(liq);
        if self.liquidations.len() > MAX_LIQS {
            self.liquidations.pop_back();
        }
    }

    pub fn update_book(
        &mut self,
        bids: Vec<Level>,
        asks: Vec<Level>,
    ) {
        self.bids = bids;
        self.asks = asks;
    }

    pub fn update_ticker(&mut self, s: TickerStats) {
        self.ticker = s;
    }

    pub fn tick(&mut self) {
        if self.liq_alert_ticks > 0 {
            self.liq_alert_ticks -= 1;
        }
    }

    pub fn mid_price(&self) -> f64 {
        if !self.bids.is_empty() && !self.asks.is_empty() {
            (self.bids[0].price + self.asks[0].price) / 2.0
        } else {
            self.ticker.last_price
        }
    }

    pub fn spread_bps(&self) -> f64 {
        if !self.bids.is_empty()
            && !self.asks.is_empty()
            && self.bids[0].price > 0.0
        {
            ((self.asks[0].price - self.bids[0].price)
                / self.bids[0].price)
                * 10_000.0
        } else {
            0.0
        }
    }

    pub fn total_bid_depth(&self) -> f64 {
        self.bids.iter().map(|l| l.value_usd).sum()
    }

    pub fn total_ask_depth(&self) -> f64 {
        self.asks.iter().map(|l| l.value_usd).sum()
    }

    pub fn book_imbalance(&self) -> f64 {
        let b = self.total_bid_depth();
        let a = self.total_ask_depth();
        let t = b + a;
        if t > 0.0 { b / t } else { 0.5 }
    }

    pub fn buy_pct(&self) -> f64 {
        let t = self.buy_volume + self.sell_volume;
        if t > 0.0 { (self.buy_volume / t) * 100.0 } else { 50.0 }
    }

    pub fn reset_session(&mut self) {
        self.buy_volume      = 0.0;
        self.sell_volume     = 0.0;
        self.whale_count     = 0;
        self.liq_count_long  = 0;
        self.liq_count_short = 0;
        self.total_liq_value = 0.0;
        self.cvd             = 0.0;
        self.signal          = None;
        self.signal_tick     = 0;
        self.liquidations.clear();
    }

    // ── Signal Engine ─────────────────────────────────

    pub fn compute_signal(&mut self) {
        let price = self.mid_price();
        if price <= 0.0 || self.price_history.len() < 20 {
            return;
        }

        let hist: Vec<f64> =
            self.price_history.iter().copied().collect();
        let n = hist.len();

        // Factor 1: Book imbalance score (-1 to +1)
        let imb = self.book_imbalance();
        let imb_score = (imb - 0.5) * 2.0;

        // Factor 2: Volume pressure score (-1 to +1)
        let vol_score = (self.buy_pct() / 100.0 - 0.5) * 2.0;

        // Factor 3: Momentum (recent 5 vs prev 5)
        let r5 = if n >= 5 {
            hist[n - 5..].iter().sum::<f64>() / 5.0
        } else {
            price
        };
        let p5 = if n >= 10 {
            hist[n - 10..n - 5].iter().sum::<f64>() / 5.0
        } else {
            price
        };
        let mom = if p5 > 0.0 {
            ((r5 - p5) / p5 * 1000.0).clamp(-1.0, 1.0)
        } else {
            0.0
        };

        // Factor 4: Liquidation bias
        let tl = self.liq_count_long + self.liq_count_short;
        let liq_score = if tl > 0 {
            let sp = self.liq_count_short as f64 / tl as f64;
            (sp - 0.5) * 2.0
        } else {
            0.0
        };

        let composite = imb_score * 0.30
            + vol_score  * 0.35
            + mom        * 0.20
            + liq_score  * 0.15;

        let confidence =
            (composite.abs() * 100.0).clamp(5.0, 95.0);

        let atr = self.calc_atr(&hist, 14)
            .max(price * 0.0003);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let (stype, sl, tp1, tp2, tp3) =
            if composite > 0.10 {
                (
                    SignalType::Long,
                    price - atr * 2.5,
                    price + atr * 2.0,
                    price + atr * 4.0,
                    price + atr * 7.0,
                )
            } else if composite < -0.10 {
                (
                    SignalType::Short,
                    price + atr * 2.5,
                    price - atr * 2.0,
                    price - atr * 4.0,
                    price - atr * 7.0,
                )
            } else {
                (
                    SignalType::Neutral,
                    price - atr * 2.5,
                    price + atr * 2.0,
                    price + atr * 4.0,
                    price + atr * 7.0,
                )
            };

        let sl_dist = (price - sl).abs().max(0.01);
        let rr      = (tp1 - price).abs() / sl_dist;

        let mut reasons: Vec<&str> = Vec::new();
        if imb_score >  0.15 { reasons.push("Bid-heavy"); }
        if imb_score < -0.15 { reasons.push("Ask-heavy"); }
        if vol_score >  0.15 { reasons.push("Buy pressure"); }
        if vol_score < -0.15 { reasons.push("Sell pressure"); }
        if mom >  0.15 { reasons.push("Upward momentum"); }
        if mom < -0.15 { reasons.push("Downward momentum"); }
        if liq_score >  0.10 && tl > 0 {
            reasons.push("Short liqs");
        }
        if liq_score < -0.10 && tl > 0 {
            reasons.push("Long liqs");
        }

        let reasoning = if reasons.is_empty() {
            "Weak — wait for setup".to_string()
        } else {
            reasons.join(" + ")
        };

        self.signal = Some(TradingSignal {
            signal_type: stype,
            entry_price: price,
            stop_loss: sl,
            take_profit_1: tp1,
            take_profit_2: tp2,
            take_profit_3: tp3,
            risk_reward: rr,
            confidence,
            reasoning,
            timestamp: now,
        });
    }

    fn calc_atr(&self, prices: &[f64], period: usize) -> f64 {
        if prices.len() < 2 {
            return 0.0;
        }
        let n  = prices.len();
        let s  = n.saturating_sub(period + 1);
        let sl = &prices[s..];
        if sl.len() < 2 {
            return 0.0;
        }
        let sum: f64 = sl
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .sum();
        sum / (sl.len() - 1) as f64
    }
}