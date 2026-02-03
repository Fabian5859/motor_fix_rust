use dotenv::dotenv;
use log::{error, info};
use std::env;
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, interval};

mod fix_engine;
mod network;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    env_logger::init();

    info!("=== MOTOR FIX v0.2.0 - CAPTURA DE DATOS (DEPTH) ===");

    // 1. Cargar configuración
    let host = env::var("FIX_HOST")?;
    let port = env::var("FIX_PORT")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let sub_id = env::var("FIX_SENDER_SUB_ID")?;
    let password = env::var("FIX_PASSWORD")?;

    let mut engine = fix_engine::FixEngine::new();
    let mut stream = network::connect_to_broker(&host, &port).await?;

    // Buffer grande (8KB) para manejar ráfagas de profundidad de mercado
    let mut response_buffer = [0u8; 8192];
    let mut seq_num: u64 = 1;

    // --- PASO 1: LOGON ---
    let mut fix_buffer = Vec::new();
    engine.build_logon(&mut fix_buffer, &sender_id, &target_id, &sub_id, &password);
    stream.write_all(&fix_buffer).await?;

    let n = stream.read(&mut response_buffer).await?;
    let logon_res = String::from_utf8_lossy(&response_buffer[..n]).replace("\x01", "|");

    if logon_res.contains("|35=A|") {
        info!("✅ Logon Exitoso.");
        seq_num += 1; // Incrementamos a 2 para el siguiente mensaje
    } else {
        error!("❌ Fallo en Logon: {}", logon_res);
        return Ok(());
    }

    // --- PASO 2: SUSCRIPCIÓN A MARKET DATA ---
    // Pedimos EURUSD (ID = 1) con profundidad total (Full Book)
    let mut md_buffer = Vec::new();
    engine.build_market_data_request(&mut md_buffer, &sender_id, &target_id, seq_num, "1");
    stream.write_all(&md_buffer).await?;
    info!("📡 Suscripción enviada para EURUSD (ID: 1). Esperando flujo de datos...");
    seq_num += 1; // Incrementamos a 3 para el primer Heartbeat

    // --- PASO 3: BUCLE DE EVENTOS (HEARTBEATS + TICKS) ---
    let mut hb_timer = interval(Duration::from_secs(25));
    hb_timer.tick().await; // Saltamos el primer tick inmediato para cumplir los 25s

    loop {
        tokio::select! {
            // Tarea A: Mantener la sesión viva
            _ = hb_timer.tick() => {
                let mut hb_buffer = Vec::new();
                engine.build_heartbeat(&mut hb_buffer, &sender_id, &target_id, seq_num);
                stream.write_all(&hb_buffer).await?;
                info!("💓 Heartbeat enviado (seq={})", seq_num);
                seq_num += 1;
            }

            // Tarea B: Recibir Ticks y Snapshot del Broker
            result = stream.read(&mut response_buffer) => {
                match result {
                    Ok(0) => {
                        error!("⚠️ Conexión cerrada por el broker.");
                        break;
                    }
                    Ok(n) => {
                        let raw = String::from_utf8_lossy(&response_buffer[..n]);
                        let readable = raw.replace("\x01", "|");

                        // Clasificación visual de los datos entrantes
                        if readable.contains("|35=W|") {
                            info!("📸 [SNAPSHOT] Recibida profundidad inicial completa.");
                        } else if readable.contains("|35=X|") {
                            // Este es el que alimentará a Gauss
                            info!("⚡ [TICK] Cambio en el libro de órdenes.");
                        } else if readable.contains("|35=0|") {
                            info!("📥 Heartbeat del servidor recibido.");
                        } else if readable.contains("|35=h|") {
                            info!("ℹ️ Mensaje de estado de sesión recibido.");
                        } else {
                            info!("📥 Otro Mensaje: {}", readable);
                        }
                    }
                    Err(e) => {
                        error!("❌ Error de lectura en el stream: {}", e);
                        break;
                    }
                }
            }
        }
    }

    info!("Cerrando motor de trading...");
    Ok(())
}
