use crate::risk::RiskManager;
use crate::state::TradeStatus;
use log::{error, info, warn};

pub struct Executor;

impl Executor {
    /// Procesa mensajes del broker, buscando específicamente Execution Reports (35=8)
    /// y actualiza el estado del RiskManager en consecuencia.
    pub fn handle_execution_report(msg: &str, risk_manager: &mut RiskManager) {
        // Verificamos que sea un Execution Report
        if !msg.contains("35=8") {
            return;
        }

        info!("[EXECUTOR] 📨 Execution Report recibido. Analizando estado...");

        // Extraer el OrdStatus (Tag 39)
        if let Some(status_part) = msg.split('|').find(|s| s.starts_with("39=")) {
            let status_val = status_part.replace("39=", "");

            match status_val.as_str() {
                "0" => {
                    info!("[EXECUTOR] ✅ Orden aceptada por el Broker (Status: NEW)");
                    risk_manager.set_status(TradeStatus::New);
                }
                "1" => {
                    info!("[EXECUTOR] ⚠️ Ejecución Parcial (Status: PARTIALLY_FILLED)");
                    risk_manager.set_status(TradeStatus::PartiallyFilled);
                }
                "2" => {
                    info!("[EXECUTOR] 🎯 ORDEN TOTALMENTE EJECUTADA (Status: FILLED)");
                    risk_manager.set_status(TradeStatus::Filled);
                }
                "4" | "C" => {
                    warn!("[EXECUTOR] 🛑 Orden cancelada/Expirada.");
                    risk_manager.set_status(TradeStatus::Idle);
                }
                "8" => {
                    error!("[EXECUTOR] ❌ ORDEN RECHAZADA por el Broker.");
                    risk_manager.set_status(TradeStatus::Rejected);
                    // Opcional: Volver a Idle después de un rechazo para permitir re-intento
                    risk_manager.set_status(TradeStatus::Idle);
                }
                _ => {
                    warn!("[EXECUTOR] ❓ Estado de orden desconocido: {}", status_val);
                }
            }
        }

        // Extraer el OrderID del Broker (Tag 37) para logs de auditoría
        if let Some(order_id_part) = msg.split('|').find(|s| s.starts_with("37=")) {
            let broker_id = order_id_part.replace("37=", "");
            info!("[EXECUTOR] ID del Broker asignado: {}", broker_id);
        }
    }

    /// Maneja el mensaje OrderCancelReject (35=9)
    pub fn handle_cancel_reject(msg: &str) {
        if msg.contains("35=9") {
            error!("[EXECUTOR] ⚠️ Error al intentar cancelar/modificar orden. El mercado se movió demasiado rápido.");
        }
    }
}
