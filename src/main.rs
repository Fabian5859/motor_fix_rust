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

    info!("=== MOTOR FIX v0.2.0 - MODO OPERATIVO (FASE 2) ===");

    let host = env::var("FIX_HOST")?;
    let port = env::var("FIX_PORT")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let sub_id = env::var("FIX_SENDER_SUB_ID")?;
    let password = env::var("FIX_PASSWORD")?;

    let mut engine = fix_engine::FixEngine::new();
    let mut stream = network::connect_to_broker(&host, &port).await?;

    // 1. Handshake Inicial (Logon)
    let mut fix_buffer = Vec::new();
    engine.build_logon(&mut fix_buffer, &sender_id, &target_id, &sub_id, &password);
    stream.write_all(&fix_buffer).await?;

    // Esperar respuesta inicial de Logon
    let mut response_buffer = [0u8; 4096];
    let n = stream.read(&mut response_buffer).await?;
    let logon_res = String::from_utf8_lossy(&response_buffer[..n]).replace("\x01", "|");

    if logon_res.contains("|35=A|") {
        info!("✅ Logon Exitoso. Iniciando persistencia...");
    } else {
        error!("❌ Fallo en Logon inicial: {}", logon_res);
        return Ok(());
    }

    // 2. BUCLE INFINITO: Heartbeats y Escucha de Datos
    let mut seq_num: u64 = 2;
    let mut hb_timer = interval(Duration::from_secs(25));
    // El primer tick de interval ocurre inmediatamente, lo saltamos para no enviar HB justo tras el Logon
    hb_timer.tick().await;

    info!("Bot en línea. Escuchando mercado...");

    loop {
        tokio::select! {
            // Tarea A: Tick del temporizador para Heartbeat
            _ = hb_timer.tick() => {
                let mut hb_buffer = Vec::new();
                engine.build_heartbeat(&mut hb_buffer, &sender_id, &target_id, seq_num);
                stream.write_all(&hb_buffer).await?;
                info!("💓 Heartbeat enviado (seq={})", seq_num);
                seq_num += 1;
            }

            // Tarea B: Datos entrantes del servidor
            result = stream.read(&mut response_buffer) => {
                match result {
                    Ok(0) => {
                        error!("El servidor cerró la conexión (EOF).");
                        break;
                    }
                    Ok(n) => {
                        let raw = String::from_utf8_lossy(&response_buffer[..n]);
                        let readable = raw.replace("\x01", "|");
                        info!("📥 Mensaje Recibido: {}", readable);

                        // Aquí procesaremos los precios en la siguiente tarea
                    }
                    Err(e) => {
                        error!("Error de lectura en el stream: {}", e);
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}
