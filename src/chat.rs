use crate::app_state::APP_STATE;
use crate::models::message::Message;
use crate::models::payload::Payload;
use crate::models::user::User;
use crate::utils::play_notification_sound;
use std::sync::mpsc::Receiver;
use std::thread;

pub(crate) struct Chat {}

impl Chat {
    pub fn new() -> Self {
        let mut goat = User::new("GoatWithAccordion 🐐🪗");
        let example_messages = vec![
            Message::new(
                &mut goat,
                r#"Click the green "Connect" button (in the top-right corner) to connect.
After connecting, you'll see your address, UTXO count, and balance are updated.
The status in the bottom-left corner will change to "Connected". Enjoy! 🚀
IMPORTANT:
1) Never send your real Kaspa to anyone under any circumstances. Kaspeak operates exclusively in the testnet environment and uses only test currency. We do not accept donations or require fees for usage. Any real funds sent anywhere will be lost forever!
2) Do not use Kaspeak for transmitting anonymous or confidential information. Do not disclose personal data, schedule meetings, or conduct state affairs. Information transmitted through this service is publicly accessible across the entire test network, and we do not guarantee anonymity."#,
                0,
            ),
            Message::new(
                &mut goat,
                r#"Нажмите на зелёную кнопку «Connect» (в правом верхнем углу), чтобы подключиться.
После подключения Вы увидите, что адрес, число UTXO и Ваш баланс обновились.
Также в нижнем левом углу статус сменится на «Connected». Приятного пользования! 🚀
ВАЖНО:
1) Никому и никогда не переводите свою настоящую Kaspa. Kaspeak существует исключительно в рамках тестнета, и использует исключительно тестовую валюту. Мы не собираем пожертвования, не просим взносов за использование. Все отправленные куда-либо настоящие средства будут утрачены навсегда!
2) Не используйте Kaspeak для передачи анонимной и конфиденциальной информации. Не раскрывайте свои персональные данные, не планируйте встречи, не вершите государственные дела. Информация, переданная в рамках сервиса, общедоступна в рамках всей тестовой сети. Мы также не гарантируем Вам анонимность."#,
                0,
            ),
        ];
        APP_STATE.chat_state.add_message(0, example_messages[0].clone());
        APP_STATE.chat_state.add_message(0, example_messages[1].clone());
        Self {}
    }

    pub fn subscribe_to_channel(&self, chat_rx: Receiver<Payload>) {
        self.spawn_incoming_messages_thread(chat_rx);
    }

    /// Поток для чтения/обработки входящих сообщений
    fn spawn_incoming_messages_thread(&self, rx: Receiver<Payload>) {
        thread::spawn(move || {
            log::info!("Incoming message processing thread started");
            for payload in rx {
                //todo обновить список юзеров если юзера из сообщения не было
                log::info!("Received text message: {}", String::from_utf8_lossy(payload.get_data()));
                let message = Message::from_payload(payload);
                if let Err(err) = Chat::handle_incoming_message(message) {
                    log::error!("Error while processing message: {}", err);
                }
            }
            log::info!("Incoming message processing thread finished (channel closed).");
        });
    }

    fn handle_incoming_message(message: Message) -> Result<(), String> {
        let channel = message.channel;
        APP_STATE.chat_state.add_message(channel, message);

        APP_STATE.with_listener_state_read(|state| {
            // Уведомления только для текущего канала
            if state.channel_number == channel {
                thread::spawn(|| match play_notification_sound() {
                    Ok(_) => {}
                    Err(err) => {
                        log::error!("Error while playing notification sound: {}", err)
                    }
                });
            }
        })
    }
}
