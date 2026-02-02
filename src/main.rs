use dotenv::dotenv;
use log::{error, info};
use std::env;
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt}; // Importante: AsyncReadExt para la Tarea 5
use tokio::time::Duration;

mod fix_engine;
mod network;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. Inicialización de entorno y logs
    dotenv().ok();
    env_logger::init();

    info!("=== MOTOR FIX v0.1.0 - INICIANDO HANDSHAKE (FASE 1) ===");

    // 2. Carga de credenciales desde el archivo .env
    let host = env::var("FIX_HOST").expect("Falta FIX_HOST en .env");
    let port = env::var("FIX_PORT").expect("Falta FIX_PORT en .env");
    let sender_id = env::var("FIX_SENDER_ID").expect("Falta FIX_SENDER_ID en .env");
    let target_id = env::var("FIX_TARGET_ID").expect("Falta FIX_TARGET_ID en .env");
    let sub_id = env::var("FIX_SENDER_SUB_ID").expect("Falta FIX_SENDER_SUB_ID en .env");
    let password = env::var("FIX_PASSWORD").expect("Falta FIX_PASSWORD en .env");

    // 3. Instanciar el motor y conectar la red
    let mut engine = fix_engine::FixEngine::new();

    let mut stream = match network::connect_to_broker(&host, &port).await {
        Ok(s) => s,
        Err(e) => {
            error!("Error de red: {}", e);
            return Err(e);
        }
    };

    // 4. Construir y enviar el mensaje Logon (MsgType=A)
    let mut fix_buffer = Vec::new();
    engine.build_logon(&mut fix_buffer, &sender_id, &target_id, &sub_id, &password);

    info!("Enviando mensaje Logon a IC Markets...");
    stream.write_all(&fix_buffer).await?;

    // 5. LÓGICA DE LA TAREA 5: ESCUCHAR LA RESPUESTA (LISTENER)
    info!("Esperando confirmación del servidor...");

    let mut response_buffer = [0u8; 4096]; // Espacio para recibir la respuesta del broker

    // Esperamos la respuesta con un máximo de 5 segundos (Timeout)
    match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut response_buffer)).await {
        Ok(Ok(n)) if n > 0 => {
            // Convertimos bytes a String
            let raw_response = String::from_utf8_lossy(&response_buffer[..n]);

            // Reemplazamos el delimitador FIX (\x01) por | para legibilidad
            let readable_response = raw_response.replace("\x01", "|");

            info!("--- RESPUESTA RECIBIDA ---");
            info!("{}", readable_response);
            info!("--------------------------");

            // Validación de la respuesta
            if readable_response.contains("|35=A|") {
                info!("✅ [SISTEMA] LOGON EXITOSO: Sesión FIX establecida y activa.");
                info!("¡Fase 1 de Conexión completada con éxito!");
            } else if readable_response.contains("|35=5|") {
                error!("❌ [SISTEMA] LOGON RECHAZADO: El servidor envió un Logout.");
                if readable_response.contains("58=") {
                    // El Tag 58 suele contener el texto del error
                    error!("Razón del rechazo: {}", readable_response);
                }
            }
        }
        Ok(Ok(_)) => {
            error!("El servidor cerró la conexión inmediatamente después del envío.");
        }
        Ok(Err(e)) => {
            error!("Error al intentar leer del socket: {}", e);
        }
        Err(_) => {
            error!("Timeout agotado: El servidor no respondió al Logon en 5 segundos.");
        }
    }

    info!("Cerrando motor de prueba v0.1.0.");
    Ok(())
}
