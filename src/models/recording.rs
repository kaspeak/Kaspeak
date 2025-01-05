use crate::models::payload::StatusFlag;

#[derive(Debug)]
pub(crate) struct Recording {
    pub audio: Vec<u8>,
    pub state: StatusFlag,
    pub fragment_num: u32,
}
