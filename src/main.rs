use dotenv::dotenv;
use log::{error, info, warn};
use std::env;
use std::error::Error;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, interval};

mod features;
mod fix_engine;
mod network;
mod state;

use features::FeatureCollector;
use state::OrderBook;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    env_logger::init();

    info!("=== MOTOR FIX v0.9.5 - IA NORMALIZED DATA ===");

    // 1. Cargar Configuración
    let host = env::var("FIX_HOST")?;
    let port = env::var("FIX_PORT")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let sub_id = env::var("FIX_SENDER_SUB_ID")?;
    let password = env::var("FIX_PASSWORD")?;

    // 2. Inicializar Componentes de Inteligencia
    let mut engine = fix_engine::FixEngine::new();
    let mut order_book = OrderBook::new();
    let mut collector = FeatureCollector::new(100); // Ventana de 100 eventos

    // Variables de Métricas Temporales
    let mut last_velocity_calc = Instant::now();
    let mut tick_count = 0.0;
    let mut current_velocity = 0.0;
    let mut msg_count: u64 = 0;
    let mut mid_price_history: Vec<f64> = Vec::with_capacity(20);

    // 3. Conexión y Sesión
    let mut stream = network::connect_to_broker(&host, &port).await?;
    let mut response_buffer = [0u8; 16384];
    let mut seq_num: u64 = 1;

    // --- LOGON ---
    let mut fix_buffer = Vec::new();
    engine.build_logon(&mut fix_buffer, &sender_id, &target_id, &sub_id, &password);
    stream.write_all(&fix_buffer).await?;
    let _ = stream.read(&mut response_buffer).await?;
    info!("✅ Sesión FIX Activa.");
    seq_num += 1;

    // --- MARKET DATA REQUEST (Suscripción) ---
    let mut md_buffer = Vec::new();
    engine.build_market_data_request(&mut md_buffer, &sender_id, &target_id, seq_num, "1");
    stream.write_all(&md_buffer).await?;
    info!("📡 Suscripción enviada al Símbolo '1'");
    seq_num += 1;

    let mut hb_timer = interval(Duration::from_secs(25));

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
                    Ok(0) => { warn!("Conexión terminada por el servidor."); break; }
                    Ok(n) => {
                        let raw = String::from_utf8_lossy(&response_buffer[..n]);
                        let messages: Vec<&str> = raw.split("8=FIX.4.4").collect();

                        for content in messages {
                            if content.is_empty() { continue; }
                            let msg = content.replace('\x01', "|");

                            if msg.contains("|35=W|") || msg.contains("|35=X|") {
                                let entries: Vec<&str> = msg.split("|279=").collect();

                                for (i, entry) in entries.iter().enumerate() {
                                    if i == 0 { continue; }
                                    let fragment = format!("|279={}", entry);

                                    let action_val = extract_tag(&fragment, "279").unwrap_or(0.0);
                                    let action = if action_val == 2.0 { '2' } else if action_val == 1.0 { '1' } else { '0' };
                                    let side_val = extract_tag(&fragment, "269").unwrap_or(-1.0);
                                    let side = if side_val == 0.0 { '0' } else { '1' };
                                    let price = extract_tag(&fragment, "270").unwrap_or(0.0);
                                    let volume = extract_tag(&fragment, "271").unwrap_or(0.0);

                                    order_book.update(action, side, price, volume);
                                    tick_count += 1.0;
                                }

                                // --- PROCESAMIENTO DE CARACTERÍSTICAS (AI ENGINE) ---
                                msg_count += 1;

                                // Cálculo de Mid-Price y Volatilidad
                                if let Some(mid) = order_book.get_mid_price() {
                                    mid_price_history.push(mid);
                                    if mid_price_history.len() > 20 { mid_price_history.remove(0); }

                                    let volatility = if mid_price_history.len() > 1 {
                                        let mean = mid_price_history.iter().sum::<f64>() / mid_price_history.len() as f64;
                                        let variance = mid_price_history.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / mid_price_history.len() as f64;
                                        variance.sqrt() * 100000.0
                                    } else { 0.0 };

                                    // Cálculo de Velocidad (cada segundo)
                                    let elapsed = last_velocity_calc.elapsed().as_secs_f64();
                                    if elapsed >= 1.0 {
                                        current_velocity = tick_count / elapsed;
                                        tick_count = 0.0;
                                        last_velocity_calc = Instant::now();
                                    }

                                    // Alimentar el recolector
                                    collector.push_features(&order_book, current_velocity, volatility);

                                    // --- DIAGNÓSTICO CADA 10 MENSAJES ---
                                    if msg_count % 10 == 0 {
                                        let raw_v = collector.get_last_vector();
                                        let norm_v = collector.get_standardized_vector();

                                        info!("--------------------------------------------------");
                                        info!("📊 RAW  : {:.4?}", raw_v.to_vec());
                                        info!("🧪 NORM : {:.4?}", norm_v.to_vec());
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => { error!("Error en red FIX: {}", e); break; }
                }
            }
        }
    }
    Ok(())
}

fn extract_tag(msg: &str, tag: &str) -> Option<f64> {
    let pattern = format!("|{}=", tag);
    if let Some(start) = msg.find(&pattern) {
        let val_start = start + pattern.len();
        let end_offset = msg[val_start..].find('|').unwrap_or(msg[val_start..].len());
        let val_str = &msg[val_start..val_start + end_offset];
        return val_str.parse::<f64>().ok();
    }
    None
}

