use std::future::Future;
use std::io::Error;
use std::sync::mpsc::Receiver;
use std::sync::{mpsc, RwLock};
use std::sync::{Arc, Mutex};
use std::thread;

mod app_state;
mod audio;
mod chat;
mod constants;
mod core;
mod gui;
mod logs;
mod models;
mod settings;
mod utils;

use crate::audio::player::Player;
use crate::audio::recorder::Recorder;
use crate::gui::{Gui, GuiEvent};
use core::broadcaster::Broadcaster;
use core::listener::Listener;
use core::wallet::WalletService;

use kaspa_wrpc_client::result::Result;

use crate::app_state::APP_STATE;
use crate::chat::Chat;
use crate::constants::{APP_ICON_FILE_INLINED, MARKER, NETWORK_ID};
use crate::logs::logger;
use crate::models::instruction::Instruction;
use crate::models::payload::{MessageType, Payload};
use crate::models::recording::Recording;
use crate::utils::try_connect_to_node;
use iced::{window, Executor, Task};
use image::ImageFormat;
use kaspa_wrpc_client::KaspaRpcClient;
use rdev::{listen, EventType};
use tokio::main;
use tokio::runtime::Handle;
use tokio::sync::mpsc as async_mpsc;
use window::icon;
use workflow_core::channel::oneshot;

pub struct TokioExecutor {
    handle: Handle,
}

impl Executor for TokioExecutor {
    fn new() -> std::result::Result<TokioExecutor, Error> {
        Ok(Self { handle: Handle::current() })
    }

    fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        self.handle.spawn(future);
    }
}

#[main]
async fn main() -> Result<()> {
    logger::init();
    // Инициализация основных компонентов
    let (kaspa_rpc_client, broadcaster, listener, recorder, payload_tx) = init_core_components().await?;

    let recorder_rw_lock = match recorder {
        None => None,
        Some(recorder) => Some(Arc::new(RwLock::new(recorder))),
    };

    // Инициализация плеера и синхронного канала для него
    let (player_tx, player_rx) = mpsc::channel::<Payload>();
    let player = Player::new();
    spawn_player_thread(player, player_rx);

    // Инициализация чата и синхронного канала для него
    let (chat_tx, chat_rx) = mpsc::channel::<Payload>();
    let chat = Chat::new();
    spawn_chat_thread(chat, chat_rx);

    // Мосты payload(async) -> (player, payload_logger, chat)
    let payload_rx_logger = payload_tx.subscribe();
    let payload_rx_dispatcher = payload_tx.subscribe();
    spawn_payload_dispatcher_bridge(payload_rx_dispatcher, player_tx, chat_tx);
    spawn_payload_logger(payload_rx_logger);

    // Мост recorder -> broadcaster(async)
    let (recording_tx, recording_rx) = mpsc::channel::<Arc<Recording>>();
    spawn_recording_bridge(broadcaster.clone(), recording_rx);

    // (опционально) глобальный обработчик клавиши TAB
    // let is_recording = Arc::new(Mutex::new(false));
    // spawn_keyboard_listener(is_recording, event_tx.clone());

    // Обработчик сигналов завершения
    setup_signal_handler();

    // mpsc канал для событий от GUI
    let (event_tx, event_rx) = async_mpsc::channel::<GuiEvent>(100);
    // GUI event handler (запуск/остановка записи)
    spawn_gui_event_handler(recorder_rw_lock.clone(), kaspa_rpc_client.clone(), broadcaster.clone(), recording_tx.clone(), event_rx);

    // Запуск Iced GUI
    let cloned_event_tx = event_tx.clone();
    let icon = icon::from_file_data(APP_ICON_FILE_INLINED, Some(ImageFormat::Png)).expect("Failed to load application icon");
    let window_settings = window::Settings { icon: Some(icon), position: window::Position::Centered, ..Default::default() };
    let _ = iced::application("KASPEAK", Gui::update, Gui::view)
        .subscription(Gui::subscription)
        .theme(Gui::theme)
        .window(window_settings)
        .executor::<TokioExecutor>()
        .run_with(move || (Gui::new(cloned_event_tx), Task::none()));

    // Graceful Shutdown listener и broadcaster
    shutdown(listener.clone(), broadcaster.clone()).await;
    Ok(())
}

/// Инициализация основных компонентов: Kaspa RPC Client, Listener, Broadcaster, Recorder и payload каналы.
async fn init_core_components(
) -> Result<(Arc<KaspaRpcClient>, Arc<Broadcaster>, Arc<Listener>, Option<Recorder>, tokio::sync::broadcast::Sender<Payload>)> {
    let (payload_tx, _payload_rx) = tokio::sync::broadcast::channel(1024);

    let kaspa_rpc_client = utils::bootstrap_rpc_client(NETWORK_ID, None);

    let wallet_service = match WalletService::new(kaspa_rpc_client.clone(), APP_STATE.get_mnemonic()).await {
        Ok(wallet_service) => Arc::new(wallet_service),
        Err(err) => {
            return Err(kaspa_wrpc_client::error::Error::from(err.to_string()));
        }
    };

    let broadcaster = Arc::new(Broadcaster::try_new(kaspa_rpc_client.clone(), wallet_service)?);

    let listener = Arc::new(Listener::try_new(kaspa_rpc_client.clone(), payload_tx.clone())?);

    listener.start().await?;
    broadcaster.start().await?;

    let recorder = Recorder::try_new().unwrap();

    Ok((kaspa_rpc_client, broadcaster, listener, recorder, payload_tx))
}

/// Запуск плеера в отдельном потоке (синхронный player.run_blocking).
fn spawn_player_thread(player: Player, player_rx: Receiver<Payload>) {
    thread::spawn(move || {
        Arc::new(player).run_blocking(player_rx);
    });
}

/// Запуск чата в отдельном потоке
fn spawn_chat_thread(chat: Chat, chat_rx: Receiver<Payload>) {
    thread::spawn(move || {
        Arc::new(chat).subscribe_to_channel(chat_rx);
    });
}

/// Мост между Payload данными (асинхронными) и потребителями этих данных
/// Читаем Payload пакеты из broadcast (async) и пересылаем в синхронный канал потребителя.
fn spawn_payload_dispatcher_bridge(
    mut rx_player: tokio::sync::broadcast::Receiver<Payload>,
    player_tx: mpsc::Sender<Payload>,
    chat_tx: mpsc::Sender<Payload>,
) {
    tokio::spawn(async move {
        while let Ok(payload) = rx_player.recv().await {
            match payload.get_message_type() {
                MessageType::Voice => {
                    let player_tx = player_tx.clone();
                    tokio::task::spawn_blocking(move || {
                        let _ = player_tx.send(payload);
                    });
                }
                MessageType::Text => {
                    let chat_tx = chat_tx.clone();
                    tokio::task::spawn_blocking(move || {
                        let _ = chat_tx.send(payload);
                    });
                }
                MessageType::File | MessageType::Unknown(_) => {
                    log::error!("Messages of this type are not yet implemented");
                }
            }
        }
    });
}

/// Логируем payload для отладки.
fn spawn_payload_logger(mut payload_rx_logger: tokio::sync::broadcast::Receiver<Payload>) {
    tokio::spawn(async move {
        while let Ok(payload) = payload_rx_logger.recv().await {
            log::debug!(
                "[Logging Payload Subscriber] Received payload {}, data length {} bytes",
                String::from_utf8_lossy(MARKER),
                payload.get_data().len()
            );
        }
    });
}

/// Мост от синхронных фрагментов записи (Recorder) к асинхронному каналу (Broadcaster).
/// Recorder пишет в recording_tx (sync), мы читаем из recording_rx и пересылаем в async_mpsc.
fn spawn_recording_bridge(broadcaster: Arc<Broadcaster>, recording_rx: Receiver<Arc<Recording>>) {
    let (async_tx, mut async_rx) = async_mpsc::channel::<Arc<Recording>>(10);

    // Поток для переноса данных из sync в async
    thread::spawn(move || {
        while let Ok(recording) = recording_rx.recv() {
            if let Err(e) = async_tx.blocking_send(recording) {
                log::error!("Error while sending data to asynchronous channel: {}", e);
                break;
            }
        }
    });

    // Асинхронная задача: читает из async_rx и отправляет броадкастеру инструкции.
    tokio::spawn(async move {
        while let Some(fragment) = async_rx.recv().await {
            log::debug!(
                "Sending instruction to broadcaster: state={:?}, num={}, size={}",
                fragment.state,
                fragment.fragment_num,
                fragment.audio.len()
            );
            let instruction = Instruction::try_from_recording(fragment.as_ref());
            check_and_send_instruction(broadcaster.clone(), instruction).await;
        }
    });
}

/// Обработчик клавиатуры (TAB): начало/остановка аудиозаписи
fn spawn_keyboard_listener(is_recording: Arc<Mutex<bool>>, event_tx: async_mpsc::Sender<GuiEvent>) {
    thread::spawn(move || {
        if let Err(error) = listen(move |event| match event.event_type {
            EventType::KeyPress(key) if key == rdev::Key::Tab => {
                let mut recording = is_recording.lock().unwrap();
                if !*recording {
                    *recording = true;
                    let _ = event_tx.blocking_send(GuiEvent::StartRecording);
                }
            }
            EventType::KeyRelease(key) if key == rdev::Key::Tab => {
                let mut recording = is_recording.lock().unwrap();
                if *recording {
                    *recording = false;
                    let _ = event_tx.blocking_send(GuiEvent::StopRecording);
                }
            }
            _ => {}
        }) {
            log::error!("Input error: {:?}", error);
        }
    });
}

/// Установка обработчика сигналов завершения (Ctrl+C, SIGTERM).
fn setup_signal_handler() {
    let (shutdown_sender, _shutdown_receiver) = oneshot::<()>();
    let ctrlc_sender_channel = shutdown_sender.clone();
    ctrlc::set_handler(move || {
        log::info!("^SIGTERM - shutting down...");
        ctrlc_sender_channel.try_send(()).expect("Shutdown signal error");
    })
    .expect("Failed to set Ctrl+C handler");
}

/// Обработка событий GUI: при начале записи — запускаем recorder.run_blocking(...) в отдельном потоке,
/// при остановке — вызываем recorder.stop_recording().
fn spawn_gui_event_handler(
    recorder: Option<Arc<RwLock<Recorder>>>,
    kaspa_rpc_client: Arc<KaspaRpcClient>,
    broadcaster: Arc<Broadcaster>,
    recording_tx: mpsc::Sender<Arc<Recording>>,
    mut event_rx: async_mpsc::Receiver<GuiEvent>,
) {
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let kaspa_rpc_client = kaspa_rpc_client.clone();
            match event {
                GuiEvent::StartRecording => {
                    if let Some(recorder) = recorder.as_ref() {
                        let recorder_ref = Arc::clone(&recorder);
                        let tx_clone = recording_tx.clone();
                        thread::spawn(move || {
                            recorder_ref.read().unwrap().run_blocking(tx_clone);
                        });
                    }
                }
                GuiEvent::StopRecording => {
                    if let Some(recorder) = recorder.as_ref() {
                        recorder.read().unwrap().stop_recording();
                    }
                }
                GuiEvent::NodeConnectButtonPressed(node_url) => {
                    try_connect_to_node(kaspa_rpc_client, node_url.clone()).await;
                }
                GuiEvent::MessageSent(message) => {
                    let instruction = Instruction::try_from_message(message);
                    check_and_send_instruction(broadcaster.clone(), instruction).await;
                }
            }
        }
    });
}

/// Корректное завершение Listener и Broadcaster.
async fn shutdown(listener: Arc<Listener>, broadcaster: Arc<Broadcaster>) {
    listener.stop().await.unwrap();
    broadcaster.stop().await.unwrap();
}

async fn check_and_send_instruction(broadcaster: Arc<Broadcaster>, instruction: Result<Instruction>) {
    match instruction {
        Ok(instruction) => {
            if let Err(e) = broadcaster.send_instruction(instruction).await {
                log::error!("Error while sending instruction to broadcaster: {}", e);
            }
        }
        Err(e) => {
            log::error!("Error while generating instruction for broadcaster: {}", e);
        }
    }
}
