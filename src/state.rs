use std::collections::BTreeMap;

pub struct OrderBook {
    pub bids: BTreeMap<i64, f64>,
    pub asks: BTreeMap<i64, f64>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    fn scale_price(price: f64) -> i64 {
        (price * 100000.0).round() as i64
    }

    pub fn update(&mut self, action: char, side: char, price: f64, volume: f64) {
        let scaled_price = Self::scale_price(price);

        // 1. Determinar el mapa objetivo y el mapa opuesto
        let (target_map, opposite_map) = if side == '0' {
            (&mut self.bids, &mut self.asks)
        } else {
            (&mut self.asks, &mut self.bids)
        };

        match action {
            '0' | '1' => {
                // New o Change
                if volume > 0.0 {
                    target_map.insert(scaled_price, volume);
                    // 2. Limpieza de libro cruzado:
                    // Si inserto un Bid >= mejor Ask, o un Ask <= mejor Bid, elimino el conflicto
                    opposite_map.remove(&scaled_price);
                } else {
                    target_map.remove(&scaled_price);
                }
            }
            '2' => {
                // Delete
                target_map.remove(&scaled_price);
            }
            _ => {}
        }
    }

    pub fn get_best_bid(&self) -> Option<f64> {
        self.bids.keys().rev().next().map(|&p| p as f64 / 100000.0)
    }

    pub fn get_best_ask(&self) -> Option<f64> {
        self.asks.keys().next().map(|&p| p as f64 / 100000.0)
    }

    pub fn get_imbalance(&self) -> f64 {
        let total_bid_vol: f64 = self.bids.values().sum();
        let total_ask_vol: f64 = self.asks.values().sum();
        let total_vol = total_bid_vol + total_ask_vol;

        if total_vol > 0.0 {
            (total_bid_vol - total_ask_vol) / total_vol
        } else {
            0.0
        }
    }
}

