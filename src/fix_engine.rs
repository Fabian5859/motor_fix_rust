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
        let now = Utc::now().format("%Y%m%d-%H:%M:%S").to_string();
        let account_number = sender_id.split('.').last().unwrap_or(sender_id);

        buffer.clear();
        let mut msg = self.encoder.start_message(b"FIX.4.4", buffer, b"A");

        msg.set_any(TagU16::new(49).unwrap(), sender_id.as_bytes());
        msg.set_any(TagU16::new(56).unwrap(), target_id.as_bytes());
        msg.set_any(TagU16::new(50).unwrap(), sender_sub_id.as_bytes());
        msg.set_any(TagU16::new(57).unwrap(), sender_sub_id.as_bytes());
        msg.set_any(TagU16::new(34).unwrap(), b"1");
        msg.set_any(TagU16::new(52).unwrap(), now.as_bytes());

        msg.set_any(TagU16::new(98).unwrap(), b"0");
        msg.set_any(TagU16::new(108).unwrap(), b"30");
        msg.set_any(TagU16::new(553).unwrap(), account_number.as_bytes());
        msg.set_any(TagU16::new(554).unwrap(), password.as_bytes());
        msg.set_any(TagU16::new(141).unwrap(), b"Y");

        msg.wrap();
    }

    pub fn build_heartbeat(
        &mut self,
        buffer: &mut Vec<u8>,
        sender_id: &str,
        target_id: &str,
        seq_num: u64,
    ) {
        let now = Utc::now().format("%Y%m%d-%H:%M:%S").to_string();
        buffer.clear();
        let mut msg = self.encoder.start_message(b"FIX.4.4", buffer, b"0");
        msg.set_any(TagU16::new(49).unwrap(), sender_id.as_bytes());
        msg.set_any(TagU16::new(56).unwrap(), target_id.as_bytes());
        msg.set_any(
            TagU16::new(34).unwrap(),
            ToString::to_string(&seq_num).as_bytes(),
        );
        msg.set_any(TagU16::new(52).unwrap(), now.as_bytes());
        msg.wrap();
    }

    pub fn build_market_data_request(
        &mut self,
        buffer: &mut Vec<u8>,
        sender_id: &str,
        target_id: &str,
        seq_num: u64,
        symbol: &str,
    ) {
        let now = Utc::now().format("%Y%m%d-%H:%M:%S").to_string();
        buffer.clear();

        let mut msg = self.encoder.start_message(b"FIX.4.4", buffer, b"V");

        // Header
        msg.set_any(TagU16::new(49).unwrap(), sender_id.as_bytes());
        msg.set_any(TagU16::new(56).unwrap(), target_id.as_bytes());
        msg.set_any(
            TagU16::new(34).unwrap(),
            ToString::to_string(&seq_num).as_bytes(),
        );
        msg.set_any(TagU16::new(52).unwrap(), now.as_bytes());

        // Body: Request Market Data
        msg.set_any(TagU16::new(262).unwrap(), b"REQ_GAUSS_01"); // ID único
        msg.set_any(TagU16::new(263).unwrap(), b"1"); // 1 = Suscribirse
        msg.set_any(TagU16::new(264).unwrap(), b"1"); // 1 = Full Book (Depth completo)
        msg.set_any(TagU16::new(265).unwrap(), b"1"); // 1 = Incremental Refresh

        msg.set_any(TagU16::new(267).unwrap(), b"2"); // 2 tipos de entradas:
        msg.set_any(TagU16::new(269).unwrap(), b"0"); // 0 = Bid
        msg.set_any(TagU16::new(269).unwrap(), b"1"); // 1 = Ask

        msg.set_any(TagU16::new(146).unwrap(), b"1"); // 1 símbolo solicitado
        msg.set_any(TagU16::new(55).unwrap(), symbol.as_bytes());

        msg.wrap();
    }
}
