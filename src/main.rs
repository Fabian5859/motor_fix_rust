use log::info;
use std::error::Error;

mod fix_engine;
mod network; //Declaramos el nuevo modulo

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. Inicializar Logs (Tarea 1)
    env_logger::init();
    info!("=== MOTOR FIX RUST: FASE 1 - TAREA 3 ===");

    //1. Instanciamos el motor FIX
    let mut engine = fix_engine::FixEngine::new();
    engine.prepare_logon();

    // 2. Conexión de red (Tarea 2)
    let _stream = network::connect_to_broker().await?;

    info!("Estado: Motor inicializado y Socket conectado.");
    Ok(())
}
