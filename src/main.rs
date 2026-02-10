use dotenv::dotenv;
use log::{error, info, warn};
use std::env;
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, interval};

mod fix_engine;
mod network;
mod state;

use state::OrderBook;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. Cargar entorno y logger
    dotenv().ok();
    env_logger::init();

    info!("=== MOTOR FIX v0.8.0 - LOB PROFESIONAL (VOLUMEN REAL) ===");

    // 2. Configuración
    let host = env::var("FIX_HOST")?;
    let port = env::var("FIX_PORT")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let sub_id = env::var("FIX_SENDER_SUB_ID")?;
    let password = env::var("FIX_PASSWORD")?;

    let mut engine = fix_engine::FixEngine::new();
    let mut order_book = OrderBook::new();

    // 3. Conexión
    let mut stream = network::connect_to_broker(&host, &port).await?;
    let mut response_buffer = [0u8; 16384];
    let mut seq_num: u64 = 1;

    // --- SESIÓN: LOGON ---
    let mut fix_buffer = Vec::new();
    engine.build_logon(&mut fix_buffer, &sender_id, &target_id, &sub_id, &password);
    stream.write_all(&fix_buffer).await?;
    let _ = stream.read(&mut response_buffer).await?;
    info!("✅ Sesión FIX Iniciada. Recibiendo datos L2...");
    seq_num += 1;

    // --- SUSCRIPCIÓN ---
    let mut md_buffer = Vec::new();
    engine.build_market_data_request(&mut md_buffer, &sender_id, &target_id, seq_num, "1");
    stream.write_all(&md_buffer).await?;
    seq_num += 1;

    let mut hb_timer = interval(Duration::from_secs(25));

    // --- BUCLE DE PROCESAMIENTO ---
    loop {
        tokio::select! {
            _ = hb_timer.tick() => {
                let mut hb_buffer = Vec::new();
                engine.build_heartbeat(&mut hb_buffer, &sender_id, &target_id, seq_num);
                let _ = stream.write_all(&hb_buffer).await;
                seq_num += 1;
            }

            result = stream.read(&mut response_buffer) => {
                match result {
                    Ok(0) => { warn!("Conexión cerrada por el servidor."); break; }
                    Ok(n) => {
                        let raw = String::from_utf8_lossy(&response_buffer[..n]);
                        // Separamos mensajes FIX por el inicio estándar
                        let messages: Vec<&str> = raw.split("8=FIX.4.4").collect();

                        for content in messages {
                            if content.is_empty() { continue; }
                            let msg = content.replace('\x01', "|");

                            // Procesamos Market Data Incremental (X) y Snapshots (W)
                            if msg.contains("|35=W|") || msg.contains("|35=X|") {
                                // cTrader usa 279 para indicar la acción (0=New, 1=Change, 2=Delete)
                                let entries: Vec<&str> = msg.split("|279=").collect();

                                for (i, entry) in entries.iter().enumerate() {
                                    if i == 0 { continue; }
                                    let fragment = format!("|279={}", entry);

                                    let action_val = extract_tag(&fragment, "279").unwrap_or(0.0);
                                    let action = if action_val == 2.0 { '2' } else if action_val == 1.0 { '1' } else { '0' };

                                    let side_val = extract_tag(&fragment, "269").unwrap_or(-1.0);
                                    let side = if side_val == 0.0 { '0' } else { '1' };

                                    let price = extract_tag(&fragment, "270").unwrap_or(0.0);
                                    let volume = extract_tag(&fragment, "271").unwrap_or(1.0);

                                    if action == '2' {
                                        order_book.update('2', side, price, 0.0);
                                    } else if price > 0.0 {
                                        order_book.update(action, side, price, volume);
                                    }
                                }

                                // Cálculo de Mid-Price e Imbalance
                                if let (Some(b), Some(a)) = (order_book.get_best_bid(), order_book.get_best_ask()) {
                                    if b < a {
                                        let imb = order_book.get_imbalance();
                                        info!("📊 LOB | BID: {:.5} | ASK: {:.5} | IMB: {:+.4} | SPR: {:.1}",
                                              b, a, imb, (a - b) * 100000.0);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => { error!("Error de lectura: {}", e); break; }
                }
            }
        }
    }
    Ok(())
}

/// Extrae el valor de un tag FIX como f64
fn extract_tag(msg: &str, tag: &str) -> Option<f64> {
    let patterns = [format!("|{}=", tag), format!("{}=", tag)];
    for pattern in patterns {
        if let Some(start) = msg.find(&pattern) {
            let val_start = start + pattern.len();
            let end_offset = msg[val_start..].find('|').unwrap_or(msg[val_start..].len());
            let val_str = &msg[val_start..val_start + end_offset];
            return val_str.parse::<f64>().ok();
        }
    }
    None
}

