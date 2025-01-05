use std::sync::{Arc, Mutex};

use kaspa_wallet_core::prelude::*;
use kaspa_wallet_core::{rpc::DynRpcApi, rpc::Rpc, wallet::Wallet};
use kaspa_wrpc_client::KaspaRpcClient;
use workflow_core::prelude::Abortable;

use crate::app_state::APP_STATE;
use crate::constants::{NETWORK_ID, UNIT};

use kaspa_wallet_core::account::Account;
use kaspa_wallet_core::error::Error;
use kaspa_wallet_core::prelude::{Language, Mnemonic, Secret};
use kaspa_wallet_core::result::Result as KaspaResult;

/// Пример «сервиса» для кошелька.
/// Хранит ссылку на `kaspa_wallet_core::Wallet`, `account`,
/// логику `setup_wallet_and_account`, `handle_airdrop`, `send_transaction`.
pub struct WalletService {
    wallet: Arc<Wallet>,
    wallet_secret: Secret,
    personal_account: Mutex<Option<Arc<dyn Account>>>,
    airdrop_account: Mutex<Option<Arc<dyn Account>>>,
}

impl WalletService {
    /// Создаём `WalletService`.
    pub async fn new(client: Arc<KaspaRpcClient>, wallet_mnemonic: String) -> kaspa_wallet_core::result::Result<Self> {
        let rpc_api: Arc<DynRpcApi> = client.clone();
        let rpc = Rpc::new(rpc_api, client.ctl().clone());

        let storage = Wallet::local_store().expect("Failed to initialize local wallet storage");
        let wallet = Arc::new(Wallet::try_with_rpc(Some(rpc), storage, Some(NETWORK_ID)).expect("Failed to create a Wallet with RPC"));
        let wallet_service = Self {
            wallet,
            personal_account: Mutex::new(None),
            airdrop_account: Mutex::new(None),
            wallet_secret: Secret::new(wallet_mnemonic.clone().into_bytes()),
        };
        wallet_service.setup_wallet_and_account(&wallet_mnemonic).await?;

        Ok(wallet_service)
    }

    /// Создаём кошелёк/аккаунт, запускаем кошелёк, записываем всё в self.account
    async fn setup_wallet_and_account(&self, wallet_mnemonic: &str) -> KaspaResult<()> {
        let wallet_secret = self.wallet_secret.clone();
        let wallet = self.wallet.clone();
        self.wallet.load_settings().await?;

        // 1) Переопределяем кошелек
        let _ = wallet
            .create_wallet(
                &wallet_secret,
                WalletCreateArgs::new(Some("Application Wallet".to_string()), None, EncryptionKind::XChaCha20Poly1305, None, true),
            )
            .await?;

        wallet.start().await?;
        log::info!("New wallet is connected");

        let personal_account = {
            let mnemonic = Mnemonic::new(wallet_mnemonic, Language::English).expect("Failed to create Mnemonic");
            log::info!("Created mnemonic: {}", mnemonic.phrase_string());
            init_account_from_mnemonic("Personal Account", mnemonic, wallet.clone(), &wallet_secret).await
        }?;

        // Запоминаем локально
        self.personal_account.lock()?.replace(personal_account.clone());

        // Устанавливаем адрес в стейт приложения
        if let Ok(address) = personal_account.receive_address() {
            let _ = APP_STATE.set_account_address(Some(address));
        }

        Ok(())
    }

    /// Отправка транзакции
    async fn send_transaction(
        &self,
        from: Option<Arc<dyn Account>>,
        destination: Address,
        amount: Option<u64>,
        payload: Option<Vec<u8>>,
    ) -> KaspaResult<Balance> {
        let account = match from {
            None => match self.personal_account.lock()?.clone() {
                Some(account) => account,
                None => return Err(Error::from("Personal account is not initialized")),
            },
            Some(account) => account,
        };

        let payload_size = payload.as_ref().map_or(0, |p| p.len());
        let default_amount = (5.0 * UNIT) as u64;
        let final_amount = amount.unwrap_or(default_amount);

        let tx_result = account
            .clone()
            .send(
                PaymentDestination::PaymentOutputs(PaymentOutputs::from((destination, final_amount))),
                Fees::SenderPays(APP_STATE.get_fee_size()?),
                payload,
                self.wallet_secret.clone(),
                None,
                &Abortable::new(),
                None,
            )
            .await;

        match tx_result {
            Ok((summary, _tx_ids)) => {
                // Коэффициент для перевода из атомарных единиц в KAS
                let utxos = summary.aggregated_utxos();
                let fees_atomic = summary.aggregated_fees();
                let tx_count = summary.number_of_generated_transactions();

                // final_transaction_amount: Option<u64>
                let final_amount_atomic = summary.final_transaction_amount().unwrap_or(0);
                // final_transaction_id: Option<TransactionId>
                let final_txid = summary.final_transaction_id().unwrap_or_default();

                let fees_kas = fees_atomic as f64 / UNIT;
                let amount_kas = final_amount_atomic as f64 / UNIT;

                log::info!(
                    "Transaction successfully sent: utxos={}, fee={:.8} TKAS, tx_count={}, amount={:.8} TKAS, final_txid={}, payload_size={}",
                    utxos,
                    fees_kas,
                    tx_count,
                    amount_kas,
                    final_txid,
                    payload_size,
                );
            }
            Err(err) => {
                log::error!("Error while sending transaction: {:?}", err);
            }
        }

        let current_balance = account.balance().unwrap_or_default();
        Ok(current_balance)
    }

    pub async fn send_transaction_to_self(&self, amount: Option<u64>, payload: Option<Vec<u8>>) -> KaspaResult<Balance> {
        let account = match self.personal_account.lock()?.clone() {
            Some(account) => account,
            None => return Err(Error::from("Personal account is not initialized")),
        };
        self.send_transaction(None, account.receive_address()?, amount, payload).await?;
        Ok(self.update_app_state_balance(account.clone()).await)
    }

    /// Обновить баланс в AppState
    pub async fn update_app_state_balance(&self, account: Arc<dyn Account>) -> Balance {
        let balance_info = account.balance().unwrap_or_default();
        let balance = balance_info.mature;
        let utxos = balance_info.mature_utxo_count;
        let _ = APP_STATE.set_balance(balance);
        let _ = APP_STATE.set_utxos(utxos);

        let balance_tkas = (balance as f64) / UNIT;
        log::info!("Current balance: {:.8} TKAS | {} UTXOs", balance_tkas, utxos);

        balance_info
    }

    /// Airdrop
    pub async fn handle_airdrop(&self) -> KaspaResult<()> {
        log::info!("Starting airdrop");

        //Init airdrop account or use initialized if already have been done
        let airdrop_account_initialized = { self.airdrop_account.lock()?.is_some() };
        let airdrop_account = match airdrop_account_initialized {
            true => self.airdrop_account.lock()?.clone().unwrap(),
            false => {
                let wallet_secret = self.wallet_secret.clone();
                let wallet = self.wallet.clone();
                let airdrop_mnemonic_entropy = hex::decode("8ee7277ab57bb6cae8c5bec2cf530459069c5a2e4ff7dc00c523a2ef0e42f97a")
                    .expect("Failed to decode the airdrop wallet");
                let airdrop_mnemonic = Mnemonic::from_entropy(airdrop_mnemonic_entropy, Language::English)
                    .expect("Failed to create the airdrop mnemonic");
                let airdrop_account = init_account_from_mnemonic("Airdrop Account", airdrop_mnemonic, wallet, &wallet_secret).await?;
                airdrop_account.clone().start().await?;
                self.airdrop_account.lock()?.replace(airdrop_account.clone());
                airdrop_account
            }
        };

        let target_account = match self.personal_account.lock()?.clone() {
            Some(account) => account,
            None => return Err(Error::from("Personal account is not initialized")),
        };

        let destination = target_account.receive_address()?;

        // Шлём 20 транзакций по 10 TKAS
        for _ in 0..20 {
            let ten_kas_atomic = (10.0f64 * UNIT) as u64;
            self.send_transaction(Some(airdrop_account.clone()), destination.clone(), Some(ten_kas_atomic), None).await?;
            self.update_app_state_balance(target_account.clone()).await;
        }

        self.update_app_state_balance(target_account).await;
        log::info!("Airdrop finished");
        Ok(())
    }

    pub async fn handle_connect_to_node(&self) -> KaspaResult<Balance> {
        let personal_account = match self.personal_account.lock()?.clone() {
            Some(account) => account,
            None => return Err(Error::from("Personal account is not initialized")),
        };

        // Запускаем аккаунт
        personal_account.clone().start().await?;

        // Обновляем AppState (баланс, utxos)
        Ok(self.update_app_state_balance(personal_account.clone()).await)
    }
}

async fn init_account_from_mnemonic(
    account_name: &str,
    account_mnemonic: Mnemonic,
    wallet: Arc<Wallet>,
    wallet_secret: &Secret,
) -> kaspa_wallet_core::result::Result<Arc<dyn Account>> {
    let prv_key_data_id = wallet
        .as_api()
        .prv_key_data_create(
            wallet_secret.clone(),
            PrvKeyDataCreateArgs::new(Some(account_name.to_string()), None, Secret::from(account_mnemonic.phrase_string())),
        )
        .await?;
    let wallet_guard = wallet.guard();
    let account_name = Some(account_name.to_string());
    let guard = wallet_guard.lock().await;
    wallet.create_account(&wallet_secret, AccountCreateArgs::Legacy { prv_key_data_id, account_name }, false, &guard).await
}
