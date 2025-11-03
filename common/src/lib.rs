// Common library for celestia-recursion workspace

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[repr(u8)]
pub enum ProofType {
    Stark,
    Groth16,
}