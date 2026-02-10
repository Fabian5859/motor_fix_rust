use dotenv::dotenv;
use log::{error, info};
use std::env;
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, interval};

// Módulos internos
mod fix_engine;
mod network;
mod state;

use state::PriceBuffer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. Inicialización de entorno y logs
    dotenv().ok();
    env_logger::init();

    info!("=== MOTOR FIX v0.4.0 - STATE MANAGEMENT ===");

    // 2. Cargar variables de entorno
    let host = env::var("FIX_HOST")?;
    let port = env::var("FIX_PORT")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let sub_id = env::var("FIX_SENDER_SUB_ID")?;
    let password = env::var("FIX_PASSWORD")?;

    // 3. Inicializar Motor FIX y Almacén de Datos
    let mut engine = fix_engine::FixEngine::new();
    let mut price_history = PriceBuffer::new(200); // Guardamos los últimos 200 ticks

    // 4. Conectar al servidor
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
        info!("✅ Logon Exitoso en servidor de precios.");
        seq_num += 1;
    } else {
        error!("❌ Fallo en Logon: {}", logon_res);
        return Ok(());
    }

    // --- SUSCRIPCIÓN (Market Data Request) ---
    let mut md_buffer = Vec::new();
    // Suscribimos al símbolo "1" (típicamente EURUSD en cTrader FIX)
    engine.build_market_data_request(&mut md_buffer, &sender_id, &target_id, seq_num, "1");
    stream.write_all(&md_buffer).await?;
    info!("📡 Suscripción enviada. Llenando buffer de memoria...");
    seq_num += 1;

    // Timer para Heartbeats (cada 25 segundos para mantener sesión viva)
    let mut hb_timer = interval(Duration::from_secs(25));
    hb_timer.tick().await; // El primer tick es inmediato, lo saltamos

    // --- BUCLE PRINCIPAL DE EVENTOS ---
    loop {
        tokio::select! {
            // Evento 1: Toca enviar Heartbeat
            _ = hb_timer.tick() => {
                let mut hb_buffer = Vec::new();
                engine.build_heartbeat(&mut hb_buffer, &sender_id, &target_id, seq_num);
                stream.write_all(&hb_buffer).await?;
                seq_num += 1;
            }

            // Evento 2: Llegan datos del servidor
            result = stream.read(&mut response_buffer) => {
                match result {
                    Ok(0) => {
                        error!("⚠️ El servidor cerró la conexión.");
                        break;
                    }
                    Ok(n) => {
                        let raw = String::from_utf8_lossy(&response_buffer[..n]);
                        let readable = raw.replace("\x01", "|");

                        // Procesar mensajes de Market Data (35=W o 35=X)
                        if readable.contains("|35=W|") || readable.contains("|35=X|") {

                            // 1. Identificar lado (Bid/Ask)
                            let side = if readable.contains("|269=0|") { "BID" } else { "ASK" };

                            // 2. Extraer el precio (Tag 270)
                            if let Some(pos_270) = readable.find("|270=") {
                                let start = pos_270 + 5;
                                if let Some(end_offset) = readable[start..].find('|') {
                                    let price_str = &readable[start..start + end_offset];

                                    // 3. Convertir a número y guardar en el State
                                    if let Ok(price) = price_str.parse::<f64>() {
                                        price_history.add_price(price);

                                        // Mostramos el precio y cuántas muestras llevamos para Gauss
                                        info!("📈 EURUSD {} -> {} [Buffer: {}/200]",
                                              side,
                                              price,
                                              price_history.get_count()
                                        );
                                    }
                                }
                            }
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

    info!("Fin de la ejecución del motor.");
    Ok(())
}

