use crate::risk::RiskManager;
use crate::state::{Position, TradeStatus};
use log::{error, info, warn};

pub struct Executor {
    pub active_position: Option<Position>,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            active_position: None,
        }
    }

    pub fn monitor_position(
        &mut self,
        current_mid: f64,
        current_sigma: f64,
        risk_manager: &mut RiskManager,
    ) -> bool {
        let pos = match &self.active_position {
            Some(p) => p,
            None => return false,
        };

        let trigger_exit = if pos.side == '1' {
            current_mid >= pos.tp_price || current_mid <= pos.sl_price
        } else {
            current_mid <= pos.tp_price || current_mid >= pos.sl_price
        };

        if trigger_exit {
            info!("[EXECUTOR] 🎯 Salida por niveles detectada (TP/SL).");
            return true;
        }

        if current_sigma > (pos.entry_sigma_total * risk_manager.lambda_epistemic) {
            warn!("[EXECUTOR] ⚠️ Tesis invalidada por alta incertidumbre (Sigma Spike).");
            return true;
        }

        false
    }

    pub fn handle_execution_report(
        &mut self,
        msg: &str,
        risk_manager: &mut RiskManager,
        pending_thesis: &mut Option<Position>, // Cambiado a mut para poder limpiar la tesis
    ) {
        // --- 1. EXTRACCIÓN DE TAGS CRÍTICOS ---
        let tags: std::collections::HashMap<&str, &str> = msg
            .split('|')
            .filter_map(|s| {
                let mut parts = s.splitn(2, '=');
                Some((parts.next()?, parts.next()?))
            })
            .collect();

        let msg_type = tags.get("35").unwrap_or(&"");
        let cl_ord_id = tags.get("11").unwrap_or(&"");
        let ord_status = tags.get("39").unwrap_or(&"");
        let text = tags.get("58").unwrap_or(&"Sin detalle");

        // --- 2. MANEJO DE REJECTS DE NEGOCIO (35=j) O EJECUCIÓN (39=8) ---
        if *msg_type == "j" || *ord_status == "8" {
            error!(
                "[EXECUTOR] ❌ ORDEN RECHAZADA. Razón: {}. ID: {}",
                text, cl_ord_id
            );

            // Si el rechazo es sobre nuestra tesis pendiente, limpiamos
            if let Some(thesis) = pending_thesis {
                if thesis.cl_ord_id == *cl_ord_id {
                    risk_manager.set_status(TradeStatus::Idle);
                    *pending_thesis = None;
                    info!("[EXECUTOR] Tesis descartada. Sistema en Cooldown.");
                }
            }
            return;
        }

        // --- 3. MANEJO DE SESSION REJECT (35=3) ---
        if *msg_type == "3" {
            error!("[EXECUTOR] 🚨 SESSION REJECT: Error de protocolo. Revisar logs FIX. No se altera posición.");
            return;
        }

        // --- 4. MANEJO DE EXECUTION REPORTS (35=8) ---
        if *msg_type == "8" {
            match *ord_status {
                "0" => {
                    // NEW
                    info!("[EXECUTOR] ✅ Orden aceptada en servidor: {}", cl_ord_id);
                    risk_manager.set_status(TradeStatus::New);
                }
                "2" => {
                    // FILLED
                    // VALIDACIÓN DE IDENTIDAD
                    if let Some(thesis) = pending_thesis {
                        if thesis.cl_ord_id == *cl_ord_id {
                            info!(
                                "[EXECUTOR] 🎯 FILL CONFIRMADO para ID: {}. Activando tracking.",
                                cl_ord_id
                            );
                            risk_manager.set_status(TradeStatus::Filled);
                            self.active_position = Some(thesis.clone());
                            *pending_thesis = None; // Limpiamos la tesis una vez activa
                        } else {
                            warn!(
                                "[EXECUTOR] ⚠️ Recibido Fill para ClOrdID ajeno: {}. Ignorando.",
                                cl_ord_id
                            );
                        }
                    }
                }
                "4" | "C" => {
                    // CANCELLED o EXPIRED
                    info!("[EXECUTOR] 🛑 Orden cerrada/cancelada. ID: {}", cl_ord_id);
                    risk_manager.set_status(TradeStatus::Idle);
                    self.active_position = None;
                }
                _ => {}
            }
        }

        // --- 5. CANCEL REJECT (35=9) ---
        if *msg_type == "9" {
            let cxl_rej_reason = tags.get("102").unwrap_or(&"0");
            warn!("[EXECUTOR] ⚠️ Cancel Reject (ID: {}). Motivo Tag 102: {}. Esperando reporte final.", cl_ord_id, cxl_rej_reason);
            // No asumimos cierre; esperamos el 35=8 que confirme el estado real.
        }
    }
}

