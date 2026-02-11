use dotenv::dotenv;
use log::{error, info, warn};
use std::collections::VecDeque;
use std::env;
use std::error::Error;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, interval};

mod features;
mod fix_engine;
mod gaussian;
mod model;
mod network;
mod state; // Nuevo módulo

use features::FeatureCollector;
use gaussian::GaussianFilter;
use model::LogisticModel;
use state::OrderBook; // Importación del filtro

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    env_logger::init();

    info!("=== MOTOR FIX v1.1.0 - GAUSSIAN FILTER ENABLED ===");

    // 1. Inicialización de Componentes
    let mut engine = fix_engine::FixEngine::new();
    let mut order_book = OrderBook::new();
    let mut collector = FeatureCollector::new(100);
    let mut ai_model = LogisticModel::new(4, 0.05);

    // Filtro Gaussiano: Ventana de 20 precios, suavizado 1.5, varianza 1.0
    let mut g_filter = GaussianFilter::new(20, 1.5, 1.0);

    let mut prediction_queue = VecDeque::new();

    let mut last_velocity_calc = Instant::now();
    let mut tick_count = 0.0;
    let mut current_velocity = 0.0;
    let mut msg_count: u64 = 0;
    let mut mid_price_history: Vec<f64> = Vec::with_capacity(20);

    // 2. Variables de Entorno
    let host = env::var("FIX_HOST")?;
    let port = env::var("FIX_PORT")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let sub_id = env::var("FIX_SENDER_SUB_ID")?;
    let password = env::var("FIX_PASSWORD")?;

    // 3. Conexión de Red
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

    // --- SUSCRIPCIÓN ---
    let mut md_buffer = Vec::new();
    engine.build_market_data_request(&mut md_buffer, &sender_id, &target_id, seq_num, "1");
    stream.write_all(&md_buffer).await?;
    info!("📡 Suscripción enviada. Esperando Market Data...");
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
                    Ok(0) => { warn!("Conexión cerrada."); break; }
                    Ok(n) => {
                        let raw = String::from_utf8_lossy(&response_buffer[..n]);
                        let messages: Vec<&str> = raw.split("8=FIX.4.4").collect();

                        for content in messages {
                            if content.is_empty() { continue; }
                            let msg = content.replace('\x01', "|");

                            if msg.contains("|35=W|") || msg.contains("|35=X|") {
                                let entries: Vec<&str> = msg.split("|279=").collect();

                                for entry in entries.iter().skip(1) {
                                    let fragment = format!("|279={}", entry);
                                    let side = if extract_tag(&fragment, "269").unwrap_or(-1.0) == 0.0 { '0' } else { '1' };
                                    let price = extract_tag(&fragment, "270").unwrap_or(0.0);
                                    let volume = extract_tag(&fragment, "271").unwrap_or(0.0);

                                    order_book.update('1', side, price, volume);
                                    tick_count += 1.0;
                                }

                                if let Some(mid) = order_book.get_mid_price() {
                                    msg_count += 1;

                                    // --- FILTRO GAUSSIANO (ACTUALIZACIÓN) ---
                                    g_filter.add_price(mid);
                                    let uncertainty = g_filter.compute_uncertainty();

                                    // 1. Características base
                                    mid_price_history.push(mid);
                                    if mid_price_history.len() > 20 { mid_price_history.remove(0); }
                                    let volatility = if mid_price_history.len() > 1 {
                                        let mean = mid_price_history.iter().sum::<f64>() / mid_price_history.len() as f64;
                                        mid_price_history.iter().map(|p| (p - mean).powi(2)).sum::<f64>().sqrt() * 1000.0
                                    } else { 0.0 };

                                    let elapsed = last_velocity_calc.elapsed().as_secs_f64();
                                    if elapsed >= 1.0 {
                                        current_velocity = tick_count / elapsed;
                                        tick_count = 0.0;
                                        last_velocity_calc = Instant::now();
                                    }

                                    // 2. Normalización
                                    collector.push_features(&order_book, current_velocity, volatility);
                                    let norm_v = collector.get_standardized_vector();

                                    // 3. APRENDIZAJE Y PREDICCIÓN CON FILTRO
                                    prediction_queue.push_back((norm_v.clone(), mid));

                                    if prediction_queue.len() > 5 {
                                        if let Some((old_features, old_price)) = prediction_queue.pop_front() {
                                            let target = if mid > old_price { 1.0 } else { 0.0 };
                                            let loss = ai_model.train(&old_features, target);

                                            if msg_count % 5 == 0 {
                                                let prob = ai_model.predict(&norm_v);

                                                // --- LÓGICA DE DECISIÓN CON GAUSSIANO ---
                                                let status = if uncertainty > 0.70 { "🚫 RUIDO ALTO" } else { "✅ SEGURO" };

                                                info!("🤖 PRED: {:.1}% ALZA | RUIDO: {:.2} ({}) | LOSS: {:.4}",
                                                    prob * 100.0, uncertainty, status, loss);
                                            }
                                        }
                                    }
                                }
                            } else if msg.contains("|35=0|") {
                                // Heartbeat silencioso
                            } else {
                                let msg_type = extract_tag(&msg, "35");
                                info!("📩 FIX Msg Type: {:?}", msg_type);
                            }
                        }
                    }
                    Err(e) => { error!("Error FIX: {}", e); break; }
                }
            }
        }
    }
    Ok(())
}

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

