use crate::constants::{CHANNELS, FRAME_SIZE, SAMPLE_RATE};

use crate::models::payload::{Payload, StatusFlag};
use cpal::traits::{DeviceTrait, HostTrait};
use dashmap::DashMap;
use opus::Decoder as OpusDecoder;
use rodio::{OutputStream, OutputStreamHandle, PlayError, Sink};
use std::collections::BTreeMap;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct UserBuffer {
    buffer: BTreeMap<u32, (Payload, Instant)>,
    is_stream_active: bool,
}

impl UserBuffer {
    pub fn new() -> Self {
        UserBuffer { buffer: BTreeMap::new(), is_stream_active: true }
    }

    /// Добавление фрагмента в буфер
    pub fn add_fragment(&mut self, fragment: Payload) {
        self.buffer.insert(fragment.get_fragment_number(), (fragment.clone(), Instant::now()));

        match fragment.get_status_flag() {
            StatusFlag::Start => {}
            StatusFlag::Continue => {}
            StatusFlag::End => {
                self.is_stream_active = false;
            }
            _ => {}
        }
    }

    /// Получение следующего фрагмента по порядку
    pub fn next_fragment(&mut self) -> Option<Payload> {
        let smallest_key = self.buffer.keys().next().cloned();
        match smallest_key {
            Some(key) => {
                if let Some((fragment, _)) = self.buffer.remove(&key) {
                    Some(fragment)
                } else {
                    None
                }
            }
            None => None,
        }
    }
}

pub struct Player {
    buffers: Arc<DashMap<String, UserBuffer>>,
}

impl Player {
    pub fn new() -> Self {
        // Информация о доступных устройствах вывода
        let host = cpal::default_host();
        if let Ok(devices) = host.output_devices() {
            log::info!("Available output devices:");
            for dev in devices {
                log::info!(" - {}", dev.name().unwrap_or_else(|_| "Unknown".to_string()));
            }
        } else {
            log::error!("Error while obtaining output devices");
        }

        if let Some(default_output) = host.default_output_device() {
            log::info!("Current output device: {:?}", default_output.name().unwrap_or_else(|_| "Unknown".to_string()));
        } else {
            log::info!("No default output device found");
        }

        Player { buffers: Arc::new(DashMap::new()) }
    }

    /// Запускает блокирующий цикл обработки входящих данных и воспроизведения.
    pub fn run_blocking(self: Arc<Self>, rx: Receiver<Payload>) {
        // Отдельный поток для обработки входящих фрагментов
        self.spawn_incoming_fragments_thread(rx);

        // Отдельный поток для "уборки мусора" (не реализован подробно, заготовка)
        self.spawn_cleanup_thread();

        // Отдельный поток для считывания и воспроизведения буферов
        Self::start_dynamic_playback(Arc::clone(&self.buffers))
    }

    /// Поток для чтения/обработки входящих фрагментов
    fn spawn_incoming_fragments_thread(&self, rx: Receiver<Payload>) {
        let buffers = Arc::clone(&self.buffers);
        thread::spawn(move || {
            log::info!("Incoming fragment processing thread started");
            for fragment in rx {
                let mut user_buffer = buffers.entry(fragment.get_username().to_string()).or_insert_with(UserBuffer::new);
                user_buffer.add_fragment(fragment.clone());
                log::debug!("Current sender buffer size id={} : {} fragments", fragment.get_username(), user_buffer.buffer.len());
            }
            log::info!("Incoming fragment processing thread finished (channel closed)");
        });
    }

    /// Поток для уборки устаревших данных (пока заглушка)
    fn spawn_cleanup_thread(&self) {
        let buffers_for_cleanup = Arc::clone(&self.buffers);
        thread::spawn(move || {
            log::info!("Garbage collector thread started");
            let cleanup_interval = Duration::from_secs(10);

            loop {
                thread::sleep(cleanup_interval);
                // TODO: Реализовать логику удаления старых буферов, неактивных пользователей
                let _ = buffers_for_cleanup;
            }
        });
    }

    /// Стартует потоки для воспроизведения буферов
    /// TODO проверить, логика выглядит чуть странной
    fn start_dynamic_playback(buffers: Arc<DashMap<String, UserBuffer>>) {
        let active_threads = Arc::new(DashMap::new());

        thread::spawn({
            let buffers = Arc::clone(&buffers);
            let active_threads = Arc::clone(&active_threads);

            move || loop {
                // Проверяем текущие буферы
                for user_id in buffers.iter().map(|entry| entry.key().clone()) {
                    let active_threads_inner = Arc::clone(&active_threads);

                    // Запускаем поток, если его ещё нет
                    if !active_threads_inner.contains_key(&user_id) {
                        let buffer = Arc::clone(&buffers);
                        let user_id_inner = user_id.clone();

                        thread::spawn(move || {
                            log::info!("Playback thread for sender buffer id={} started", &user_id_inner);
                            let (_stream, stream_handle) = OutputStream::try_default().unwrap();
                            let mut opus_decoder = OpusDecoder::new(SAMPLE_RATE, CHANNELS).unwrap();

                            loop {
                                if let Some(mut buffer) = buffer.get_mut(&user_id_inner) {
                                    if let Some(fragment) = buffer.next_fragment() {
                                        match Player::handle_fragment(&fragment, &mut opus_decoder, &stream_handle) {
                                            Ok(_) => (),
                                            Err(e) => {
                                                log::error!("Error processing fragment from {}: {:?}", fragment.get_username(), e)
                                            }
                                        }
                                    }
                                } else {
                                    log::info!("Playback thread for id={} finished (buffer removed)", &user_id_inner);
                                    break;
                                }

                                thread::sleep(Duration::from_millis(10));
                            }

                            // Удаляем поток из списка активных
                            active_threads_inner.remove(&user_id_inner);
                        });

                        active_threads.insert(user_id, ());
                    }
                }

                // Очищаем active_threads от тех, чьи буферы были удалены
                let current_buffers: Vec<String> = buffers.iter().map(|entry| entry.key().clone()).collect();
                active_threads.retain(|user_id, _| current_buffers.contains(user_id));

                thread::sleep(Duration::from_millis(100)); // Небольшая пауза для разгрузки CPU
            }
        });
    }

    /// Раскодировать и воспроизвести один фрагмент.
    fn handle_fragment(
        fragment: &Payload,
        opus_decoder: &mut OpusDecoder,
        stream_handle: &OutputStreamHandle,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // todo decode_audio_fragment тоже можно вынести в utils
        let decoded_audio = Self::decode_audio_fragment(opus_decoder, &fragment.get_data())?;
        if decoded_audio.is_empty() {
            log::warn!("Decoded audio is empty for fragment: {}", fragment.get_fragment_number());
            return Ok(());
        }

        Player::play_audio(decoded_audio, stream_handle)?;
        Ok(())
    }

    fn decode_audio_fragment(opus_decoder: &mut OpusDecoder, opus_data: &[u8]) -> Result<Vec<i16>, opus::Error> {
        let mut decoded_audio = Vec::new();
        let mut offset = 0;

        while offset + 4 <= opus_data.len() {
            let fragment_size_bytes = &opus_data[offset..offset + 4];
            let fragment_size = u32::from_le_bytes(fragment_size_bytes.try_into().unwrap()) as usize;
            offset += 4;

            if offset + fragment_size > opus_data.len() {
                log::error!("Invalid fragment size: expected {}, available {}", fragment_size, opus_data.len() - offset);
                break;
            }

            let fragment = &opus_data[offset..offset + fragment_size];
            offset += fragment_size;

            let mut decoded_frame = vec![0i16; FRAME_SIZE];

            match opus_decoder.decode(fragment, &mut decoded_frame, false) {
                Ok(decoded_samples) => {
                    decoded_audio.extend_from_slice(&decoded_frame[..decoded_samples]);
                }
                Err(e) => {
                    log::error!("Decoder error: {:?}", e);
                }
            }
        }

        Ok(decoded_audio)
    }

    fn play_audio(decoded_audio: Vec<i16>, stream_handle: &OutputStreamHandle) -> Result<(), PlayError> {
        if decoded_audio.is_empty() {
            log::error!("Decoded audio is empty, nothing to play");
            return Err(PlayError::DecoderError(rodio::decoder::DecoderError::DecodeError("Empty audio data".into())));
        }

        let sink = Sink::try_new(stream_handle)?;
        let source = rodio::buffer::SamplesBuffer::new(1, SAMPLE_RATE, decoded_audio.clone());

        log::info!("Playback started: {} samples", decoded_audio.len());
        sink.append(source);
        sink.sleep_until_end();
        log::info!("Playback finished");
        Ok(())
    }
}
