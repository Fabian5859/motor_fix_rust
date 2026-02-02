use log::{error, info};
use std::error::Error;
use tokio::net::TcpStream;

/// Establece una conexión TCP con el broker.
/// Recibe el host y el puerto como strings para permitir configuración dinámica.
pub async fn connect_to_broker(host: &str, port: &str) -> Result<TcpStream, Box<dyn Error>> {
    // Combinamos host y puerto en una sola dirección (ej: "demo-uk-eqx-01.p.c-trader.com:5202")
    let addr = format!("{}:{}", host, port);

    info!("Intentando conectar a la dirección TCP: {}...", addr);

    // Intentamos establecer la conexión
    match TcpStream::connect(&addr).await {
        Ok(stream) => {
            info!("¡ÉXITO! Conexión TCP establecida con el servidor de cTrader.");
            Ok(stream)
        }
        Err(e) => {
            error!(
                "Error de red: No se pudo conectar a {}. Verifica tu conexión a internet o el Host/Port.",
                addr
            );
            Err(Box::new(e))
        }
    }
}

