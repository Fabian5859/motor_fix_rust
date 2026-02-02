use dotenv::dotenv;
use log::{error, info};
use std::env;
use std::error::Error;
use tokio::io::AsyncWriteExt;
use tokio::time::{Duration, sleep};

mod fix_engine;
mod network;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. Inicialización de entorno y logs
    dotenv().ok();
    env_logger::init();

    info!("=== MOTOR FIX v0.1.0 - INICIANDO LOGON ===");

    // 2. Carga de credenciales desde el archivo .env
    // Usamos .expect para que el programa se detenga con un mensaje claro si falta algo
    let host = env::var("FIX_HOST").expect("Falta FIX_HOST en .env");
    let port = env::var("FIX_PORT").expect("Falta FIX_PORT en .env");
    let sender_id = env::var("FIX_SENDER_ID").expect("Falta FIX_SENDER_ID en .env");
    let target_id = env::var("FIX_TARGET_ID").expect("Falta FIX_TARGET_ID en .env");
    let sub_id = env::var("FIX_SENDER_SUB_ID").expect("Falta FIX_SENDER_SUB_ID en .env");
    let password = env::var("FIX_PASSWORD").expect("Falta FIX_PASSWORD en .env");

    // 3. Instanciar el motor y conectar la red
    let mut engine = fix_engine::FixEngine::new();

    // Conectamos usando los datos dinámicos (Trade Connection: 5202)
    let mut stream = match network::connect_to_broker(&host, &port).await {
        Ok(s) => s,
        Err(e) => {
            error!("No se pudo conectar al broker: {}", e);
            return Err(e);
        }
    };

    // 4. Construir el mensaje Logon (MsgType=A)
    let mut fix_buffer = Vec::new();
    engine.build_logon(&mut fix_buffer, &sender_id, &target_id, &sub_id, &password);

    // 5. Envío del mensaje a través del stream TCP
    info!("Enviando mensaje Logon a IC Markets...");
    if let Err(e) = stream.write_all(&fix_buffer).await {
        error!("Error al enviar el mensaje FIX: {}", e);
        return Err(Box::new(e));
    }

    info!("¡Mensaje enviado con éxito!");
    info!("Esperando 10 segundos para verificar estabilidad de la sesión...");

    // 6. Mantenemos la conexión abierta un momento para recibir respuesta
    // En la Tarea 5 implementaremos el "Listener" para leer qué nos respondió el servidor
    sleep(Duration::from_secs(10)).await;

    info!("Fin de la ejecución de prueba (v0.1.0).");
    Ok(())
}

