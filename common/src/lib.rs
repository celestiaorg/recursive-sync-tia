// Common library for celestia-recursion workspace

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[repr(u8)]
pub enum ProofType {
    Stark,
    Groth16,
}

#[derive(Serialize, Deserialize)]
pub struct Groth16VkeyCheckpoint {
    pub version: u32,
    pub block_hash: [u8; 32],
    pub groth16_vk: [u8; 32],
}