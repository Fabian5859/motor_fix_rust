use std::collections::BTreeMap;

/// Representa el ciclo de vida completo de una orden.
/// Añadimos Copy para facilitar comparaciones en el loop principal.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TradeStatus {
    Idle,
    PendingNew,
    New,
    PartiallyFilled,
    Filled,
    Rejected,
}

/// --- Gestión de Posición Activa ---
/// Esta estructura guarda la "tesis" de la operación.
#[derive(Debug, Clone)]
pub struct Position {
    pub cl_ord_id: String,
    pub entry_price: f64,
    pub side: char, // '1' Buy, '2' Sell
    pub qty: f64,
    pub tp_price: f64,
    pub sl_price: f64,

    // Métricas Bayesianas para monitoreo de "Sigma Spike"
    pub entry_mu: f64,
    pub entry_sigma_total: f64,
    pub entry_snr: f64,
}

pub struct OrderBook {
    pub bids: BTreeMap<i64, f64>, // Precio escalado (i64) -> Volumen
    pub asks: BTreeMap<i64, f64>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    /// Actualiza el libro de órdenes.
    /// Escala precios a i64 para evitar errores de precisión en las llaves del BTreeMap.
    pub fn update(&mut self, action: char, side: char, price: f64, volume: f64) {
        let p_key = (price * 100000.0).round() as i64;
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

    /// Calcula el precio medio (Mid Price)
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

    /// Imbalance del primer nivel (L1)
    pub fn get_imbalance(&self) -> f64 {
        let b_vol = self.bids.values().rev().next().unwrap_or(&0.0);
        let a_vol = self.asks.values().next().unwrap_or(&0.0);
        if b_vol + a_vol == 0.0 {
            return 0.0;
        }
        (b_vol - a_vol) / (b_vol + a_vol)
    }

    pub fn get_book_intensity(&self) -> f64 {
        self.bids.values().sum::<f64>() + self.asks.values().sum::<f64>()
    }

    /// Genera un vector de imbalance para múltiples niveles de profundidad.
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

