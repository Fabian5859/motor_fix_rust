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
mod math_utils;
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
use state::{OrderBook, Position, TradeStatus};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    env_logger::init();

    info!("=== MOTOR FIX v2.4.0 - FULL BAYESIAN STACK (MODO SEGURO) ===");

    // 1. Inicialización de Componentes
    let mut engine = fix_engine::FixEngine::new();
    let mut order_book = OrderBook::new();
    let mut collector = FeatureCollector::new(100);
    let id_factory = IdGenerator::new();

    let mut risk_manager = RiskManager::new(5000.0);
    let mut brain = BayesianBrain::new(7, 12, 0.01);
    let mut g_filter = GaussianFilter::new(20, 1.5, 1.0);
    let bayes_net = BayesianNetwork::new(0.45);

    let mut executor = Executor::new();
    let mut pending_thesis: Option<Position> = None;

    let mut prediction_queue = VecDeque::new();
    let mut last_velocity_calc = Instant::now();
    let mut tick_count = 0.0;
    let mut current_velocity = 0.0;
    let mut msg_count: u64 = 0;

    // 2. Variables de Entorno
    let host = env::var("FIX_HOST")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let password = env::var("FIX_PASSWORD")?;
    let port_quote = env::var("FIX_PORT_QUOTE")?;
    let port_trade = env::var("FIX_PORT_TRADE")?;
    let symbol = env::var("FIX_SYMBOL").unwrap_or_else(|_| "1".to_string());

    // 3. Conexión de Red
    let mut quote_stream = network::connect_to_broker(&host, &port_quote).await?;
    let mut trade_stream = network::connect_to_broker(&host, &port_trade).await?;

    let mut quote_response_buffer = [0u8; 16384];
    let mut trade_response_buffer = [0u8; 16384];
    let mut quote_seq: u64 = 1;
    let mut trade_seq: u64 = 1;

    // Logon Quote
    let mut buf_q = Vec::new();
    engine.build_logon(&mut buf_q, &sender_id, &target_id, "QUOTE", &password);
    quote_stream.write_all(&buf_q).await?;

    // Logon Trade
    let mut buf_t = Vec::new();
    engine.build_logon(&mut buf_t, &sender_id, &target_id, "TRADE", &password);
    trade_stream.write_all(&buf_t).await?;

    // Suscripción Market Data
    quote_seq += 1;
    let mut md_buffer = Vec::new();
    engine.build_market_data_request(&mut md_buffer, &sender_id, &target_id, quote_seq, &symbol);
    quote_stream.write_all(&md_buffer).await?;

    let mut hb_timer = interval(Duration::from_secs(25));

    info!("🚀 Sistema Operativo. Esperando Market Data...");

    loop {
        tokio::select! {
            // Heartbeat
            _ = hb_timer.tick() => {
                quote_seq += 1;
                let mut hb = Vec::new();
                engine.build_heartbeat(&mut hb, &sender_id, &target_id, quote_seq);
                let _ = quote_stream.write_all(&hb).await;
            }

            // --- CANAL DE TRADE (Recepción de confirmaciones/rechazos) ---
            res_t = trade_stream.read(&mut trade_response_buffer) => {
                match res_t {
                    Ok(n) if n > 0 => {
                        let raw = String::from_utf8_lossy(&trade_response_buffer[..n]);
                        let msg = raw.replace('\x01', "|");
                        info!("[TRADE STREAM] Mensaje recibido: {}", msg);

                        // Procesamos el mensaje validando contra la tesis pendiente
                        executor.handle_execution_report(&msg, &mut risk_manager, &mut pending_thesis);
                    }
                    _ => {}
                }
            }

            // --- CANAL DE QUOTE (Lógica de Trading y Market Data) ---
            res_q = quote_stream.read(&mut quote_response_buffer) => {
                match res_q {
                    Ok(n) if n > 0 => {
                        let raw = String::from_utf8_lossy(&quote_response_buffer[..n]);
                        let messages: Vec<&str> = raw.split("8=FIX.4.4").collect();

                        for content in messages {
                            if content.is_empty() { continue; }
                            let msg = content.replace('\x01', "|");

                            process_lob_message(&msg, &mut order_book, &mut tick_count);

                            if let Some(mid) = order_book.get_mid_price() {
                                msg_count += 1;
                                g_filter.add_price(mid);
                                let noise = g_filter.compute_uncertainty();

                                // --- 1. GESTIÓN DE POSICIÓN ACTIVA ---
                                if risk_manager.status == TradeStatus::Filled {
                                    if executor.monitor_position(mid, noise, &mut risk_manager) {
                                        if let Some(pos) = &executor.active_position {
                                            trade_seq += 1;
                                            let side_exit = if pos.side == '1' { '2' } else { '1' };
                                            let mut exit_buf = Vec::new();
                                            let exit_id = id_factory.next_id();

                                            engine.build_order_request(&mut exit_buf, &sender_id, &target_id, trade_seq, &exit_id, &symbol, side_exit, pos.qty);
                                            let _ = trade_stream.write_all(&exit_buf).await;
                                            info!("💥 ORDEN DE CIERRE ENVIADA | ID: {} | Qty: {}", exit_id, pos.qty);
                                        }
                                    }
                                }

                                // --- 2. EXTRACCIÓN DE FEATURES Y CONTEXTO ---
                                let elapsed = last_velocity_calc.elapsed().as_secs_f64();
                                if elapsed >= 1.0 {
                                    current_velocity = tick_count / elapsed;
                                    tick_count = 0.0;
                                    last_velocity_calc = Instant::now();
                                }

                                let spread = (order_book.get_best_ask().unwrap_or(mid) - order_book.get_best_bid().unwrap_or(mid)).abs() * 100000.0;
                                let context = bayes_net.compute_context_score(spread, current_velocity, order_book.get_imbalance(), order_book.get_book_intensity());
                                collector.push_features(&order_book, current_velocity, noise, context);

                                let norm_v = collector.get_standardized_vector();

                                // --- 3. ENTRENAMIENTO Y SEÑAL DE ENTRADA ---
                                if !norm_v.is_empty() {
                                    prediction_queue.push_back((norm_v.clone(), mid));
                                    if prediction_queue.len() > 5 {
                                        if let Some((old_f, old_p)) = prediction_queue.pop_front() {
                                            let target = if mid > old_p { 1.0 } else { 0.0 };
                                            brain.train(&old_f, target);
                                        }
                                    }

                                    // Evaluamos señal cada 10 ticks para reducir ruido
                                    if msg_count % 10 == 0 {
                                        let b_out = brain.predict_bayesian(&norm_v, 20);
                                        let context_favorable = bayes_net.is_context_favorable(context);

                                        if msg_count % 500 == 0 {
                                            info!("[DIAGNÓSTICO] Status: {:?} | SNR: {:.2} | Context: {}", risk_manager.status, b_out.snr, context_favorable);
                                        }

                                        // Filtro de entrada: Solo si estamos Idle y el contexto es favorable
                                        if risk_manager.status == TradeStatus::Idle && noise < 0.7 && context_favorable {
                                            if b_out.snr > 0.40 {
                                                let side = if b_out.mu > 0.5 { '1' } else { '2' };

                                                if let Some((qty, tp, sl)) = risk_manager.evaluate_bayesian_trade(mid, b_out.mu, noise, b_out.sigma_epistemic, side) {
                                                    let cl_ord_id = id_factory.next_id();

                                                    // Creamos la tesis pendiente
                                                    pending_thesis = Some(Position {
                                                        cl_ord_id: cl_ord_id.clone(),
                                                        entry_price: mid,
                                                        side,
                                                        qty,
                                                        tp_price: tp,
                                                        sl_price: sl,
                                                        entry_mu: b_out.mu,
                                                        entry_sigma_total: noise + b_out.sigma_epistemic,
                                                        entry_snr: b_out.snr,
                                                    });

                                                    trade_seq += 1;
                                                    let mut buf = Vec::new();
                                                    engine.build_order_request(&mut buf, &sender_id, &target_id, trade_seq, &cl_ord_id, &symbol, side, qty);

                                                    info!("[NUEVA ORDEN] Enviando ID: {} | Side: {} | Qty: {} | TP: {:.5}", cl_ord_id, side, qty, tp);

                                                    match trade_stream.write_all(&buf).await {
                                                        Ok(_) => risk_manager.set_status(TradeStatus::PendingNew),
                                                        Err(e) => error!("[SOCKET ERROR] No se pudo enviar orden: {:?}", e),
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

// --- FUNCIONES HELPER ---

fn process_lob_message(msg: &str, order_book: &mut OrderBook, tick_count: &mut f64) {
    let separator = if msg.contains("|35=W|") {
        "|269="
    } else {
        "|279="
    };
    let entries: Vec<&str> = msg.split(separator).collect();

    for entry in entries.iter().skip(1) {
        let fragment = format!("{}{}", separator, entry);
        let action_val = extract_tag(&fragment, "279").unwrap_or(0.0);
        let side_val = extract_tag(&fragment, "269").unwrap_or(-1.0);
        let price = extract_tag(&fragment, "270").unwrap_or(0.0);
        let volume = extract_tag(&fragment, "271").unwrap_or(0.0);

        if side_val >= 0.0 {
            let side = if side_val == 0.0 { '0' } else { '1' };
            let action = if action_val == 2.0 { '2' } else { '1' };
            order_book.update(action, side, price, volume);
            *tick_count += 1.0;
        }
    }
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

