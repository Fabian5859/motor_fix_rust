use log::{error, info};
use std::error::Error;
use tokio::net::TcpStream;

/// Establece la conexión TCP con el servidor de IC Markets
pub async fn connect_to_broker() -> Result<TcpStream, Box<dyn Error>> {
    let host = "demo-uk-eqx-01.p.c-trader.com";
    let port = "5201";
    let address = format!("{}:{}", host, port);

    info!("Intentando conectar a Londres: {}...", address);

    match TcpStream::connect(&address).await {
        Ok(stream) => {
            info!("¡ÉXITO! Conexión TCP establecida.");
            Ok(stream)
        }
        Err(e) => {
            error!("FALLO de conexión: {}.", e);
            Err(Box::new(e))
        }
    }
}
