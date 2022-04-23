use std::fmt::Display;

use time::OffsetDateTime;

/// Instrument's price change event
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tick {
    pub time: i64,

    pub ask: f64,
    pub bid: f64,
    pub ask_volume: f64,
    pub bid_volume: f64,
}

impl Display for Tick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Ok(date_time) = OffsetDateTime::from_unix_timestamp(self.time) {
            write!(
                f,
                "{} {}\t\t{:<16} {:<16} {:<26} {:<26}",
                date_time.date(), date_time.time(), self.ask, self.bid, self.ask_volume, self.bid_volume
            )
        } else {
            Err(std::fmt::Error::default())
        }
    }
}
