use futures::stream::FuturesUnordered;
use futures::{select_biased, FutureExt, StreamExt};
use kaspa_wallet_core::rpc::RpcApi;
use kaspa_wrpc_client::prelude::RpcState;
use std::sync::Arc;
use tokio::sync::Semaphore;
use workflow_core::{
    channel::{Channel, DuplexChannel, SendError},
    task::spawn,
};

use crate::app_state::APP_STATE;
use crate::constants::{MINIMUM_AIRDROP_BALANCE_TKAS, UNIT};
use crate::core::wallet::WalletService;
use crate::models::instruction::Instruction;
use crate::models::instruction::Instruction::{Airdrop, SendTx};
use kaspa_wrpc_client::{result::Result as KaspaResult, KaspaRpcClient};

pub struct BroadcasterInner {
    task_ctl: DuplexChannel<()>,
    client: Arc<KaspaRpcClient>,
    instruction_channel: Channel<Instruction>,
    wallet_service: Arc<WalletService>,
}

#[derive(Clone)]
pub struct Broadcaster {
    pub inner: Arc<BroadcasterInner>,
}

impl Broadcaster {
    pub fn try_new(client: Arc<KaspaRpcClient>, wallet_service: Arc<WalletService>) -> KaspaResult<Self> {
        let inner =
            BroadcasterInner { task_ctl: DuplexChannel::oneshot(), client, instruction_channel: Channel::unbounded(), wallet_service };
        Ok(Self { inner: Arc::new(inner) })
    }

    pub async fn start(&self) -> KaspaResult<()> {
        self.spawn_event_loop().await?;
        Ok(())
    }

    pub async fn stop(&self) -> KaspaResult<()> {
        self.client().disconnect().await?;
        self.stop_event_loop().await?;
        Ok(())
    }

    pub async fn send_instruction(&self, instruction: Instruction) -> Result<(), SendError<Instruction>> {
        self.inner.instruction_channel.send(instruction).await.expect("Failed to send the instruction");
        Ok(())
    }

    pub fn client(&self) -> &Arc<KaspaRpcClient> {
        &self.inner.client
    }

    async fn spawn_event_loop(&self) -> KaspaResult<()> {
        let broadcaster = self.clone();
        let rpc_ctl_channel = self.client().rpc_ctl().multiplexer().channel();
        let task_ctl_receiver = self.inner.task_ctl.request.receiver.clone();
        let task_ctl_sender = self.inner.task_ctl.response.sender.clone();
        let instruction_receiver = self.inner.instruction_channel.receiver.clone();

        let semaphore = Arc::new(Semaphore::new(10));
        let mut futures = FuturesUnordered::new();

        spawn(async move {
            let mut deferred_instructions = Vec::new();

            loop {
                select_biased! {
                    msg = rpc_ctl_channel.receiver.recv().fuse() => {
                        if let Ok(msg) = msg {
                            match msg {
                                RpcState::Connected => {
                                    if let Err(err) = broadcaster.handle_connect().await {
                                        log::error!("Error while connecting: {err}");
                                    } else {
                                        while let Some(instr) = deferred_instructions.pop() {
                                            let permit = semaphore.clone().acquire_owned().await.unwrap();
                                            futures.push(spawn_task(broadcaster.clone(), instr, permit));
                                        }
                                    }
                                },
                                RpcState::Disconnected => {
                                    if let Err(err) = broadcaster.handle_disconnect().await {
                                        log::error!("Error while disconnecting: {err}");
                                    }
                                },
                            }
                        } else {
                            log::error!("RPC CTL channel error");
                            break;
                        }
                    },
                    instruction = instruction_receiver.recv().fuse() => {
                        if let Ok(instr) = instruction {
                            let is_connected = Self::is_connected();
                            if !is_connected {
                                deferred_instructions.push(instr);
                            } else {
                                let permit = semaphore.clone().acquire_owned().await.unwrap();
                                futures.push(spawn_task(broadcaster.clone(), instr, permit));
                            }
                        } else {
                            log::error!("Instruction channel error");
                            break;
                        }
                    },
                    _ = task_ctl_receiver.recv().fuse() => {
                        break;
                    },
                    _ = futures.next() => { /* игнорируем завершённые */ },
                }
            }

            log::info!("Event loop task has finished.");

            if Self::is_connected() {
                broadcaster.handle_disconnect().await.unwrap_or_else(|err| log::error!("Error while disconnecting: {err}"));
            }

            task_ctl_sender.send(()).await.unwrap();
        });
        Ok(())
    }

    async fn stop_event_loop(&self) -> KaspaResult<()> {
        self.inner.task_ctl.signal(()).await.expect("Failed to stop the event loop");
        Ok(())
    }

    fn is_connected() -> bool {
        APP_STATE.is_broadcaster_connected().unwrap_or_else(|err| {
            log::error!("Error while retrieving is_connected flag: {}", err);
            false
        })
    }

    async fn handle_connect(&self) -> KaspaResult<()> {
        log::info!("Connected to {:?}", self.client().url());
        let server_info = self.client().get_server_info().await?;
        log::info!("Server info: {:?}", server_info);

        match self.inner.wallet_service.handle_connect_to_node().await {
            Ok(curr_balance) => {
                let balance_tkas = (curr_balance.mature as f64) / UNIT;
                if balance_tkas < MINIMUM_AIRDROP_BALANCE_TKAS {
                    if let Err(err) = self.send_instruction(Airdrop).await {
                        log::error!("Error while sending airdrop instruction: {}", err);
                    }
                }
            }
            Err(err) => {
                return Err(kaspa_wrpc_client::error::Error::from(err.to_string()));
            }
        }

        APP_STATE.set_broadcaster_connected(true).map_err(|e| log::error!("Error set_broadcaster_connected: {}", e)).ok();
        APP_STATE.chat_state.clear();

        Ok(())
    }

    async fn handle_disconnect(&self) -> KaspaResult<()> {
        log::info!("Disconnected from {:?}", self.client().url());
        APP_STATE.set_broadcaster_connected(false).map_err(|e| log::error!("Error set_broadcaster_connected: {}", e)).ok();
        Ok(())
    }

    async fn handle_instruction(&self, instruction: Instruction) -> KaspaResult<()> {
        let wallet_service = &self.inner.wallet_service;

        match instruction {
            SendTx(send_tx) => match wallet_service.send_transaction_to_self(None, send_tx.tx_payload).await {
                Ok(current_balance) => {
                    let balance_tkas = (current_balance.mature as f64) / UNIT;
                    if balance_tkas < MINIMUM_AIRDROP_BALANCE_TKAS {
                        if let Err(err) = self.send_instruction(Airdrop).await {
                            log::error!("Error sending airdrop instruction: {}", err);
                        }
                    }
                }
                Err(err) => {
                    log::error!("Error while sending transaction: {}", err);
                }
            },
            Airdrop => {
                if let Err(err) = wallet_service.handle_airdrop().await {
                    log::error!("Error while performing Airdrop instruction: {}", err);
                }
            }
        }
        Ok(())
    }
}

async fn spawn_task(broadcaster: Broadcaster, instruction: Instruction, _permit: tokio::sync::OwnedSemaphorePermit) {
    if let Err(err) = broadcaster.handle_instruction(instruction).await {
        log::error!("Error while processing instruction: {err}");
    }
}
