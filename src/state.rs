use std::collections::BTreeMap;

pub struct OrderBook {
    pub bids: BTreeMap<i64, f64>, // Precio (escalado a i64 para precisión) -> Volumen
    pub asks: BTreeMap<i64, f64>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    /// Actualiza el libro. side: '0'=Bid, '1'=Ask. action: '1'=Update/Insert, '2'=Delete
    pub fn update(&mut self, action: char, side: char, price: f64, volume: f64) {
        let p_key = (price * 100000.0) as i64;
        if side == '0' {
            if action == '2' || volume == 0.0 {
                self.bids.remove(&p_key);
            } else {
                self.bids.insert(p_key, volume);
            }
        } else {
            if action == '2' || volume == 0.0 {
                self.asks.remove(&p_key);
            } else {
                self.asks.insert(p_key, volume);
            }
        }
    }

    pub fn get_mid_price(&self) -> Option<f64> {
        let best_bid = self.bids.keys().rev().next()?;
        let best_ask = self.asks.keys().next()?;
        Some((*best_bid as f64 + *best_ask as f64) / 200000.0)
    }

    pub fn get_best_bid(&self) -> Option<f64> {
        self.bids.keys().rev().next().map(|&p| p as f64 / 100000.0)
    }

    pub fn get_best_ask(&self) -> Option<f64> {
        self.asks.keys().next().map(|&p| p as f64 / 100000.0)
    }

    pub fn get_imbalance(&self) -> f64 {
        let b_vol = self.bids.values().next().unwrap_or(&0.0);
        let a_vol = self.asks.values().next().unwrap_or(&0.0);
        if b_vol + a_vol == 0.0 {
            return 0.0;
        }
        (b_vol - a_vol) / (b_vol + a_vol)
    }

    /// MÉTRICA NUEVA: Intensidad total del libro (Liquidez total visible)
    pub fn get_book_intensity(&self) -> f64 {
        let total_bid: f64 = self.bids.values().sum();
        let total_ask: f64 = self.asks.values().sum();
        total_bid + total_ask
    }
}

