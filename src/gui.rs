use crate::app_state::APP_STATE;
use crate::constants::{MAX_TEXT_CHARS, UNIT};
use crate::models::message::Message as ChatMessage;
use crate::models::user::User;
use crate::utils::shorten_address;
use cpal::traits::DeviceTrait;
use iced::keyboard::{self, key};
use iced::widget::{
    button, column, container, pick_list, rich_text, row, scrollable, span, text, text_editor, text_input, toggler, Column, Row, Rule,
};
use iced::{font, time, Subscription, Task};
use iced::{Alignment, Color, Length, Theme};
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum GuiEvent {
    StartRecording,
    StopRecording,
    NodeConnectButtonPressed(Option<String>),
    MessageSent(String),
}

#[derive(Debug, Clone)]
pub enum Message {
    ToggleRecording(bool),
    StartDone(Result<(), String>),
    StopDone(Result<(), String>),
    UpdateInputDevice(String),
    UpdateOutputDevice(String),
    InputNodeAddress(String),
    ConnectNodeAddress,
    NodeConnectComplete(Result<(), String>),
    ToggleListenSelf(bool),
    ToggleMuteAll(bool),
    ThemeChanged(Theme),
    ChatEditorAction(text_editor::Action),
    ChatSendPressed,
    SendMessageDone(Result<(), String>),
    FeeInputChanged(String),
    ChannelInputChanged(String),
    OpenLink(String),
    Tick,
}

pub struct Gui {
    event_tx: Sender<GuiEvent>,
    chat_scroll_id: scrollable::Id,
    is_recording: bool,
    listen_self: bool,
    mute_all: bool,
    full_address: String,
    display_address: String,
    username: String,
    node_address: String,

    // Размер комиссии (цельные сомпи)
    fee_size: u64,
    channel_number: u32,

    input_device: String,
    output_device: String,
    selected_theme: Theme,
    available_input_devices: Vec<String>,

    fee_size_input: String,
    channel_number_input: String,

    users: Vec<User>,
    chat_messages: Vec<ChatMessage>,
    last_seen_message_id: Option<Uuid>,
    chat_editor_content: text_editor::Content,
}

impl Gui {
    pub fn new(event_tx: Sender<GuiEvent>) -> Self {
        let app_state = APP_STATE.clone();
        let recorder_state = app_state.recorder_state.read().unwrap();

        let available_devices = recorder_state.available_input_devices.clone();
        // todo нужно больше ручек
        let input_device = recorder_state
            .selected_input_device
            .as_ref()
            .and_then(|device| device.name().ok())
            .unwrap_or_else(|| "Default Device".to_string());

        let users = vec![];

        let default_fee = app_state.get_fee_size().unwrap_or(0);
        let default_channel: u32 = app_state.get_channel_number().unwrap_or(0);

        let full_address = match APP_STATE.get_account_address() {
            Ok(val) => val.unwrap_or("Empty".to_string()),
            Err(e) => {
                log::error!("Error while reading address: {}", e);
                "Error".to_string()
            }
        };
        let display_address = shorten_address(&full_address);

        Self {
            event_tx,
            chat_scroll_id: scrollable::Id::unique(),
            is_recording: false,
            listen_self: false,
            mute_all: false,
            full_address,
            display_address,
            username: app_state.get_username(),
            // node_address: "ws://127.0.0.1:17310".to_string(),
            node_address: "".to_string(),
            fee_size: default_fee,
            channel_number: default_channel,
            input_device,
            output_device: "Speaker 1".to_string(),
            selected_theme: Theme::Oxocarbon,
            available_input_devices: available_devices,
            fee_size_input: default_fee.to_string(),
            channel_number_input: default_channel.to_string(),
            users,
            chat_messages: Vec::new(),
            last_seen_message_id: None,
            chat_editor_content: text_editor::Content::new(),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleRecording(value) => {
                let tx = self.event_tx.clone();
                if value {
                    Task::perform(
                        async move { tx.send(GuiEvent::StartRecording).await.map_err(|e| e.to_string()) },
                        Message::StartDone,
                    )
                } else {
                    Task::perform(async move { tx.send(GuiEvent::StopRecording).await.map_err(|e| e.to_string()) }, Message::StopDone)
                }
            }
            Message::StartDone(result) => {
                match result {
                    Ok(_) => self.is_recording = true,
                    Err(e) => log::error!("Error while enabling recording: {}", e),
                }
                Task::none()
            }
            Message::StopDone(result) => {
                match result {
                    Ok(_) => self.is_recording = false,
                    Err(e) => log::error!("Error while disabling recording: {}", e),
                }
                Task::none()
            }
            Message::UpdateInputDevice(device_name) => {
                self.input_device = device_name.clone();
                if let Err(e) = APP_STATE.update_selected_input_device(&device_name) {
                    log::error!("Error changing input device: {}", e);
                } else {
                    log::info!("Input device successfully changed to '{}'", device_name);
                }
                Task::none()
            }
            Message::InputNodeAddress(value) => {
                self.node_address = value;
                log::info!("Node address: {:?}", self.node_address);
                Task::none()
            }
            Message::ConnectNodeAddress => {
                let tx = self.event_tx.clone();
                let node_url = match self.node_address.clone() {
                    str if str.is_empty() => None,
                    url => Some(url),
                };
                log::info!("Trying to connect to node: {:?}", node_url);
                Task::perform(
                    async move { tx.send(GuiEvent::NodeConnectButtonPressed(node_url)).await.map_err(|e| e.to_string()) },
                    Message::NodeConnectComplete,
                )
            }
            Message::NodeConnectComplete(_) => {
                //todo()
                Task::none()
            }
            Message::UpdateOutputDevice(value) => {
                self.output_device = value;
                Task::none()
            }
            Message::ToggleListenSelf(value) => {
                match APP_STATE.set_listen_self(value) {
                    Ok(_) => {
                        self.listen_self = value;
                        log::info!("Listening to own packets is {}active.", if value { "" } else { "not " });
                    }
                    Err(err) => {
                        log::error!("Error enabling / disabling self-listening: {}", err)
                    }
                }
                Task::none()
            }
            Message::ToggleMuteAll(value) => {
                match APP_STATE.set_mute_all(value) {
                    Ok(_) => {
                        self.mute_all = value;
                        log::info!("Playback of all audio packets is {}active.", if value { "not " } else { "" });
                    }
                    Err(err) => {
                        log::error!("Error enabling / disabling global mute: {}", err)
                    }
                }
                Task::none()
            }
            Message::ThemeChanged(theme) => {
                self.selected_theme = theme;
                Task::none()
            }

            // --- Многострочный ввод text_editor, постобработка ---
            Message::ChatEditorAction(action) => {
                let old_text = self.chat_editor_content.text().to_string();
                self.chat_editor_content.perform(action);

                let new_text = self.chat_editor_content.text();
                let new_char_count = new_text.chars().count();
                if new_char_count > MAX_TEXT_CHARS {
                    self.chat_editor_content = text_editor::Content::with_text(&old_text);
                }
                Task::none()
            }
            Message::ChatSendPressed => {
                let text = self.chat_editor_content.text().trim().to_string();
                if !text.is_empty() {
                    let tx = self.event_tx.clone();
                    Task::perform(
                        async move {
                            let send_res = tx.send(GuiEvent::MessageSent(text)).await.map_err(|e| e.to_string());
                            if let Err(err) = send_res {
                                return Err(err);
                            }
                            Ok(())
                        },
                        Message::SendMessageDone,
                    )
                } else {
                    Task::none()
                }
            }
            Message::SendMessageDone(result) => match result {
                Ok(_) => {
                    self.chat_editor_content = text_editor::Content::new();
                    Task::none()
                }
                Err(err) => {
                    log::error!("Error while sending message: {}", err);
                    Task::none()
                }
            },
            Message::Tick => {
                let channel_number = APP_STATE.get_channel_number().unwrap_or(0);
                let new_messages =
                    APP_STATE.chat_state.messages_by_channel.get(&channel_number).map(|messages| messages.clone()).unwrap_or_default();
                let last_new_id = new_messages.last().map(|msg| msg.get_id());
                let need_scroll = match (last_new_id, self.last_seen_message_id) {
                    (Some(new_id), Some(old_id)) => new_id != old_id,
                    (Some(_), None) => true,
                    _ => false,
                };
                self.chat_messages = new_messages;
                if need_scroll {
                    self.last_seen_message_id = last_new_id;
                    return scrollable::snap_to(self.chat_scroll_id.clone(), scrollable::RelativeOffset::END);
                }
                Task::none()
            }
            Message::FeeInputChanged(value) => {
                let filtered: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
                if filtered.is_empty() {
                    self.fee_size = 0;
                    APP_STATE.set_fee_size(self.fee_size).unwrap();
                    self.fee_size_input.clear();
                } else if let Ok(mut parsed) = filtered.parse::<u64>() {
                    let max_fee = UNIT as u64 * 10; // max 10 KAS
                    if parsed > max_fee {
                        parsed = max_fee;
                    }
                    self.fee_size = parsed;
                    APP_STATE.set_fee_size(self.fee_size).unwrap();
                    self.fee_size_input = parsed.to_string();
                } else {
                    // parse error => игнор
                }
                Task::none()
            }

            Message::ChannelInputChanged(value) => {
                // только цифры
                let filtered: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
                // не более 7 символов
                let limited = if filtered.len() > 7 { &filtered[0..7] } else { &filtered };
                if limited.is_empty() {
                    self.channel_number = 0;
                    self.channel_number_input.clear();
                } else if let Ok(parsed) = limited.parse::<u32>() {
                    self.channel_number = parsed;
                    self.channel_number_input = parsed.to_string();
                }
                if let Err(err) = APP_STATE.set_channel_number(self.channel_number) {
                    log::error!("Error while changing channel: {}", &err)
                }

                Task::none()
            }
            Message::OpenLink(url) => {
                if let Err(e) = webbrowser::open(&url) {
                    log::error!("Could not open link {}: {}", url, e);
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Column<Message> {
        column![self.build_top_bar(), Rule::horizontal(1), self.build_main_layout(), Rule::horizontal(1), self.build_footer(),]
            .spacing(0)
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fill)
    }

    fn build_top_bar(&self) -> Row<Message> {
        let input_device_options = self.available_input_devices.clone();
        // let output_device_options = vec!["Speaker 1".to_string(), "Speaker 2".to_string()];

        let pick_list_input_device = column![pick_list(input_device_options, Some(&self.input_device), Message::UpdateInputDevice,)
            .placeholder("Select Input Device")
            .width(Length::Fill)]
        .width(Length::FillPortion(2))
        .height(Length::Shrink);

        // let pick_list_output_device = pick_list(
        //     output_device_options,
        //     Some(&self.output_device),
        //     Message::UpdateOutputDevice,
        // )
        // .placeholder("Select Output Device")
        // .padding(9)
        // .width(Length::FillPortion(2));

        let input_node_address = column![text_input("Enter node address (Optional)", &self.node_address)
            .on_input(Message::InputNodeAddress)
            .width(Length::Fill)]
        .width(Length::FillPortion(3))
        .height(Length::Shrink);

        let button_connect =
            column![button("Connect").on_press(Message::ConnectNodeAddress).style(button::success).width(Length::Fill)]
                .width(Length::FillPortion(1))
                .height(Length::Shrink);

        let title = column![row![
            text("KASPEAK")
                .style(text::success)
                .font(font::Font { weight: iced::font::Weight::Semibold, ..font::Font::DEFAULT })
                .size(26),
            text("Alpha").style(text::secondary).size(14)
        ]
        .height(Length::Shrink)]
        .width(Length::FillPortion(6))
        .height(Length::Shrink);

        row![
            title,
            pick_list_input_device,
            // pick_list_output_device,
            input_node_address,
            button_connect,
        ]
        .padding(6)
        .spacing(6)
    }

    fn build_left_side_bar(&self) -> Column<Message> {
        column![self.build_recorder(), Rule::horizontal(1), self.build_user_info(),]
            .spacing(10)
            .padding(0)
            .height(Length::Fill)
            .align_x(Alignment::Start)
    }

    fn build_right_side_bar(&self) -> Column<Message> {
        let clients_container = container(text("Clients")).padding(10).width(Length::Fill).height(Length::FillPortion(1));

        let transactions_scroll =
            scrollable(column![text("Transactions")].padding(10)).width(Length::Fill).height(Length::FillPortion(1));

        column![clients_container, Rule::horizontal(1), transactions_scroll]
            .spacing(10)
            .padding(0)
            .height(Length::Fill)
            .align_x(Alignment::Start)
    }

    fn build_recorder(&self) -> Column<Message> {
        let button_recording = button(if self.is_recording { "Stop Recording" } else { "Start Recording" })
            .on_press(Message::ToggleRecording(!self.is_recording));

        let toggle_listen_self = toggler(self.listen_self).label("Listen to yourself").on_toggle(Message::ToggleListenSelf);

        let toggle_mute_all = toggler(self.mute_all).label("Mute All").on_toggle(Message::ToggleMuteAll);

        column![
            row![button_recording.width(Length::FillPortion(1)).padding(9)].padding(5),
            row![toggle_listen_self.width(Length::FillPortion(1))].padding(5),
            row![toggle_mute_all.width(Length::FillPortion(1))].padding(5),
        ]
    }

    fn build_user_info(&self) -> Column<Message> {
        let balance = APP_STATE.get_balance().unwrap_or_else(|e| {
            log::error!("Error while reading balance: {}", e);
            0
        });
        let balance_in_kas = (balance as f64) / UNIT;

        let utxos = APP_STATE.get_utxos().unwrap_or_else(|e| {
            log::error!("Error while reading UTXO count: {}", e);
            0
        });

        let fee_in_kas = (self.fee_size as f64) / UNIT;
        let channel = self.channel_number.to_string();
        let fee_input = text_input("Fee (sompi)", &self.fee_size_input)
            .on_input(Message::FeeInputChanged)
            .padding(5)
            .size(16)
            .width(Length::FillPortion(1));

        let channel_input = text_input("Channel", &self.channel_number_input)
            .on_input(Message::ChannelInputChanged)
            .padding(5)
            .size(16)
            .width(Length::FillPortion(1));

        let address_button = button(text(&self.display_address).align_x(Alignment::End))
            .on_press(Message::OpenLink(format!("https://explorer-tn11.kaspa.org/addresses/{}", &self.full_address)))
            .style(button::text)
            .width(Length::Shrink)
            .height(Length::Shrink);

        column![
            row![rich_text([span("📍 Address: ").size(16)])].padding(6),
            row![address_button],
            row![rich_text([span("🪪 Name: ").size(16)]), rich_text([span(&self.username).size(16)])].padding(6),
            row![rich_text([span("💵 Balance: ").size(16)]), text(format!("{:.3} TKAS", balance_in_kas)).size(16)].padding(6),
            row![rich_text([span("↕️ UTXO's: ").size(16)]), text(utxos.to_string()).size(16)].padding(6),
            row![rich_text([span("💬 Channel: ").size(16)]), text(format!("{channel}")).size(16)].padding(6),
            row![channel_input].padding(6),
            row![rich_text([span("🧾 Fee: ").size(16)]), text(format!("{:.8} TKAS", fee_in_kas)).size(16),].padding(6),
            row![fee_input].padding(6),
        ]
    }

    fn build_chat_view(&self) -> Column<Message> {
        let mut messages_col = column![];

        for msg in &self.chat_messages {
            let name_text = rich_text([span(msg.user.get_username()).size(16)]).style(text::primary).width(Length::Fill);

            let content_text = rich_text([span(msg.get_content()).size(15)]).style(text::base).width(Length::Fill);

            let time_text =
                rich_text([span(msg.get_time()).size(12)]).width(Length::Fill).style(text::secondary).align_x(Alignment::End);

            let message_block = column![name_text, content_text, time_text].spacing(2).padding(5);

            messages_col = messages_col.push(message_block);
        }

        let scroll_of_messages =
            scrollable(messages_col).id(self.chat_scroll_id.clone()).width(Length::Fill).height(Length::FillPortion(7));

        let chat_editor = text_editor(&self.chat_editor_content)
            .placeholder(format!("Type a message for channel #{}", self.channel_number))
            .on_action(Message::ChatEditorAction)
            .size(16)
            .height(Length::FillPortion(2))
            .key_binding(|key_press| match key_press.key.as_ref() {
                keyboard::Key::Named(key::Named::Enter) if !key_press.modifiers.shift() => {
                    Some(text_editor::Binding::Custom(Message::ChatSendPressed))
                }
                _ => text_editor::Binding::from_key_press(key_press),
            });

        let msg_len = self.chat_editor_content.text().chars().count();
        let max_symbols_field =
            rich_text([span(format!("{}/{}", msg_len, MAX_TEXT_CHARS)).size(14)]).style(text::secondary).width(Length::FillPortion(5));

        let send_button =
            button(text("Send").align_x(Alignment::Center)).on_press(Message::ChatSendPressed).width(Length::FillPortion(1));

        column![row![scroll_of_messages], row![chat_editor], row![max_symbols_field, send_button],]
            .spacing(0)
            .padding(0)
            .height(Length::Fill)
    }

    fn build_main_layout(&self) -> Row<Message> {
        let left_side_bar = self.build_left_side_bar();
        // let right_side_bar = self.build_right_side_bar();
        let chat = self.build_chat_view();

        row![
            left_side_bar.width(Length::FillPortion(1)),
            Rule::vertical(1),
            chat.width(Length::FillPortion(4)),
            Rule::vertical(1),
            // right_side_bar.width(Length::FillPortion(1))
        ]
        .spacing(0)
        .height(Length::Fill)
        .width(Length::Fill)
    }

    fn build_footer(&self) -> Row<Message> {
        // Recording: Active/Inactive
        let recording_prefix = text("Recording: ").size(16);
        let (recording_status_text, recording_color) = if self.is_recording {
            ("Active", Color::from_rgb(0.0, 1.0, 0.0)) // зелёный
        } else {
            ("Inactive", Color::from_rgb(1.0, 0.0, 0.0)) // красный
        };
        let recording_status_label = text(recording_status_text).size(16).color(recording_color);

        // "Status: Connected/Disconnected"
        let connected_prefix = text("Status: ").size(16);
        let listener_connected = APP_STATE.is_listener_connected().unwrap_or_else(|e| {
            log::error!("Error while reading Listener status: {}", e);
            false
        });
        let broadcaster_connected = APP_STATE.is_broadcaster_connected().unwrap_or_else(|e| {
            log::error!("Error while reading Broadcaster status: {}", e);
            false
        });
        let overall_connected = if listener_connected && broadcaster_connected { "Connected" } else { "Disconnected" };
        let (overall_text, overall_color) = if overall_connected == "Connected" {
            (overall_connected, Color::from_rgb(0.0, 1.0, 0.0)) // зелёный
        } else {
            (overall_connected, Color::from_rgb(1.0, 0.0, 0.0)) // красный
        };
        let overall_status_label = text(overall_text).size(16).color(overall_color);

        let status_column = column![
            row![recording_prefix, recording_status_label].height(Length::Shrink), // "Recording: Active/Inactive"
            row![connected_prefix, overall_status_label].height(Length::Shrink),   // "Status: Connected/Disconnected"
        ]
        .height(Length::Shrink)
        .width(Length::FillPortion(1))
        .align_x(Alignment::Start);

        let en_chat_button = button(text("EN TG GROUP").size(12).align_x(iced::alignment::Horizontal::Right))
            .on_press(Message::OpenLink("https://t.me/kaspeak_en".to_string()))
            .style(button::text)
            .width(Length::Fill);

        let ru_chat_button = button(text("RU TG GROUP").size(12).align_x(iced::alignment::Horizontal::Left))
            .on_press(Message::OpenLink("https://t.me/kaspeak_ru".to_string()))
            .style(button::text)
            .width(Length::Fill);

        let links_column = column![row![].padding(8), row![en_chat_button, text(" | ").size(14), ru_chat_button]]
            .width(Length::FillPortion(3))
            .height(Length::Shrink)
            .align_x(Alignment::Center);

        let theme_column = column![pick_list(Theme::ALL, Some(&self.selected_theme), Message::ThemeChanged,).width(Length::Fill)]
            .width(Length::FillPortion(1))
            .height(Length::Shrink)
            .align_x(Alignment::End);

        row![status_column, links_column, theme_column,].padding(6).spacing(6)
    }

    pub fn theme(&self) -> Theme {
        self.selected_theme.clone()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        time::every(Duration::from_millis(100)).map(|_| Message::Tick)
    }
}
