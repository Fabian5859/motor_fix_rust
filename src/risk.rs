use crate::math_utils;
use crate::state::TradeStatus;
use log::{error, info, warn};
use std::time::{Duration, Instant};

pub struct RiskManager {
    pub status: TradeStatus,
    pub last_order_time: Instant,
    pub cooldown: Duration,
    pub max_units: f64,

    // --- PARÁMETROS BAYESIANOS ---
    pub snr_threshold: f64,
    pub kelly_fraction: f64,
    pub lambda_epistemic: f64,
    pub tp_quantile: f64,
    pub sl_quantile: f64,
}

impl RiskManager {
    pub fn new(max_units: f64) -> Self {
        Self {
            status: TradeStatus::Idle,
            last_order_time: Instant::now() - Duration::from_secs(60),
            cooldown: Duration::from_secs(5),
            max_units,
            snr_threshold: 0.4,
            kelly_fraction: 0.1,
            lambda_epistemic: 1.5,
            tp_quantile: 0.75,
            sl_quantile: 0.25,
        }
    }

    pub fn evaluate_bayesian_trade(
        &self,
        mid_price: f64,
        mu: f64,
        sigma_aleatoria: f64,
        sigma_epistemic: f64,
        side: char,
    ) -> Option<(f64, f64, f64)> {
        // [DEBUG RISK 1]: Verificación de disponibilidad
        if self.status != TradeStatus::Idle {
            info!(
                "[DEBUG RISK 1] Rechazado: Motor ocupado ({:?})",
                self.status
            );
            return None;
        }

        let elapsed = self.last_order_time.elapsed();
        if elapsed < self.cooldown {
            return None;
        }

        // [DEBUG RISK 2]: Cálculo de Incertidumbre Combinada
        let weighted_epistemic = (self.lambda_epistemic * sigma_epistemic).powi(2);
        let sigma_total = (sigma_aleatoria.powi(2) + weighted_epistemic).sqrt();

        // [DEBUG RISK 3]: Filtro de SNR
        let mu_signal = (mu - 0.5).abs();
        let snr = mu_signal / sigma_total.max(1e-6);

        if snr < self.snr_threshold {
            warn!(
                "[DEBUG RISK 3] SNR bajo: {:.2} < {:.2}",
                snr, self.snr_threshold
            );
            return None;
        }

        // [DEBUG RISK 4]: Dimensionamiento y Redondeo Estricto
        // Por ahora, forzamos a 1000.0 según tu requerimiento.
        // En el futuro, usa: let volume_step = 1000.0;
        let final_qty = 1000.0;

        // [DEBUG RISK 5]: Cálculo de niveles
        let directional_mu = mu - 0.5;

        let (tp, sl) = math_utils::calculate_bayesian_levels(
            mid_price,
            directional_mu,
            sigma_total,
            side,
            self.tp_quantile,
            self.sl_quantile,
        );

        // Redondeo de precios para evitar 35=j por precisión de precio (ej. 5 o 6 decimales según símbolo)
        let tp_rounded = (tp * 100000.0).round() / 100000.0;
        let sl_rounded = (sl * 100000.0).round() / 100000.0;

        // [DEBUG RISK 6]: Verificación de consistencia
        if tp_rounded <= 0.0 || sl_rounded <= 0.0 || (tp_rounded - mid_price).abs() < 1e-7 {
            error!(
                "[DEBUG RISK 6] Niveles inválidos: TP {}, SL {}",
                tp_rounded, sl_rounded
            );
            return None;
        }

        info!(
            "✅ [RISK OK] SEÑAL VALIDADA | IDLE -> PendingNew | Qty: {} | TP: {:.5} | SL: {:.5}",
            final_qty, tp_rounded, sl_rounded
        );

        Some((final_qty, tp_rounded, sl_rounded))
    }

    pub fn set_status(&mut self, new_status: TradeStatus) {
        info!("[STATUS] {:?} -> {:?}", self.status, new_status);
        if new_status == TradeStatus::PendingNew {
            self.last_order_time = Instant::now();
        }
        self.status = new_status;
    }
}

