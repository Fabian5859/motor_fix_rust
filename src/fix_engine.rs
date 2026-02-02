use fefix::prelude::*; // Importante para TagU16
use fefix::tagvalue::{Config, Encoder, FvWrite};
use log::info;

pub struct FixEngine {
    pub encoder: Encoder<Config>,
}

impl FixEngine {
    /// Crea una nueva instancia del motor FIX
    pub fn new() -> Self {
        info!("Inicializando Motor FEFIX v0.7.0");
        Self {
            encoder: Encoder::<Config>::default(),
        }
    }

    /// Construye el mensaje de Logon (MsgType=A) con los tags requeridos por cTrader
    pub fn build_logon(
        &mut self,
        buffer: &mut Vec<u8>,
        sender_id: &str,
        target_id: &str,
        sender_sub_id: &str,
        password: &str,
    ) {
        info!("Construyendo mensaje Logon (MsgType=A)...");
        info!(
            "SenderCompID: {}, SenderSubID: {}",
            sender_id, sender_sub_id
        );

        // Limpiamos el buffer para evitar basura de mensajes anteriores
        buffer.clear();

        // Iniciamos el mensaje: Versión FIX.4.4, Buffer de salida, MsgType "A" (Logon)
        let mut msg = self.encoder.start_message(b"FIX.4.4", buffer, b"A");

        // --- CUERPO DEL MENSAJE ---

        // Tag 49: SenderCompID (ID del usuario)
        msg.set_any(TagU16::new(49).unwrap(), sender_id.as_bytes());

        // Tag 56: TargetCompID (Siempre "cServer" para cTrader)
        msg.set_any(TagU16::new(56).unwrap(), target_id.as_bytes());

        // Tag 50: SenderSubID (TRADE o QUOTE según la conexión)
        msg.set_any(TagU16::new(50).unwrap(), sender_sub_id.as_bytes());

        // Tag 34: MsgSeqNum (Iniciamos en 1 para este ejemplo)
        msg.set_any(TagU16::new(34).unwrap(), b"1");

        // Tag 98: EncryptMethod (0 = None/Other)
        msg.set_any(TagU16::new(98).unwrap(), b"0");

        // Tag 108: HeartBtInt (Intervalo de latido en segundos)
        msg.set_any(TagU16::new(108).unwrap(), b"30");

        // Tag 554: Password (Credencial de la API FIX)
        msg.set_any(TagU16::new(554).unwrap(), password.as_bytes());

        // Tag 141: ResetSeqNumFlag (Y = Reiniciar secuencia a 1)
        // Es muy recomendable ponerlo en 'Y' para el primer logon del día
        msg.set_any(TagU16::new(141).unwrap(), b"Y");

        // Finaliza el mensaje calculando el Checksum y el BodyLength automáticamente
        msg.wrap();

        info!("Mensaje Logon construido exitosamente.");
    }
}

