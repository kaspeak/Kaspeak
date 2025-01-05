use crate::app_state::APP_STATE;
use crate::constants::{FRAME_DURATION_MS, OPUS_BITRATE, OPUS_MAX_PACKET_SIZE};
use crate::models::payload::StatusFlag;
use crate::models::recording::Recording;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, StreamConfig};
use opus::{Application, Bitrate, Channels, Encoder as OpusEncoder};
use std::error::Error;
use std::sync::mpsc::Sender;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

pub struct Recorder {
    opus_encoder: Arc<Mutex<OpusEncoder>>,
    pub config: Arc<Mutex<StreamConfig>>,
    pub sample_rate: Arc<Mutex<u32>>,
    pub channels_count: Arc<Mutex<usize>>,
    recording: Arc<AtomicBool>,
}

impl Recorder {
    pub fn try_new() -> Result<Option<Self>, Box<dyn Error>> {
        let option_encoder = Self::create_opus_encoder()?;
        if let Some(encoder) = option_encoder {
            let (opus_encoder, config, sample_rate, channels_count) = encoder;
            Ok(Some(Self {
                opus_encoder: Arc::new(Mutex::new(opus_encoder)),
                config: Arc::new(Mutex::new(config)),
                sample_rate: Arc::new(Mutex::new(sample_rate)),
                channels_count: Arc::new(Mutex::new(channels_count)),
                recording: Arc::new(AtomicBool::new(false)),
            }))
        } else {
            Ok(None)
        }
    }

    /// Запускает непрерывную запись в блокирующем режиме, пока не будет вызван stop_recording().
    /// Каждую секунду (или по увеличивающейся длительности) записывает фрагмент и отправляет его через канал `tx`.
    pub fn run_blocking(&self, tx: Sender<Arc<Recording>>) {
        // Если уже идёт запись, выходим
        if self.recording.load(Ordering::SeqCst) {
            log::info!("Recording is already in progress...");
            return;
        }
        self.recording.store(true, Ordering::SeqCst);

        // Проверка на смену устройства перед началом записи
        self.check_and_update_device();

        let mut fragment_num = 0;
        let mut first_packet_sent = false;

        while self.recording.load(Ordering::SeqCst) {
            let fragment_duration = Self::calculate_fragment_duration(fragment_num);

            // Пишем и кодируем аудио
            match self.record_and_encode_fragment(fragment_num, fragment_duration) {
                Ok(mut recording) => {
                    recording.fragment_num = fragment_num;

                    // Определяем текущее состояние фрагмента
                    let state = if fragment_num == 0 {
                        StatusFlag::Start
                    } else if !self.recording.load(Ordering::SeqCst) {
                        StatusFlag::End
                    } else {
                        StatusFlag::Continue
                    };
                    recording.state = state;

                    // Отправляем фрагмент
                    if let Err(e) = self.send_recorded_fragment(&tx, &mut first_packet_sent, recording) {
                        log::error!("{}", e);
                        break;
                    }

                    // Если фрагмент с признаком End — завершаем цикл
                    if state == StatusFlag::End {
                        break;
                    }

                    fragment_num += 1;
                }
                Err(err) => {
                    // При ошибке отправляем пустой фрагмент с End, чтобы «сообщить» получателю о завершении
                    log::error!("Audio recording error: {}", err);
                    self.send_ending_fragment(&tx, fragment_num);
                    break;
                }
            }
        }
    }

    /// Останавливает запись (run_blocking() завершится после следующей итерации цикла)
    pub fn stop_recording(&self) {
        self.recording.store(false, Ordering::SeqCst);
    }

    /// Перенастроить устройство ввода и Opus-энкодер при изменении входного устройства.
    pub fn update_input_device(&self) -> Result<(), Box<dyn Error>> {
        let option_encoder = Self::create_opus_encoder()?;
        if let Some(encoder) = option_encoder {
            let (opus_encoder, config, sample_rate, channels_count) = encoder;
            *self.opus_encoder.lock().unwrap() = opus_encoder;
            *self.config.lock().unwrap() = config;
            *self.sample_rate.lock().unwrap() = sample_rate;
            *self.channels_count.lock().unwrap() = channels_count;

            Ok(())
        } else {
            Ok(())
        }
    }

    /// Высчитывает оптимальную длительность фрагмента на основе номера
    fn calculate_fragment_duration(fragment_num: u32) -> Duration {
        /*  let min_duration = 600.0; // минимальная длительность в миллисекундах
        let max_duration = 1200.0; // максимальная длительность в миллисекундах
        let growth_factor: f32 = 1.2;

        let duration_ms = min_duration * growth_factor.powi(fragment_num as i32);
        let clamped_ms = duration_ms.min(max_duration);*/
        let duration = match fragment_num {
            num if num < 1 => 800,
            _ => 1200,
        };

        Duration::from_millis(duration)
    }

    /// Проверяет, нужно ли перенастроить устройство ввода, и при необходимости делает это.
    fn check_and_update_device(&self) {
        let device_changed = APP_STATE.is_input_device_changed().expect("Error while reading the flag");
        if device_changed {
            log::info!("Input device change detected. Reconfiguring encoder...");

            if let Err(e) = self.update_input_device() {
                log::error!("Error while reconfiguring encoder: {}", e);
                self.recording.store(false, Ordering::SeqCst);
                return;
            }

            // Сбрасываем флаг
            APP_STATE.set_input_device_changed(false).ok().expect("Failed to reset the flag");
        }
    }

    /// Запись и кодирование одного фрагмента аудио (используется в `run_blocking`).
    fn record_and_encode_fragment(&self, fragment_num: u32, duration: Duration) -> Result<Recording, Box<dyn Error>> {
        let raw_audio = self.record_audio_data(duration)?;
        let audio_i16: Vec<i16> = raw_audio.iter().map(|&sample| (sample * i16::MAX as f32) as i16).collect();
        if audio_i16.is_empty() {
            return Err("No data to record".into());
        }
        let opus_data = self.encode_to_opus(audio_i16)?;

        Ok(Recording { audio: opus_data, state: StatusFlag::Continue, fragment_num })
    }

    ///Запись аудио данных (f32) указанной длительности с выбранного устройства.
    fn record_audio_data(&self, duration: Duration) -> Result<Vec<f32>, Box<dyn Error>> {
        let audio_data = Arc::new(Mutex::new(Vec::new()));
        let audio_data_clone = audio_data.clone();

        let device = APP_STATE.get_selected_input_device()?.ok_or("No input device selected")?;

        let config = self.config.lock().unwrap();
        let stream = device.build_input_stream(
            &*config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                audio_data_clone.lock().unwrap().extend_from_slice(data);
            },
            move |err| {
                log::error!("Audio input error: {}", err);
            },
            None,
        )?;

        stream.play()?;
        std::thread::sleep(duration);
        stream.pause()?;

        let raw_audio = audio_data.lock().unwrap().clone();
        Ok(raw_audio)
    }

    /// Кодирование в Opus нескольких фреймов (с учётом FRAME_DURATION_MS).
    fn encode_to_opus(&self, audio_i16: Vec<i16>) -> Result<Vec<u8>, Box<dyn Error>> {
        let frame_size = (*self.sample_rate.lock().unwrap() as f32 * FRAME_DURATION_MS as f32 / 1000.0) as usize;
        let channels_count = *self.channels_count.lock().unwrap();

        let total_samples = audio_i16.len() / channels_count;
        let num_frames = total_samples / frame_size;
        if num_frames == 0 {
            return Err("Not enough data to encode".into());
        }

        let mut opus_data = Vec::new();
        let mut encoder = self.opus_encoder.lock().unwrap();

        for i in 0..num_frames {
            let start = i * frame_size * channels_count;
            let end = start + frame_size * channels_count;
            let frame = &audio_i16[start..end];

            let mut encoded_frame = vec![0u8; OPUS_MAX_PACKET_SIZE];
            let len = encoder.encode(frame, &mut encoded_frame)?;
            encoded_frame.truncate(len);

            // Сначала пишем 4 байта с размером пакета, затем сам пакет
            let packet_size = len as u32;
            opus_data.extend_from_slice(&packet_size.to_le_bytes());
            opus_data.extend_from_slice(&encoded_frame);
        }

        Ok(opus_data)
    }

    /// Отправка готового фрагмента по каналу. Управляет логикой отправки Start/Continue.
    fn send_recorded_fragment(
        &self,
        tx: &Sender<Arc<Recording>>,
        first_packet_sent: &mut bool,
        recording: Recording,
    ) -> Result<(), Box<dyn Error>> {
        let arc_recording = Arc::new(recording);

        if !*first_packet_sent {
            *first_packet_sent = true;
            tx.send(arc_recording.clone()).map_err(|_| Box::<dyn Error>::from("Error sending initial write data (Start fragment)"))?;
        } else {
            tx.send(arc_recording.clone()).map_err(|_| Box::<dyn Error>::from("Error sending audio data (Continue fragment)"))?;
        }

        Ok(())
    }

    /// В случае ошибки отправляем «пустой» фрагмент с состоянием End.
    fn send_ending_fragment(&self, tx: &Sender<Arc<Recording>>, fragment_num: u32) {
        let end_recording = Recording { audio: Vec::new(), state: StatusFlag::End, fragment_num };
        let arc_end = Arc::new(end_recording);
        let _ = tx.send(arc_end);
    }

    /// Создаёт новый Opus-энкодер (вызывается при инициализации и при смене устройства).
    fn create_opus_encoder() -> Result<Option<(OpusEncoder, StreamConfig, u32, usize)>, Box<dyn Error>> {
        let selected_device = APP_STATE.get_selected_input_device()?;

        if let Some(selected_device) = selected_device {
            let (config, sample_rate, channels, channels_count) = Self::get_device_config(&selected_device)?;

            let mut opus_encoder = OpusEncoder::new(sample_rate, channels, Application::Audio)?;
            opus_encoder.set_bitrate(Bitrate::Bits(OPUS_BITRATE))?;

            log::info!(
                "Encoder configured: Sample Rate = {}, Channels = {:?}, Bitrate = {} bits/s",
                sample_rate,
                channels,
                OPUS_BITRATE
            );
            Ok(Some((opus_encoder, config, sample_rate, channels_count)))
        } else {
            log::info!("No input device selected, encoder not created.");
            Ok(None)
        }
    }

    /// Возвращает кортеж: (StreamConfig, sample_rate, Channels, channels_count) с конфигурацией выбранного девайса.
    fn get_device_config(device: &Device) -> Result<(StreamConfig, u32, Channels, usize), Box<dyn Error>> {
        let config: StreamConfig = device.default_input_config()?.into();
        let sample_rate = config.sample_rate.0;
        let channels = match config.channels {
            1 => Channels::Mono,
            2 => Channels::Stereo,
            _ => return Err(format!("Unsupported number of channels: {}", config.channels).into()),
        };
        let channels_count = config.channels as usize;

        Ok((config, sample_rate, channels, channels_count))
    }
}
