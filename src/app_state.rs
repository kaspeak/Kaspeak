use crate::constants::{DEFAULT_CHANNEL, DEFAULT_FEE_LEVEL, MAX_CHANNEL_CAPACITY};
use crate::models::message::Message;
use crate::settings::Settings;
use crate::utils::gather_audio_devices;
use config::ConfigError;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::Device;
use dashmap::DashMap;
use kaspa_wallet_core::prelude::Address;
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

pub(crate) struct RecorderSharedState {
    pub(crate) available_input_devices: Vec<String>,
    pub(crate) selected_input_device: Option<Device>,
    pub(crate) input_device_changed: Arc<AtomicBool>,
}

pub(crate) struct ChatSharedState {
    pub(crate) messages_by_channel: DashMap<u32, Vec<Message>>,
}

impl ChatSharedState {
    pub fn add_message(&self, channel: u32, message: Message) {
        let mut messages = self.messages_by_channel.entry(channel).or_insert_with(Vec::new);

        messages.push(message);
        if messages.len() > MAX_CHANNEL_CAPACITY {
            messages.remove(0);
        }
    }

    pub fn clear(&self) {
        self.messages_by_channel.clear();
    }
}

pub(crate) struct ListenerSharedState {
    pub(crate) channel_number: u32,
    pub(crate) listen_self: Arc<AtomicBool>,
    pub(crate) mute_all: Arc<AtomicBool>,
    pub(crate) is_connected: Arc<AtomicBool>,
}

pub(crate) struct BroadcasterSharedState {
    pub(crate) address: Option<Address>,
    pub(crate) fee_size: u64,
    pub(crate) balance: u64,
    pub(crate) utxos: usize,
    pub(crate) is_connected: Arc<AtomicBool>,
}

/// Основное состояние приложения
pub struct AppState {
    pub(crate) listener_state: Arc<RwLock<ListenerSharedState>>,
    pub(crate) recorder_state: Arc<RwLock<RecorderSharedState>>,
    pub(crate) broadcaster_state: Arc<RwLock<BroadcasterSharedState>>,
    pub(crate) chat_state: ChatSharedState,
    pub(crate) mnemonic: String,
    pub(crate) username: String,
    settings: Arc<Mutex<Settings>>,
}

lazy_static! {
    pub static ref APP_STATE: Arc<AppState> = Arc::new(AppState::new().expect("Failed to initialize AppState"));
}

impl AppState {
    pub(crate) fn new() -> Result<AppState, ConfigError> {
        let mut settings = Settings::new();
        match settings.load() {
            Ok(_) => {
                log::info!("Settings loaded successfully");
            }
            Err(e) => {
                if e == "NoFile" {
                    log::info!("No settings.kspk found. Creating new settings...");
                    settings.initialize_settings().map_err(|err| {
                        log::error!("Failed to init default settings: {}", err);
                        ConfigError::Message(err)
                    })?;
                } else {
                    return Err(ConfigError::Message(e));
                }
            }
        }

        let mnemonic = settings.current.mnemonic.clone();
        let username = settings.current.username.clone();

        let (available_input_devices, selected_input_device) = gather_audio_devices();
        let listener_state = Self::create_listener_state();
        let recorder_state = Self::create_recorder_state(available_input_devices, selected_input_device);
        let broadcaster_state = Self::create_broadcaster_state();
        let chat_state = Self::create_chat_state();

        Ok(Self {
            listener_state,
            recorder_state,
            broadcaster_state,
            chat_state,
            mnemonic,
            username,
            settings: Arc::new(Mutex::new(settings)),
        })
    }

    fn create_recorder_state(
        available_input_devices: Vec<String>,
        selected_input_device: Option<Device>,
    ) -> Arc<RwLock<RecorderSharedState>> {
        Arc::new(RwLock::new(RecorderSharedState {
            available_input_devices,
            selected_input_device,
            input_device_changed: Arc::new(AtomicBool::new(false)),
        }))
    }

    fn create_listener_state() -> Arc<RwLock<ListenerSharedState>> {
        Arc::new(RwLock::new(ListenerSharedState {
            channel_number: DEFAULT_CHANNEL,
            listen_self: Arc::new(AtomicBool::new(false)),
            mute_all: Arc::new(AtomicBool::new(false)),
            is_connected: Arc::new(AtomicBool::new(false)),
        }))
    }

    fn create_broadcaster_state() -> Arc<RwLock<BroadcasterSharedState>> {
        Arc::new(RwLock::new(BroadcasterSharedState {
            address: None,
            fee_size: DEFAULT_FEE_LEVEL,
            balance: 0,
            utxos: 0,
            is_connected: Arc::new(AtomicBool::new(false)),
        }))
    }

    fn create_chat_state() -> ChatSharedState {
        ChatSharedState { messages_by_channel: DashMap::new() }
    }

    /// # todo
    pub fn update_selected_input_device(&self, device_name: &str) -> Result<(), String> {
        let mut recorder_state = self.recorder_state.write().map_err(|_| "Lock poisoned")?;

        // Поиск устройства по имени
        let device = cpal::default_host()
            .input_devices()
            .map_err(|e| format!("Failed to get input devices list: {:?}", e))?
            .find(|d| d.name().ok().as_deref() == Some(device_name));

        if let Some(device) = device {
            recorder_state.selected_input_device = Some(device);
            recorder_state.input_device_changed.store(true, Ordering::SeqCst);
            Ok(())
        } else {
            Err(format!("Input device '{}' not found.", device_name))
        }
    }

    /// Метод для чтения recorder_state
    pub fn with_recorder_state_read<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&RecorderSharedState) -> R,
    {
        let guard = self.recorder_state.read().map_err(|_| "Lock poisoned")?;
        Ok(f(&*guard))
    }

    /// Метод для записи в recorder_state
    pub fn with_recorder_state_write<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut RecorderSharedState) -> Result<R, String>,
    {
        let mut guard = self.recorder_state.write().map_err(|_| "Lock poisoned")?;
        f(&mut *guard)
    }

    /// Метод для чтения listener_state
    pub fn with_listener_state_read<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&ListenerSharedState) -> R,
    {
        let guard = self.listener_state.read().map_err(|_| "Lock poisoned")?;
        Ok(f(&*guard))
    }

    /// Метод для записи в listener_state
    pub fn with_listener_state_write<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut ListenerSharedState) -> Result<R, String>,
    {
        let mut guard = self.listener_state.write().map_err(|_| "Lock poisoned")?;
        f(&mut *guard)
    }

    /// Метод для чтения broadcaster_state
    pub fn with_broadcaster_state_read<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&BroadcasterSharedState) -> R,
    {
        let guard = self.broadcaster_state.read().map_err(|_| "Lock poisoned")?;
        Ok(f(&*guard))
    }

    /// Метод для записи в broadcaster_state
    pub fn with_broadcaster_state_write<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut BroadcasterSharedState) -> Result<R, String>,
    {
        let mut guard = self.broadcaster_state.write().map_err(|_| "Lock poisoned")?;
        f(&mut *guard)
    }

    /// Единый метод для записи Settings.
    pub fn with_settings_write<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut Settings) -> Result<R, String>,
    {
        let mut guard = self.settings.lock().map_err(|_| "Mutex (settings) poisoned")?;
        f(&mut *guard)
    }

    // ------------------------------------------
    // RecorderSharedState helpers
    // ------------------------------------------

    /// Проверяет, был ли изменен input_device_changed
    pub fn is_input_device_changed(&self) -> Result<bool, String> {
        self.with_recorder_state_read(|recorder_state| {
            let changed = recorder_state.input_device_changed.load(Ordering::SeqCst);
            changed
        })
    }

    /// Устанавливает флаг input_device_changed
    pub fn set_input_device_changed(&self, value: bool) -> Result<(), String> {
        self.with_recorder_state_write(|recorder_state| {
            recorder_state.input_device_changed.store(value, Ordering::SeqCst);
            Ok(())
        })
    }

    /// Возвращает текущее выбранное устройство ввода
    pub fn get_selected_input_device(&self) -> Result<Option<Device>, String> {
        self.with_recorder_state_read(|recorder_state| recorder_state.selected_input_device.clone())
    }

    /// Устанавливает выбранное устройство ввода
    pub fn set_selected_input_device(&self, device: Option<Device>) -> Result<(), String> {
        self.with_recorder_state_write(|recorder_state| {
            recorder_state.selected_input_device = device;
            Ok(())
        })
    }

    // ------------------------------------------
    // ListenerSharedState helpers
    // ------------------------------------------

    /// Возвращает текущий канал
    pub fn get_channel_number(&self) -> Result<u32, String> {
        self.with_listener_state_read(|listener_state| listener_state.channel_number)
    }

    /// Устанавливает текущий канал
    pub fn set_channel_number(&self, channel_number: u32) -> Result<(), String> {
        self.with_listener_state_write(|listener_state| {
            listener_state.channel_number = channel_number;
            Ok(())
        })
    }

    /// Проверяет, включена ли функция прослушивания собственных пакетов
    pub fn is_listen_self(&self) -> Result<bool, String> {
        self.with_listener_state_read(|listener_state| listener_state.listen_self.load(Ordering::SeqCst))
    }

    /// Устанавливает флаг прослушивания собственных пакетов
    pub fn set_listen_self(&self, value: bool) -> Result<(), String> {
        self.with_listener_state_write(|listener_state| {
            listener_state.listen_self.store(value, Ordering::SeqCst);
            Ok(())
        })
    }

    /// Проверяет, включен ли mute_all
    pub fn is_mute_all(&self) -> Result<bool, String> {
        self.with_listener_state_read(|listener_state| listener_state.mute_all.load(Ordering::SeqCst))
    }

    /// Устанавливает флаг mute_all
    pub fn set_mute_all(&self, value: bool) -> Result<(), String> {
        self.with_listener_state_write(|listener_state| {
            listener_state.mute_all.store(value, Ordering::SeqCst);
            Ok(())
        })
    }

    /// Проверяет, подключен ли Listener
    pub fn is_listener_connected(&self) -> Result<bool, String> {
        self.with_listener_state_read(|state| state.is_connected.load(Ordering::SeqCst))
    }

    /// Устанавливает статус подключения Listener
    pub fn set_listener_connected(&self, connected: bool) -> Result<(), String> {
        self.with_listener_state_write(|state| {
            state.is_connected.store(connected, Ordering::SeqCst);
            Ok(())
        })
        .map(|_| {
            log::info!("Listener is {}active.", if connected { "" } else { "not " });
        })
    }

    // ------------------------------------------
    // Методы для доступа к полям BroadcasterSharedState
    // ------------------------------------------

    /// Получить размер комиссии
    pub fn get_fee_size(&self) -> Result<u64, String> {
        self.with_broadcaster_state_read(|state| state.fee_size)
    }

    /// Установить размер комиссии
    pub fn set_fee_size(&self, fee_size: u64) -> Result<(), String> {
        self.with_broadcaster_state_write(|state| {
            state.fee_size = fee_size;
            Ok(())
        })
    }

    /// Получить баланс
    pub fn get_balance(&self) -> Result<u64, String> {
        self.with_broadcaster_state_read(|state| state.balance)
    }

    /// Установить баланс
    pub fn set_balance(&self, balance: u64) -> Result<(), String> {
        self.with_broadcaster_state_write(|state| {
            state.balance = balance;
            Ok(())
        })
    }

    /// Получить адрес аккаунта
    pub fn get_account_address(&self) -> Result<Option<String>, String> {
        self.with_broadcaster_state_read(|state| state.address.clone().map_or(None, |addr| Some(addr.address_to_string())))
    }

    /// Установить адрес аккаунта
    pub fn set_account_address(&self, address: Option<Address>) -> Result<(), String> {
        self.with_broadcaster_state_write(|state| {
            state.address = address;
            Ok(())
        })
    }

    /// Получить количество UTXO
    pub fn get_utxos(&self) -> Result<usize, String> {
        self.with_broadcaster_state_read(|state| state.utxos)
    }

    /// Установить количество UTXO
    pub fn set_utxos(&self, utxos: usize) -> Result<(), String> {
        self.with_broadcaster_state_write(|state| {
            state.utxos = utxos;
            Ok(())
        })
    }

    /// Проверяет, подключен ли Broadcaster
    pub fn is_broadcaster_connected(&self) -> Result<bool, String> {
        self.with_broadcaster_state_read(|state| state.is_connected.load(Ordering::SeqCst))
    }

    /// Устанавливает статус подключения Broadcaster
    pub fn set_broadcaster_connected(&self, connected: bool) -> Result<(), String> {
        self.with_broadcaster_state_write(|state| {
            state.is_connected.store(connected, Ordering::SeqCst);
            Ok(())
        })
        .map(|_| {
            log::info!("Broadcaster is {}active.", if connected { "" } else { "not " });
        })
    }

    pub fn get_username(&self) -> String {
        self.username.clone()
    }

    pub fn set_username(&mut self, new_username: &str) -> Result<(), String> {
        self.username = new_username.to_string();
        self.with_settings_write(|settings| {
            settings.current.username = new_username.to_string();
            settings.save()?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn get_mnemonic(&self) -> String {
        self.mnemonic.clone()
    }
}
