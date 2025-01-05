use crate::models::payload::Payload;
use crate::models::recording::Recording;

#[derive(Debug)]
pub enum Instruction {
    SendTx(SendTxInstruction),
    Airdrop,
}

#[derive(Debug)]
pub struct SendTxInstruction {
    pub tx_payload: Option<Vec<u8>>,
}

//TODO изменить тип Result
impl Instruction {
    /// Формирование инструкции для Broadcaster из Recording.
    pub(crate) fn try_from_recording(recording: &Recording) -> kaspa_wrpc_client::result::Result<Instruction> {
        let mut payload = Payload::from_recording(&recording)?;
        payload.compress_zstd()?;

        Ok(Instruction::SendTx(SendTxInstruction { tx_payload: Some(payload.to_bytes()) }))
    }

    /// Формирование инструкции для Broadcaster из Message.
    pub(crate) fn try_from_message(message: String) -> kaspa_wrpc_client::result::Result<Instruction> {
        let mut payload = Payload::from_chat_message(&message)?;
        payload.compress_zstd()?;

        Ok(Instruction::SendTx(SendTxInstruction { tx_payload: Some(payload.to_bytes()) }))
    }
}
