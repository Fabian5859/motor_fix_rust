use log::info;
use std::error::Error;

mod fix_engine;
mod network;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. Inicializamos los logs
    env_logger::init();
    info!("=== PROYECTO MOTOR FIX - VERIFICACIÓN TAREA 3 ===");

    // 2. Instanciamos el motor FIX (v0.7 oficial)
    // Esto verifica que el struct FixEngine y el Encoder estén bien configurados
    let _engine = fix_engine::FixEngine::new();
    info!("Motor FIX inicializado correctamente siguiendo la documentación oficial.");

    // 3. Probamos la conexión de red (Tarea 2)
    // Solo para asegurar que el motor y la red pueden coexistir
    let _stream = network::connect_to_broker().await?;
    info!("¡ÉXITO! El motor está listo y la red está conectada.");

    Ok(())
}

