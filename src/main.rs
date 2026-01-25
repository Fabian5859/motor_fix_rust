use log::info;
use std::error::Error;

// Declaramos que existe un módulo llamado 'network'
mod network;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. Inicializar Logs (Tarea 1)
    env_logger::init();
    info!("INICIO: Motor FIX - Estructura Multimodular");

    // 2. Llamar a la conexión (Tarea 2)
    // Usamos el prefijo 'network::' porque la función está en ese archivo
    let _stream = network::connect_to_broker().await?;

    info!("Motor listo y conectado. Esperando lógica de mensajes FIX...");

    Ok(())
}

