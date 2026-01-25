// Importamos las herramientas de registro (logging)
use log::{error, info, warn};
// Importamos el motor de errores estándar para el retorno de main
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. Inicializamos el logger.
    // Esto lee la configuración de la variable de entorno RUST_LOG.
    env_logger::init();

    info!("========================================");
    info!("INICIO: Motor FIX IC Markets - Fase 1");
    info!("Tarea 1: Sistema de Diagnóstico Activo");
    info!("========================================");

    // En la próxima tarea (F2), aquí configuraremos el host de Londres
    let host = "demo-uk-eqx-01.p.c-trader.com";
    let port = 5201;

    info!("Preparado para conectar a {}:{}", host, port);

    // Este mensaje aparecerá en amarillo en tu terminal
    warn!("AVISO: El socket TCP aún no está abierto. Esperando Tarea 2.");

    // Aquí irá el bucle principal de nuestra estrategia de 2.6 pips

    info!("Motor cargado correctamente. Listo para ejecución.");

    Ok(())
}
