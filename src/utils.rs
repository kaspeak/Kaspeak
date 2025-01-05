use indexmap::IndexSet;
use rodio::{Decoder, OutputStream, Sink};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::sync::Arc;

use kaspa_wallet_core::prelude::*;
use kaspa_wrpc_client::prelude::*;

use crate::constants::{ADJECTIVES, EMOJIS, NOTIFICATION_SOUND_FILE_INLINED, NOUNS, PREFIX};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::Device;
use kaspa_wallet_core::rpc::ConnectStrategy::Fallback;

/// Собирает список доступных устройств ввода и возвращает кортеж: (список названий, устройство по умолчанию).
pub fn gather_audio_devices() -> (Vec<String>, Option<Device>) {
    let host = cpal::default_host();

    let available_input_devices = match host.input_devices() {
        Ok(devices) => devices.map(|d| d.name().unwrap_or_else(|_| "Unknown".to_string())).collect(),
        Err(e) => {
            log::error!("Error while retrieving input device list: {:?}", e);
            vec![]
        }
    };

    let selected_input_device = host.default_input_device();
    (available_input_devices, selected_input_device)
}

pub struct LimitedHashSet<T> {
    set: IndexSet<T>,
    capacity: usize,
}

impl<T: std::hash::Hash + Eq + std::fmt::Debug> LimitedHashSet<T> {
    /// Создает новый `LimitedHashSet` с заданной вместимостью.
    pub fn new(capacity: usize) -> Self {
        Self { set: IndexSet::new(), capacity }
    }

    /// Вставляет элемент в набор. Если достигнута вместимость, удаляется самый старый элемент.
    pub fn insert(&mut self, value: T) {
        if self.set.len() == self.capacity {
            if let Some(oldest) = self.set.shift_remove_index(0) {
                log::info!("Removed the oldest element: {:?}", oldest);
            }
        }
        self.set.insert(value);
    }

    /// Проверяет, содержит ли набор данный элемент.
    pub fn contains(&self, value: &T) -> bool {
        self.set.contains(value)
    }
}

/// Инициализирует RPC-клиент Kaspa.
pub fn bootstrap_rpc_client(network_id: NetworkId, url: Option<String>) -> Arc<KaspaRpcClient> {
    let (resolver, url) = if let Some(url) = url { (None, Some(url)) } else { (Some(Resolver::default()), None) };

    let client =
        Arc::new(KaspaRpcClient::new_with_args(WrpcEncoding::Borsh, url.as_deref(), resolver, Some(network_id), None).unwrap());

    client
}

pub fn shorten_address(full: &str) -> String {
    if full == "Empty" || full == "Error" {
        return full.to_string();
    }
    // Пробуем удалить префикс
    if let Some(rest) = full.strip_prefix(PREFIX) {
        let first5 = &rest[..5];
        let last5 = &rest[rest.len() - 5..];
        format!("{PREFIX}{first5}...{last5}")
    } else {
        // Если нет префикса "kaspatest:"
        full.to_string()
    }
}

/// Генерация "человеко-понятного" имени пользователя из входной строки (обычно это мнемоника).
pub fn generate_username(input: &str) -> String {
    // Хэшируем входную строку с помощью SHA256
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash_result = hasher.finalize();
    let bytes = hash_result.as_slice();

    // Убедимся, что хэш содержит достаточно байтов
    if bytes.len() < 18 {
        panic!("Insufficient bytes in the hash to generate a user name.");
    }

    // Извлекаем первые 8 байт для прилагательного
    let adj_bytes: [u8; 8] = bytes[0..8].try_into().unwrap();
    let adj_num = u64::from_be_bytes(adj_bytes);
    let adj_index = (adj_num as usize) % ADJECTIVES.len();
    let adjective = ADJECTIVES[adj_index];

    // Извлекаем следующие 8 байт для существительного
    let noun_bytes: [u8; 8] = bytes[8..16].try_into().unwrap();
    let noun_num = u64::from_be_bytes(noun_bytes);
    let noun_index = (noun_num as usize) % NOUNS.len();
    let noun = NOUNS[noun_index];

    // Извлекаем 1 байт для эмодзи
    let emoji_byte = bytes[16];
    let emoji_index = (emoji_byte as usize) % EMOJIS.len();
    let emoji = EMOJIS[emoji_index];

    // Формируем имя пользователя
    format!("{}{} {}", adjective, noun, emoji)
}

pub fn play_notification_sound() -> Result<(), Box<dyn std::error::Error>> {
    // Получаем устройство вывода по умолчанию
    let (_stream, stream_handle) = OutputStream::try_default()?;

    // Создаём `Sink`
    let sink = Sink::try_new(&stream_handle)?;

    let beep_source = Decoder::new_wav(Cursor::new(NOTIFICATION_SOUND_FILE_INLINED))?;

    // Добавляем источник звука в Sink
    sink.append(beep_source);

    sink.sleep_until_end();

    Ok(())
}

pub async fn try_connect_to_node(kaspa_rpc_client: Arc<KaspaRpcClient>, node_url: Option<String>) {
    // Опции для подключения
    let options = ConnectOptions { block_async_connect: true, strategy: Fallback, url: node_url.clone(), ..Default::default() };

    // Начало RPC подключения
    if let Err(e) = kaspa_rpc_client.connect(Some(options)).await {
        log::error!("Error while connecting to node '{:?}': {}", node_url, e);
    }
}

pub fn parse_3bytes_to_u32(bytes: &[u8]) -> u32 {
    ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32)
}

pub fn u32_to_3bytes(value: u32) -> [u8; 3] {
    [((value >> 16) & 0xFF) as u8, ((value >> 8) & 0xFF) as u8, (value & 0xFF) as u8]
}
