// Common library for celestia-recursion workspace

pub enum Proof {
    None,
    Groth16(Vec<u8>),
    Stark(Vec<u8>),
}

pub struct ProgramInput {
    pub genesis_hash: [u8; 32],
    pub previous_hash: [u8; 32],
    pub latest_hash: [u8; 32],
    pub previous_proof: Proof,
    pub intermediate_hashes_merkle_root: [u8; 32],
    pub intermediate_hashes: Vec<[u8; 32]>,
}

// ProgramOutput is a subset of ProgramInput
pub struct ProgramOutput {
    pub genesis_hash: [u8; 32],
    pub latest_hash: [u8; 32],
    pub intermediate_hashes_merkle_root: [u8; 32],
}