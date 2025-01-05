use zstd::{decode_all, encode_all};

use crate::app_state::APP_STATE;
use crate::constants;
use crate::models::recording::Recording;
use crate::utils::{parse_3bytes_to_u32, u32_to_3bytes};
use std::time::{SystemTime, UNIX_EPOCH};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Text = 1,
    Voice = 2,
    File = 3,
    Unknown(u8),
}

impl MessageType {
    fn from_byte(byte: u8) -> Self {
        match byte {
            1 => MessageType::Text,
            2 => MessageType::Voice,
            3 => MessageType::File,
            other => MessageType::Unknown(other),
        }
    }

    fn to_byte(&self) -> u8 {
        match *self {
            MessageType::Text => 1,
            MessageType::Voice => 2,
            MessageType::File => 3,
            MessageType::Unknown(val) => val,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFlag {
    Start = 1,
    Continue = 2,
    End = 3,
    Unknown(u8),
}

impl StatusFlag {
    fn from_byte(byte: u8) -> Self {
        match byte {
            1 => StatusFlag::Start,
            2 => StatusFlag::Continue,
            3 => StatusFlag::End,
            other => StatusFlag::Unknown(other),
        }
    }

    fn to_byte(&self) -> u8 {
        match *self {
            StatusFlag::Start => 1,
            StatusFlag::Continue => 2,
            StatusFlag::End => 3,
            StatusFlag::Unknown(val) => val,
        }
    }
}
#[derive(Debug, Clone)]
pub struct Payload {
    protocol_version: u8,
    channel_number: u32,       // 3 байта
    message_type: MessageType, // 1 байт
    status_flag: StatusFlag,   // 1 байт
    fragment_number: u32,      // 3 байта
    username: String,          // Переменная длина (<= 255 байт, <= 18 chars)
    data: Vec<u8>,             // Полезная нагрузка (<= 15000 байт)
    received_time: Option<SystemTime>,
}

impl Payload {
    pub fn new(
        channel_number: u32,
        message_type: MessageType,
        status_flag: StatusFlag,
        fragment_number: u32,
        username: &str,
        data: Vec<u8>,
        received_time: Option<SystemTime>,
    ) -> Result<Self, String> {
        let uname_chars = username.chars().count();
        let uname_bytes = username.as_bytes().len();
        if uname_chars > constants::MAX_USERNAME_CHARS {
            return Err(format!("Username has {} chars, max allowed is {}", uname_chars, constants::MAX_USERNAME_CHARS));
        }
        if uname_bytes > constants::MAX_USERNAME_BYTES {
            return Err(format!("Username has {} bytes, max allowed is {}", uname_bytes, constants::MAX_USERNAME_BYTES));
        }

        let data_len = data.len();
        if data_len > constants::MAX_PAYLOAD_BYTES {
            return Err(format!("Payload has {} bytes, max allowed is {}", data_len, constants::MAX_PAYLOAD_BYTES));
        }

        Ok(Self {
            protocol_version: constants::PROTOCOL_VERSION,
            channel_number,
            message_type,
            status_flag,
            fragment_number,
            username: username.to_string(),
            data,
            received_time,
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < constants::HEADER_SIZE {
            return Err("Incoming data is too short for header".to_string());
        }

        let mut pos = 0;

        if &bytes[pos..pos + 4] != constants::MARKER {
            return Err("Invalid marker".to_string());
        }
        pos += 4;

        let protocol_version = bytes[pos];
        if protocol_version != constants::PROTOCOL_VERSION {
            return Err(format!("Unsupported protocol version: {} (expected {})", protocol_version, constants::PROTOCOL_VERSION));
        }
        pos += 1;

        let channel_number = parse_3bytes_to_u32(&bytes[pos..pos + 3]);
        pos += 3;

        let message_type = MessageType::from_byte(bytes[pos]);
        pos += 1;

        let status_flag = StatusFlag::from_byte(bytes[pos]);
        pos += 1;

        let fragment_number = parse_3bytes_to_u32(&bytes[pos..pos + 3]);
        pos += 3;
        if pos >= bytes.len() {
            return Err("Not enough data for username length".to_string());
        }

        let username_length = bytes[pos] as usize;
        pos += 1;
        if pos + username_length > bytes.len() {
            return Err("Username length exceeds available data".to_string());
        }

        let username_bytes = &bytes[pos..pos + username_length];
        pos += username_length;

        let username = match String::from_utf8(username_bytes.to_vec()) {
            Ok(s) => s,
            Err(_) => return Err("Invalid username encoding (UTF-8)".to_string()),
        };

        if pos + 3 > bytes.len() {
            return Err("Not enough data for data length".to_string());
        }
        let data_length = parse_3bytes_to_u32(&bytes[pos..pos + 3]) as usize;
        pos += 3;

        if pos + data_length > bytes.len() {
            return Err("Payload length exceeds available data".to_string());
        }
        let data = bytes[pos..pos + data_length].to_vec();

        let payload = Self::new(channel_number, message_type, status_flag, fragment_number, &username, data, Some(SystemTime::now()))?;
        Ok(payload)
    }

    pub fn from_recording(recording: &Recording) -> Result<Self, String> {
        Self::new(
            APP_STATE.get_channel_number().unwrap_or(0),
            MessageType::Voice,
            recording.state,
            recording.fragment_num,
            APP_STATE.get_username().as_str(),
            recording.audio.clone(),
            None,
        )
    }

    pub fn from_chat_message(message: &str) -> Result<Self, String> {
        let msg_chars = message.chars().count();
        if msg_chars > constants::MAX_TEXT_CHARS {
            return Err(format!("Text data has {} chars, max allowed is {}", msg_chars, constants::MAX_TEXT_CHARS));
        }
        Self::new(
            APP_STATE.get_channel_number().unwrap_or(0),
            MessageType::Text,
            StatusFlag::End,
            0,
            APP_STATE.get_username().as_str(),
            message.as_bytes().to_vec(),
            None,
        )
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let uname_bytes = self.username.as_bytes().len();
        let mut packet = Vec::with_capacity(constants::HEADER_SIZE + uname_bytes + self.data.len());
        packet.extend_from_slice(constants::MARKER);
        packet.push(self.protocol_version);
        packet.extend_from_slice(&u32_to_3bytes(self.channel_number));
        packet.push(self.message_type.to_byte());
        packet.push(self.status_flag.to_byte());
        packet.extend_from_slice(&u32_to_3bytes(self.fragment_number));
        packet.push(uname_bytes as u8);
        packet.extend_from_slice(self.username.as_bytes());
        packet.extend_from_slice(&u32_to_3bytes(self.data.len() as u32));
        packet.extend_from_slice(&self.data);
        packet
    }

    pub fn compress_zstd(&mut self) -> Result<(), String> {
        if self.data.is_empty() {
            return Ok(());
        }
        let compressed =
            encode_all(&*self.data, constants::ZSTD_COMPRESSION_LEVEL).map_err(|e| format!("Zstd-compression error: {e}"))?;
        self.data = compressed;
        Ok(())
    }

    pub fn decompress_zstd(&mut self) -> Result<(), String> {
        if self.data.is_empty() {
            return Ok(());
        }
        let decompressed = decode_all(&*self.data).map_err(|e| format!("Zstd-decompression error: {e}"))?;
        self.data = decompressed;
        Ok(())
    }

    pub fn get_username(&self) -> &str {
        &self.username
    }
    pub fn get_data(&self) -> &[u8] {
        &self.data
    }
    pub fn get_channel(&self) -> u32 {
        self.channel_number
    }
    pub fn get_fragment_number(&self) -> u32 {
        self.fragment_number
    }
    pub fn get_message_type(&self) -> MessageType {
        self.message_type
    }
    pub fn get_status_flag(&self) -> StatusFlag {
        self.status_flag
    }
    pub fn get_received_time(&self) -> Option<SystemTime> {
        self.received_time
    }

    pub fn debug_string(&self) -> String {
        let rcv_time_str = match self.received_time {
            Some(t) => match t.duration_since(UNIX_EPOCH) {
                Ok(dur) => format!("{}ms", dur.as_millis()),
                Err(_) => "before 1970".to_string(),
            },
            None => "N/A".to_string(),
        };
        format!(
            "ver={}, channel={}, msg_type={:?}, status={:?}, fragment={}, username_len={} username='{}', data_len={}, received_time={}",
            self.protocol_version,
            self.channel_number,
            self.message_type,
            self.status_flag,
            self.fragment_number,
            self.username.len(),
            self.username,
            self.data.len(),
            rcv_time_str
        )
    }
}

#[cfg(test)]
mod payload_integration_tests {
    use super::*;
    use crate::constants;
    use crate::models::instruction::{Instruction, SendTxInstruction};

    #[test]
    fn test_text_short_ok() {
        let text = "Hi!";
        let p = Payload::from_chat_message(text).expect("Short text must be valid");
        let bytes = p.to_bytes();
        let parsed = Payload::from_bytes(&bytes).expect("Short text parse must succeed");
        assert_eq!(parsed.get_message_type(), MessageType::Text);
        assert_eq!(parsed.get_data(), text.as_bytes());
    }

    #[test]
    fn test_text_empty_ok() {
        let text = "";
        let p = Payload::from_chat_message(text).expect("Empty text is allowed");
    }

    #[test]
    fn test_text_too_long_chars() {
        let too_long = "X".repeat(constants::MAX_TEXT_CHARS + 5);
        let res = Payload::from_chat_message(&too_long);
        assert!(res.is_err(), "Should fail on too many chars");
    }

    #[test]
    fn test_incoming_incorrect_username_len() {
        let p = Payload::new(0, MessageType::Text, StatusFlag::Start, 0, "RealU", b"DATA".to_vec(), None).unwrap();
        let mut raw = p.to_bytes();
        let uname_len_offset = 13;
        raw[uname_len_offset] = 20;
        let res = Payload::from_bytes(&raw);
        assert!(res.is_err());
        let msg = res.err().unwrap();
        assert!(msg.contains("Username length exceeds available data"), "Expect mismatch in declared vs real data");
    }

    #[test]
    fn test_instruction_compress_decompress_ok() {
        let text = "Rust is fast!";
        let instr = Instruction::try_from_message(text.to_string());
        assert!(instr.is_ok(), "Creating instruction from normal text must be ok");

        // Извлекаем payload bytes
        if let Ok(Instruction::SendTx(SendTxInstruction { tx_payload: Some(bytes) })) = instr {
            let parsed = Payload::from_bytes(&bytes).expect("Parse must succeed");
            let mut cloned = parsed.clone();
            cloned.decompress_zstd().expect("Should decompress text ok");

            assert_eq!(String::from_utf8_lossy(cloned.get_data()), text);
        } else {
            panic!("Instruction is not SendTx or no payload inside");
        }
    }
}
