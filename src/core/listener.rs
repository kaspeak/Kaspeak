use futures::{select_biased, FutureExt};
use std::sync::{Arc, Mutex};
use workflow_core::{
    channel::{Channel, DuplexChannel},
    task::spawn,
};

use crate::app_state::APP_STATE;
use crate::constants::{MARKER, MAX_TEXT_CHARS};
use crate::models::payload::{MessageType, Payload};
use crate::utils::LimitedHashSet;
use kaspa_wallet_core::prelude::*;
use kaspa_wrpc_client::{prelude::*, result::Result};
use tokio::sync::broadcast::Sender;

/// Структура для хранения внутреннего состояния листнера
pub struct ListenerInner {
    // Дуплексный канал для управления задачами
    task_ctl: DuplexChannel<()>,
    // Экземпляр Kaspa wRPC клиента
    client: Arc<KaspaRpcClient>,
    // Канал для получения уведомлений от ноды Kaspa
    notification_channel: Channel<Notification>,
    // Идентификатор листнера для Kaspa RPC
    listener_id: Mutex<Option<ListenerId>>,
    // Хранилище обработанных транзакций для уникальности пейлоадов
    processed_transactions: Mutex<LimitedHashSet<String>>,
    // Броадкаст часть канала для потребителей пейлоадов
    broadcast_sender: Sender<Payload>,
}

#[derive(Clone)]
pub struct Listener {
    pub inner: Arc<ListenerInner>,
}

impl Listener {
    pub fn try_new(client: Arc<KaspaRpcClient>, broadcast_sender: Sender<Payload>) -> Result<Self> {
        let inner = ListenerInner {
            task_ctl: DuplexChannel::oneshot(),
            client,
            notification_channel: Channel::unbounded(),
            listener_id: Mutex::new(None),
            processed_transactions: Mutex::new(LimitedHashSet::new(100000)),
            broadcast_sender,
        };

        Ok(Self { inner: Arc::new(inner) })
    }

    /// Проверка текущего состояния подключения к ноде
    fn is_connected() -> bool {
        APP_STATE.is_listener_connected().unwrap_or_else(|err| {
            log::error!("Error while retrieving is_connected flag: {}", err);
            false
        })
    }

    /// Запуск листнера
    pub async fn start(&self) -> Result<()> {
        // Запуск задачи обработки событий
        self.start_event_task().await?;

        Ok(())
    }

    /// Остановка листнера
    pub async fn stop(&self) -> Result<()> {
        self.client().disconnect().await?;
        self.stop_event_task().await?;
        Ok(())
    }

    /// Получение ссылки на rpc клиент
    pub fn client(&self) -> &Arc<KaspaRpcClient> {
        &self.inner.client
    }

    /// Регистрация листнеров уведомлений в RPC API
    async fn register_notification_listeners(&self) -> Result<()> {
        // Регистрация нового слушателя
        let listener_id = self.client().rpc_api().register_new_listener(ChannelConnection::new(
            "mkga-node-subscriber",
            self.inner.notification_channel.sender.clone(),
            ChannelType::Persistent,
        ));

        *self.inner.listener_id.lock().unwrap() = Some(listener_id);

        // Подписка на уведомления о добавленных блоках
        self.client().rpc_api().start_notify(listener_id, Scope::BlockAdded(BlockAddedScope {})).await?;
        Ok(())
    }

    /// Отмена регистрации листнера уведомлений
    async fn unregister_notification_listener(&self) -> Result<()> {
        let id_option = {
            let mut guard = self.inner.listener_id.lock().unwrap();
            guard.take()
        };
        if let Some(id) = id_option {
            self.client().rpc_api().unregister_listener(id).await?;
        }
        Ok(())
    }

    /// Обработка уведомлений от ноды
    async fn handle_notification(&self, notification: Notification) -> Result<()> {
        if let Notification::BlockAdded(not) = notification {
            // Обработка полезных данных транзакций
            for _tx in not.block.transactions.clone() {
                if !_tx.payload.starts_with(MARKER) {
                    continue;
                }
                let tx_verbose = match &_tx.verbose_data {
                    Some(vd) => vd.clone(),
                    None => {
                        log::error!("No verbose_data in this transaction");
                        continue;
                    }
                };
                let tx_id: String = tx_verbose.transaction_id.to_string();
                {
                    let mut processed = self.inner.processed_transactions.lock().unwrap();
                    if processed.contains(&tx_id) {
                        continue;
                    }
                    processed.insert(tx_id.clone());
                }

                /*let sender_id = _tx
                .outputs
                .iter()
                .max_by_key(|out| out.value)
                .map_or(None, |out| Some(out.verbose_data.clone().unwrap()))
                .map_or(None, |vd| Some(vd.script_public_key_address));*/

                let mut payload = match Payload::from_bytes(&_tx.payload) {
                    Ok(payload) => {
                        log::info!("Received payload: {} (tx_id={})", payload.debug_string(), tx_id);
                        payload
                    }
                    Err(err) => {
                        log::error!("Error while parsing payload: {} (tx_id={})", err, tx_id);
                        continue;
                    }
                };
                match payload.get_message_type() {
                    MessageType::Voice => {
                        if self.filter_incoming_voice(&payload).await {
                            if let Err(err) = payload.decompress_zstd() {
                                log::error!("Error while decompressing audio: {}", err);
                                continue;
                            }
                            self.broadcast_payload(payload).await?;
                        }
                    }
                    MessageType::Text => {
                        if let Err(err) = payload.decompress_zstd() {
                            log::error!("Error while decompressing text: {}", err);
                            continue;
                        }
                        if self.filter_incoming_text(&payload).await {
                            self.broadcast_payload(payload).await?;
                        }
                    }
                    MessageType::File | MessageType::Unknown(_) => {
                        log::warn!("Unsupported message type");
                    }
                }
            }
        }
        Ok(())
    }

    async fn filter_incoming_voice(&self, payload: &Payload) -> bool {
        let self_username = APP_STATE.get_username();
        let listen_self = APP_STATE.is_listen_self().unwrap_or(false);
        let mute_all = APP_STATE.is_mute_all().unwrap_or(false);
        let channel_number = APP_STATE.get_channel_number().unwrap_or(0);
        if mute_all {
            return false;
        }
        if payload.get_username() == self_username && !listen_self {
            return false;
        }
        if payload.get_channel() != channel_number {
            return false;
        }

        true
    }

    async fn filter_incoming_text(&self, payload: &Payload) -> bool {
        if let Ok(txt) = std::str::from_utf8(payload.get_data()) {
            let char_count = txt.chars().count();
            if char_count > MAX_TEXT_CHARS {
                log::warn!("Text data has {} chars, max allowed is {}", char_count, MAX_TEXT_CHARS);
                return false;
            }
        } else {
            return false;
        }
        true
    }

    // Обработка события подключения
    async fn handle_connect(&self) -> Result<()> {
        log::info!("Connected to {:?}", self.client().url());

        // Получение информации о сервере
        let server_info = self.client().get_server_info().await?;
        log::info!("Server info: {server_info:?}");

        // Регистрация уведомлений
        self.register_notification_listeners().await?;

        // Обновление состояния подключения
        if let Err(e) = APP_STATE.set_listener_connected(true) {
            log::error!("Error while setting is_connected flag: {}", &e);
        }

        Ok(())
    }

    /// Обработка события отключения
    async fn handle_disconnect(&self) -> Result<()> {
        log::info!("Disconnected from {:?}", self.client().url());

        // Отмена регистрации уведомлений
        self.unregister_notification_listener().await?;

        if let Err(e) = APP_STATE.set_listener_connected(false) {
            log::error!("Error while setting is_connected flag: {}", &e);
        }

        Ok(())
    }

    /// Запуск задачи обработки событий
    async fn start_event_task(&self) -> Result<()> {
        let listener = self.clone();
        let rpc_ctl_channel = self.client().rpc_ctl().multiplexer().channel();
        let task_ctl_receiver = self.inner.task_ctl.request.receiver.clone();
        let task_ctl_sender = self.inner.task_ctl.response.sender.clone();
        let notification_receiver = self.inner.notification_channel.receiver.clone();

        spawn(async move {
            loop {
                select_biased! {
                    msg = rpc_ctl_channel.receiver.recv().fuse() => {
                        if let Ok(msg) = msg {
                            match msg {
                                RpcState::Connected => {
                                    if let Err(err) = listener.handle_connect().await {
                                        log::error!("Error while connecting: {err}");
                                    }
                                },
                                RpcState::Disconnected => {
                                    if let Err(err) = listener.handle_disconnect().await {
                                        log::error!("Error while disconnecting: {err}");
                                    }
                                },
                            }
                        } else {
                            log::error!("RPC CTL channel error");
                            break;
                        }
                    },
                    notification = notification_receiver.recv().fuse() => {
                        if let Ok(notification) = notification {
                            if let Err(err) = listener.handle_notification(notification).await {
                                log::error!("Error while processing notification: {err}");
                            }
                        } else {
                            log::error!("Notification channel error");
                            break;
                        }
                    },
                    _ = task_ctl_receiver.recv().fuse() => {
                        break;
                    },
                }
            }

            log::info!("Event loop task has finished");

            if Self::is_connected() {
                listener.handle_disconnect().await.unwrap_or_else(|err| log::error!("Error while disconnecting: {err}"));
            }

            task_ctl_sender.send(()).await.unwrap();
        });
        Ok(())
    }

    /// Остановка задачи обработки событий
    async fn stop_event_task(&self) -> Result<()> {
        self.inner.task_ctl.signal(()).await.expect("Failed to stop the event-processing task");
        Ok(())
    }

    async fn broadcast_payload(&self, payload: Payload) -> Result<()> {
        let tx = self.inner.broadcast_sender.clone();
        match tx.send(payload) {
            Ok(_) => Ok(()),
            Err(err) => {
                log::error!("Error while broadcasting payload: {}", err);
                Ok(())
            }
        }
    }
}
