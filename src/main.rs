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

    info!("=== MOTOR FIX v0.3.0 - PARSER DE PRECIOS ===");

    let host = env::var("FIX_HOST")?;
    let port = env::var("FIX_PORT")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let sub_id = env::var("FIX_SENDER_SUB_ID")?;
    let password = env::var("FIX_PASSWORD")?;

    let mut engine = fix_engine::FixEngine::new();
    let mut stream = network::connect_to_broker(&host, &port).await?;

    let mut response_buffer = [0u8; 8192];
    let mut seq_num: u64 = 1;

    // --- LOGON ---
    let mut fix_buffer = Vec::new();
    engine.build_logon(&mut fix_buffer, &sender_id, &target_id, &sub_id, &password);
    stream.write_all(&fix_buffer).await?;

    let n = stream.read(&mut response_buffer).await?;
    let logon_res = String::from_utf8_lossy(&response_buffer[..n]).replace("\x01", "|");

    if logon_res.contains("|35=A|") {
        info!("✅ Logon Exitoso.");
        seq_num += 1;
    } else {
        error!("❌ Fallo en Logon: {}", logon_res);
        return Ok(());
    }

    // --- SUSCRIPCIÓN ---
    let mut md_buffer = Vec::new();
    engine.build_market_data_request(&mut md_buffer, &sender_id, &target_id, seq_num, "1");
    stream.write_all(&md_buffer).await?;
    info!("📡 Suscripción enviada. Extrayendo precios en tiempo real...");
    seq_num += 1;

    let mut hb_timer = interval(Duration::from_secs(25));
    hb_timer.tick().await;

    loop {
        tokio::select! {
            _ = hb_timer.tick() => {
                let mut hb_buffer = Vec::new();
                engine.build_heartbeat(&mut hb_buffer, &sender_id, &target_id, seq_num);
                stream.write_all(&hb_buffer).await?;
                seq_num += 1;
            }

            result = stream.read(&mut response_buffer) => {
                match result {
                    Ok(0) => { error!("⚠️ Conexión cerrada."); break; }
                    Ok(n) => {
                        let raw = String::from_utf8_lossy(&response_buffer[..n]);
                        let readable = raw.replace("\x01", "|");

                        // PARSER LÓGICO: Detectar Snapshot (W) o Refresh (X)
                        if readable.contains("|35=W|") || readable.contains("|35=X|") {
                            // Extraer Tipo (Tag 269: 0=Bid, 1=Ask)
                            let side = if readable.contains("|269=0|") { "BID" } else { "ASK" };

                            // Extraer Precio (Tag 270)
                            if let Some(pos_270) = readable.find("|270=") {
                                let start = pos_270 + 5;
                                if let Some(end_offset) = readable[start..].find('|') {
                                    let price_str = &readable[start..start + end_offset];

                                    // Intentar convertir a número
                                    if let Ok(price) = price_str.parse::<f64>() {
                                        info!("📈 EURUSD {} -> {}", side, price);
                                        // TODO: Aquí alimentaremos el modelo de Gauss en la siguiente fase
                                    }
                                }
                            }
                        } else if readable.contains("|35=0|") {
                            // Heartbeat silencioso para no ensuciar la consola de precios
                        }
                    }
                    Err(e) => { error!("❌ Error: {}", e); break; }
                }
            }
        }
    }

    Ok(())
}

