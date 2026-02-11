use dotenv::dotenv;
use log::{error, info, warn};
use std::collections::VecDeque;
use std::env;
use std::error::Error;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{interval, Duration};

mod bayesian;
mod brain;
mod features;
mod fix_engine;
mod gaussian;
mod id_gen;
mod network;
mod state;
mod risk; // NUEVO MODULO

use bayesian::BayesianNetwork;
use brain::BayesianBrain;
use features::FeatureCollector;
use gaussian::GaussianFilter;
use id_gen::IdGenerator;
use state::{OrderBook, TradeStatus}; // Importamos el Enum de estado
use risk::RiskManager; // Importamos el gestor de riesgo

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    env_logger::init();

    info!("=== MOTOR FIX v1.4.2 - RISK MANAGER ACTIVE ===");

    // 1. Inicialización de Componentes
    let mut engine = fix_engine::FixEngine::new();
    let mut order_book = OrderBook::new();
    let mut collector = FeatureCollector::new(100);
    let id_factory = IdGenerator::new();
    
    // Configuramos el Risk Manager: Max 5000 unidades (0.05 lotes)
    let mut risk_manager = RiskManager::new(5000.0);

    // Arquitectura: 7 Inputs, 12 Hidden, 0.01 LR
    let mut brain = BayesianBrain::new(7, 12, 0.01);

    let mut g_filter = GaussianFilter::new(20, 1.5, 1.0);
    let bayes_net = BayesianNetwork::new(0.45);

    let mut prediction_queue = VecDeque::new();
    let mut last_velocity_calc = Instant::now();
    let mut tick_count = 0.0;
    let mut current_velocity = 0.0;
    let mut msg_count: u64 = 0;

    // 2. Conexión FIX
    let host = env::var("FIX_HOST")?;
    let port = env::var("FIX_PORT")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let sub_id = env::var("FIX_SENDER_SUB_ID")?;
    let password = env::var("FIX_PASSWORD")?;

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
    info!("📡 Suscripción enviada. Procesando profundidad de libro...");
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
                                    g_filter.add_price(mid);

                                    let spread = (order_book.get_best_ask().unwrap_or(mid) - order_book.get_best_bid().unwrap_or(mid)).abs() * 100000.0;
                                    let imbalance = order_book.get_imbalance();
                                    let intensity = order_book.get_book_intensity();
                                    let noise = g_filter.compute_uncertainty();
                                    let context = bayes_net.compute_context_score(spread, current_velocity, imbalance, intensity);

                                    let elapsed = last_velocity_calc.elapsed().as_secs_f64();
                                    if elapsed >= 1.0 {
                                        current_velocity = tick_count / elapsed;
                                        tick_count = 0.0;
                                        last_velocity_calc = Instant::now();
                                    }

                                    collector.push_features(&order_book, current_velocity, noise, context);
                                    let norm_v = collector.get_standardized_vector();

                                    if !norm_v.is_empty() {
                                        prediction_queue.push_back((norm_v.clone(), mid));

                                        if prediction_queue.len() > 5 {
                                            if let Some((old_features, old_price)) = prediction_queue.pop_front() {
                                                let target = if mid > old_price { 1.0 } else { 0.0 };
                                                brain.train(&old_features, target);

                                                if msg_count % 5 == 0 {
                                                    let (prob, brain_uncertainty) = brain.predict_with_uncertainty(&norm_v);

                                                    let is_safe = noise < 0.70;
                                                    let is_sane = bayes_net.is_context_favorable(context);
                                                    let brain_conflicts = brain_uncertainty > 0.85;

                                                    let signal = if is_safe && is_sane && !brain_conflicts {
                                                        if prob > 0.75 { "🚀 BUY" }
                                                        else if prob < 0.25 { "📉 SELL" }
                                                        else { "⏳ WAIT" }
                                                    } else {
                                                        "🚫 BLOCKED"
                                                    };

                                                    if signal != "🚫 BLOCKED" && signal != "⏳ WAIT" {
                                                        info!("P: {:.1}% | B-UNCER: {:.2} | RUIDO: {:.2} | CTXT: {:.2} | [{}]",
                                                              prob * 100.0, brain_uncertainty, noise, context, signal);
                                                    }

                                                    // --- LÓGICA DE RIESGO Y EJECUCIÓN ---
                                                    if signal == "🚀 BUY" || signal == "📉 SELL" {
                                                        let side = if signal == "🚀 BUY" { '1' } else { '2' };
                                                        let qty = 1000.0;

                                                        if risk_manager.validate_execution(qty) {
                                                            let cl_ord_id = id_factory.next_id();
                                                            let mut order_buffer = Vec::new();
                                                            
                                                            engine.build_order_request(
                                                                &mut order_buffer,
                                                                &sender_id,
                                                                &target_id,
                                                                seq_num,
                                                                &cl_ord_id,
                                                                "1", 
                                                                side,
                                                                qty
                                                            );

                                                            let fix_msg = String::from_utf8_lossy(&order_buffer).replace('\x01', "|");
                                                            info!("📦 ORDEN APROBADA POR RISK MANAGER: {}", fix_msg);
                                                            
                                                            // Bloqueamos el estado para evitar ráfagas
                                                            risk_manager.set_status(TradeStatus::PendingNew);
                                                            
                                                            // Nota: El seq_num se incrementará cuando enviemos realmente en la Fase 4-03
                                                            // seq_num += 1; 
                                                        } else {
                                                            // Solo logueamos el rechazo si no es por estar ya en una posición (para no inundar el log)
                                                            if risk_manager.status == TradeStatus::Idle {
                                                                warn!("⚠️ SEÑAL {} RECHAZADA POR COOLDOWN O LÍMITES", signal);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
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
    let pattern = format!("|{}=", tag);
    if let Some(start) = msg.find(&pattern) {
        let val_start = start + pattern.len();
        let end_offset = msg[val_start..].find('|').unwrap_or(msg[val_start..].len());
        let val_str = &msg[val_start..val_start + end_offset];
        return val_str.parse::<f64>().ok();
    }
    None
}
