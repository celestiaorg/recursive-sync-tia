//! A simple program that takes a number `n` as input, and writes the `n-1`th and `n`th fibonacci
//! number as an output.

// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use tendermint_light_client_verifier::types::LightBlock;
use common::{ProofType, Groth16VkeyCheckpoint};
use sp1_verifier::Groth16Verifier;
pub fn main() {

    // Read checkpoints
    let should_checkpoint: bool = sp1_zkvm::io::read();
    let checkpoints: Vec<Groth16VkeyCheckpoint> = sp1_zkvm::io::read();

    // Read genesis hash and commit it
    let genesis_hash = sp1_zkvm::io::read_vec();
    sp1_zkvm::io::commit(&genesis_hash);

    // Read h1 with presence flag
    // serde_cbor doesn't work nicely with Option<T> so we use this flag as a workaround
    let h1_present: bool = sp1_zkvm::io::read();
    let h1_bytes = sp1_zkvm::io::read_vec();
    let h1: Option<LightBlock> = if h1_present {
        Some(serde_cbor::from_slice(&h1_bytes).expect("couldn't deserialize h1"))
    } else {
        None
    };

    // Read h2 and commit its hash
    let h2_bytes = sp1_zkvm::io::read_vec();
    let h2: LightBlock = serde_cbor::from_slice(&h2_bytes).expect("couldn't deserialize h2");
    // commit h2 hash
    sp1_zkvm::io::commit(&h2.signed_header.header().hash().as_bytes().to_vec());
    
    let proof_type: ProofType = sp1_zkvm::io::read::<ProofType>();
    
    if h1.is_some() {
        match proof_type {
            ProofType::Stark => {
                sp1_zkvm::lib::verify::verify_sp1_proof(&[0u32; 8], &[0u8; 32]);
            }
            ProofType::Groth16 => {
                //sp1_zkvm::lib::verify::verify_sp1_proof(&[0u32; 8], &[0u8; 32]);
                Groth16Verifier::verify(&[0u8; 32], &[0u8; 32], "", &[0u8; 32]).expect("failed to verify proof");
            }
        }
    }
}
