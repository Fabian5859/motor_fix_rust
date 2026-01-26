use fefix::tagvalue::Encoder;
use log::info;

pub struct FixEngine {
    // Quitamos el <Vec<u8>> si el compilador sigue quejándose
    // y usamos el tipo por defecto de la librería.
    pub encoder: Encoder,
}

impl FixEngine {
    pub fn new() -> Self {
        info!("Inicializando Motor FEFIX 0.7");

        Self {
            encoder: Encoder::default(),
        }
    }

    pub fn prepare_logon(&mut self) {
        info!("Preparando estructura de mensaje Logon...");
    }
}
