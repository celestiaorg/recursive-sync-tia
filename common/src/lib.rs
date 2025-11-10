// Common library for celestia-recursion workspace

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[repr(u8)]
pub enum ProofType {
    Stark,
    Groth16,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct Groth16VkeyCheckpoint {
    pub block_height: [u8; 32],
    pub block_hash: [u8; 32],
    pub groth16_vk: Option<Vec<u8>>,
    pub program_vk_hash: [u32; 8],
}