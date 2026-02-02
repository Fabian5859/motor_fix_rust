use chrono::Utc;
use fefix::prelude::*;
use fefix::tagvalue::{Config, Encoder};
use log::info;

pub struct FixEngine {
    pub encoder: Encoder<Config>,
}

impl FixEngine {
    pub fn new() -> Self {
        info!("Inicializando Motor FEFIX v0.7.0");
        Self {
            encoder: Encoder::<Config>::default(),
        }
    }

    pub fn build_logon(
        &mut self,
        buffer: &mut Vec<u8>,
        sender_id: &str,
        target_id: &str,
        sender_sub_id: &str,
        password: &str,
    ) {
        info!("Construyendo mensaje Logon (MsgType=A)...");

        let now = Utc::now().format("%Y%m%d-%H:%M:%S").to_string();

        // EXTRAER EL NÚMERO DE CUENTA (Tag 553 espera un entero)
        // Si sender_id es "demo.icmarkets.9757924", esto tomará "9757924"
        let account_number = sender_id.split('.').last().unwrap_or(sender_id);

        buffer.clear();
        let mut msg = self.encoder.start_message(b"FIX.4.4", buffer, b"A");

        // --- HEADER ---
        msg.set_any(TagU16::new(49).unwrap(), sender_id.as_bytes());
        msg.set_any(TagU16::new(56).unwrap(), target_id.as_bytes());
        msg.set_any(TagU16::new(50).unwrap(), sender_sub_id.as_bytes());
        msg.set_any(TagU16::new(57).unwrap(), sender_sub_id.as_bytes());
        msg.set_any(TagU16::new(34).unwrap(), b"1");
        msg.set_any(TagU16::new(52).unwrap(), now.as_bytes());

        // --- CUERPO ---
        msg.set_any(TagU16::new(98).unwrap(), b"0");
        msg.set_any(TagU16::new(108).unwrap(), b"30");

        // AQUÍ ESTÁ EL CAMBIO:
        info!("Usando Username (Tag 553): {}", account_number);
        msg.set_any(TagU16::new(553).unwrap(), account_number.as_bytes());

        msg.set_any(TagU16::new(554).unwrap(), password.as_bytes());
        msg.set_any(TagU16::new(141).unwrap(), b"Y");

        msg.wrap();
        info!("Logon construido. Cruzando los dedos por el Tag 553 numérico.");
    }
}
