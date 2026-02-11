use crate::state::TradeStatus;
use log::warn;
use std::time::{Duration, Instant};

pub struct RiskManager {
    pub status: TradeStatus,
    pub last_order_time: Instant,
    pub cooldown: Duration,
    pub max_units: f64,
}

impl RiskManager {
    pub fn new(max_units: f64) -> Self {
        Self {
            status: TradeStatus::Idle,
            last_order_time: Instant::now() - Duration::from_secs(60),
            cooldown: Duration::from_secs(5), // Espera mínima entre órdenes
            max_units,
        }
    }

    /// La función crítica: decide si se permite la ejecución
    pub fn validate_execution(&self, requested_qty: f64) -> bool {
        // 1. Regla de Oro: Solo una posición a la vez
        if self.status != TradeStatus::Idle {
            return false;
        }

        // 2. Regla de Cooldown: Evitar ráfagas por ruido
        if self.last_order_time.elapsed() < self.cooldown {
            warn!("RiskManager: Cooldown activo. Ignorando señal.");
            return false;
        }

        // 3. Límite de Exposición
        if requested_qty > self.max_units {
            warn!(
                "RiskManager: Cantidad solicitada ({}) excede el máximo permitido.",
                requested_qty
            );
            return false;
        }

        true
    }

    pub fn set_status(&mut self, new_status: TradeStatus) {
        if new_status == TradeStatus::PendingNew {
            self.last_order_time = Instant::now();
        }
        self.status = new_status;
    }
}
