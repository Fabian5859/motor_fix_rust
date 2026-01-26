use fefix::tagvalue::{Config, Encoder, FvWrite}; // FvWrite es necesario para set_any
use fefix::prelude::*; // Trae tipos útiles como TagU16
use log::info;

pub struct FixEngine {
    pub encoder: Encoder<Config>,
}

impl FixEngine {
    pub fn new() -> Self {
        info!("Inicializando Motor FEFIX 0.7 - Validado con documentación");
        Self {
            encoder: Encoder::<Config>::default(),
        }
    }

    pub fn build_logon(&mut self, buffer: &mut Vec<u8>, sender_id: &str, target_id: &str, password: &str) {
        info!("Construyendo mensaje Logon...");
        
        buffer.clear();
        let mut msg = self.encoder.start_message(b"FIX.4.4", buffer, b"A");
        
        // La documentación usa TagU16::new(...).unwrap()
        // Para que sea legible, usamos variables claras:
        msg.set_any(TagU16::new(49).unwrap(), sender_id.as_bytes());   // SenderCompID
        msg.set_any(TagU16::new(56).unwrap(), target_id.as_bytes());   // TargetCompID
        msg.set_any(TagU16::new(34).unwrap(), b"1");                   // MsgSeqNum
        msg.set_any(TagU16::new(98).unwrap(), b"0");                   // EncryptMethod
        msg.set_any(TagU16::new(108).unwrap(), b"30");                 // HeartBtInt
        msg.set_any(TagU16::new(554).unwrap(), password.as_bytes());   // Password
        
        msg.wrap(); 
    }
}
