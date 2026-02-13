use std::collections::BTreeMap;

/// Representa el ciclo de vida completo de una orden según el protocolo FIX.
/// Crucial para el Executor (Fase 4-03) y el RiskManager (Fase 4-02).
#[derive(Debug, PartialEq, Clone)]
pub enum TradeStatus {
    Idle,            // Sin posiciones abiertas, listos para operar.
    PendingNew,      // 35=D enviada, esperando confirmación inicial del Broker.
    New,             // El broker ha aceptado la orden (ACK) y está en el libro.
    PartiallyFilled, // La orden se ha ejecutado en parte.
    Filled,          // Posición abierta y activa al 100%.
    Rejected,        // La orden fue rechazada por el broker o el motor de riesgo.
}

pub struct OrderBook {
    pub bids: BTreeMap<i64, f64>, // Precio (escalado a i64) -> Volumen
    pub asks: BTreeMap<i64, f64>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

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

    /// Imbalance simple (Nivel 1)
    pub fn get_imbalance(&self) -> f64 {
        let b_vol = self.bids.values().next().unwrap_or(&0.0);
        let a_vol = self.asks.values().next().unwrap_or(&0.0);
        if b_vol + a_vol == 0.0 {
            return 0.0;
        }
        (b_vol - a_vol) / (b_vol + a_vol)
    }

    pub fn get_book_intensity(&self) -> f64 {
        let total_bid: f64 = self.bids.values().sum();
        let total_ask: f64 = self.asks.values().sum();
        total_bid + total_ask
    }

    /// Extrae un vector de imbalance por niveles (Profundidad).
    pub fn get_depth_vector(&self, levels: usize) -> Vec<f64> {
        let mut depth_v = Vec::with_capacity(levels);

        let mut bid_iter = self.bids.values().rev();
        let mut ask_iter = self.asks.values();

        for _ in 0..levels {
            let b_vol = bid_iter.next().unwrap_or(&0.0);
            let a_vol = ask_iter.next().unwrap_or(&0.0);

            let imb = if b_vol + a_vol == 0.0 {
                0.0
            } else {
                (b_vol - a_vol) / (b_vol + a_vol)
            };
            depth_v.push(imb);
        }
        depth_v
    }
}

