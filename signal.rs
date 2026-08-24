// Signal module — computation is in app.rs
use crate::app::{TradingSignal, SignalType};

pub fn is_bullish(s: &TradingSignal) -> bool {
    s.signal_type == SignalType::Long
}

pub fn is_bearish(s: &TradingSignal) -> bool {
    s.signal_type == SignalType::Short
}

pub fn summary(s: &TradingSignal) -> String {
    format!(
        "{} E:{:.2} SL:{:.2} TP:{:.2} RR:{:.2}",
        s.signal_type,
        s.entry_price,
        s.stop_loss,
        s.take_profit_1,
        s.risk_reward,
    )
}