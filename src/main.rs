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
mod executor;
mod features;
mod fix_engine;
mod gaussian;
mod id_gen;
mod network;
mod risk;
mod state;

use bayesian::BayesianNetwork;
use brain::BayesianBrain;
use executor::Executor;
use features::FeatureCollector;
use gaussian::GaussianFilter;
use id_gen::IdGenerator;
use risk::RiskManager;
use state::{OrderBook, TradeStatus};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    env_logger::init();

    info!("=== MOTOR FIX v2.2.0 - DEEP LOB REPEATING GROUPS ===");

    // 1. Inicialización de Componentes
    let mut engine = fix_engine::FixEngine::new();
    let mut order_book = OrderBook::new();
    let mut collector = FeatureCollector::new(100);
    let id_factory = IdGenerator::new();

    let mut risk_manager = RiskManager::new(5000.0);
    let mut brain = BayesianBrain::new(7, 12, 0.01);
    let mut g_filter = GaussianFilter::new(20, 1.5, 1.0);
    let bayes_net = BayesianNetwork::new(0.45);

    let mut prediction_queue = VecDeque::new();
    let mut last_velocity_calc = Instant::now();
    let mut tick_count = 0.0;
    let mut current_velocity = 0.0;
    let mut msg_count: u64 = 0;

    // 2. Carga de Variables de Entorno
    let host = env::var("FIX_HOST")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let password = env::var("FIX_PASSWORD")?;

    let port_quote = env::var("FIX_PORT_QUOTE")?;
    let port_trade = env::var("FIX_PORT_TRADE")?;
    let sub_id_quote = env::var("FIX_SENDER_SUB_ID_QUOTE")?;
    let sub_id_trade = env::var("FIX_SENDER_SUB_ID_TRADE")?;
    let symbol = env::var("FIX_SYMBOL").unwrap_or_else(|_| "1".to_string());

    // 3. Establecer Conexiones
    info!(
        "🔗 Conectando QUOTE (Port: {}) y TRADE (Port: {})...",
        port_quote, port_trade
    );
    let mut quote_stream = network::connect_to_broker(&host, &port_quote).await?;
    let mut trade_stream = network::connect_to_broker(&host, &port_trade).await?;

    let mut quote_response_buffer = [0u8; 16384];
    let mut trade_response_buffer = [0u8; 16384];

    let mut quote_seq: u64 = 1;
    let mut trade_seq: u64 = 1;

    // LOGON QUOTE
    let mut buf_q = Vec::new();
    engine.build_logon(&mut buf_q, &sender_id, &target_id, &sub_id_quote, &password);
    quote_stream.write_all(&buf_q).await?;
    let _ = quote_stream.read(&mut quote_response_buffer).await?;
    info!("✅ Sesión QUOTE Activa.");

    // LOGON TRADE
    let mut buf_t = Vec::new();
    engine.build_logon(&mut buf_t, &sender_id, &target_id, &sub_id_trade, &password);
    trade_stream.write_all(&buf_t).await?;
    let _ = trade_stream.read(&mut trade_response_buffer).await?;
    info!("✅ Sesión TRADE Activa.");

    // SUSCRIPCIÓN
    quote_seq += 1;
    let mut md_buffer = Vec::new();
    engine.build_market_data_request(&mut md_buffer, &sender_id, &target_id, quote_seq, &symbol);
    quote_stream.write_all(&md_buffer).await?;
    info!("📡 Suscripción Deep LOB enviada para: {}", symbol);

    let mut hb_timer = interval(Duration::from_secs(25));

    loop {
        tokio::select! {
            _ = hb_timer.tick() => {
                quote_seq += 1;
                let mut hb_q = Vec::new();
                engine.build_heartbeat(&mut hb_q, &sender_id, &target_id, quote_seq);
                let _ = quote_stream.write_all(&hb_q).await;

                trade_seq += 1;
                let mut hb_t = Vec::new();
                engine.build_heartbeat(&mut hb_t, &sender_id, &target_id, trade_seq);
                let _ = trade_stream.write_all(&hb_t).await;
            }

            res_t = trade_stream.read(&mut trade_response_buffer) => {
                match res_t {
                    Ok(0) => { warn!("Canal de Trading cerrado."); break; }
                    Ok(n) => {
                        let raw = String::from_utf8_lossy(&trade_response_buffer[..n]);
                        let msg = raw.replace('\x01', "|");
                        info!("📥 TRADE MSG: {}", msg);
                        Executor::handle_execution_report(&msg, &mut risk_manager);
                        Executor::handle_cancel_reject(&msg);
                    }
                    Err(e) => { error!("Error en canal TRADE: {}", e); break; }
                }
            }

            res_q = quote_stream.read(&mut quote_response_buffer) => {
                match res_q {
                    Ok(0) => { warn!("Canal de Precios cerrado."); break; }
                    Ok(n) => {
                        let raw = String::from_utf8_lossy(&quote_response_buffer[..n]);
                        info!("🔍 INSPECT QUOTE ({} bytes): {}", n, raw.replace('\x01', "|"));

                        let messages: Vec<&str> = raw.split("8=FIX.4.4").collect();

                        for content in messages {
                            if content.is_empty() { continue; }
                            let msg = content.replace('\x01', "|");

                            if msg.contains("|35=W|") || msg.contains("|35=X|") {
                                // AJUSTE GRUPOS REPETITIVOS:
                                // 279 (UpdateAction) para Incremental, 269 (EntryType) para Snapshot
                                let separator = if msg.contains("|35=W|") { "|269=" } else { "|279=" };
                                let entries: Vec<&str> = msg.split(separator).collect();

                                for entry in entries.iter().skip(1) {
                                    let fragment = format!("{}{}", separator, entry);

                                    let action_val = extract_tag(&fragment, "279").unwrap_or(0.0); // Default New
                                    let side_val = extract_tag(&fragment, "269").unwrap_or(-1.0);
                                    let price = extract_tag(&fragment, "270").unwrap_or(0.0);
                                    let volume = extract_tag(&fragment, "271").unwrap_or(0.0);

                                    if side_val >= 0.0 {
                                        let side = if side_val == 0.0 { '0' } else { '1' };
                                        // 2.0 en Tag 279 significa DELETE en FIX
                                        let action = if action_val == 2.0 { '2' } else { '1' };

                                        order_book.update(action, side, price, volume);
                                        tick_count += 1.0;
                                    }
                                }

                                // Tras procesar TODO el mensaje (con todos sus niveles), ejecutamos la IA
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
                                                    } else { "🚫 BLOCKED" };

                                                    if signal != "🚫 BLOCKED" && signal != "⏳ WAIT" {
                                                        info!("🤖 IA SIGNAL: P: {:.1}% | UNC: {:.2} | RUIDO: {:.2} | CTXT: {:.2} | [{}]",
                                                              prob * 100.0, brain_uncertainty, noise, context, signal);

                                                        let side = if signal == "🚀 BUY" { '1' } else { '2' };
                                                        let qty = 1000.0;

                                                        if risk_manager.validate_execution(qty) {
                                                            trade_seq += 1;
                                                            let cl_ord_id = id_factory.next_id();
                                                            let mut order_buffer = Vec::new();

                                                            engine.build_order_request(
                                                                &mut order_buffer,
                                                                &sender_id,
                                                                &target_id,
                                                                trade_seq,
                                                                &cl_ord_id,
                                                                &symbol,
                                                                side,
                                                                qty
                                                            );

                                                            if let Err(e) = trade_stream.write_all(&order_buffer).await {
                                                                error!("❌ Socket Trade Error: {}", e);
                                                            } else {
                                                                info!("📤 ORDEN ENVIADA CANAL TRADE -> ID: {}", cl_ord_id);
                                                                risk_manager.set_status(TradeStatus::PendingNew);
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
                    Err(e) => { error!("Error en canal QUOTE: {}", e); break; }
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
        let fragment = &msg[val_start..];
        let end_offset = fragment.find('|').unwrap_or(fragment.len());
        let val_str = &fragment[..end_offset];
        return val_str.parse::<f64>().ok();
    }
    None
}

