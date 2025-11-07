//! A simple program that takes a number `n` as input, and writes the `n-1`th and `n`th fibonacci
//! number as an output.

// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use common::{ProofType, Groth16VkeyCheckpoint};
use sp1_verifier::Groth16Verifier;
use std::time::Duration;
use tendermint_light_client_verifier::{
    options::Options, types::LightBlock, ProdVerifier, Verdict, Verifier,
};

mod buffer;
use buffer::Buffer;
use sha2::{Digest, Sha256};

/// Check if we're at an upgrade boundary by checking if h1's block hash matches any checkpoint
fn is_at_upgrade_boundary(h1: &LightBlock, checkpoints: &[Groth16VkeyCheckpoint]) -> bool {
    let binding = h1.signed_header.header().hash();
    let h1_hash = binding.as_bytes();
    checkpoints.iter().any(|cp| cp.block_hash == h1_hash)
}

pub fn main() {

    // Read checkpoints
    let is_upgrade: bool = sp1_zkvm::io::read();
    let last_checkpoints: Vec<Groth16VkeyCheckpoint> = sp1_zkvm::io::read();

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
    // If h1 is none, h2 must be the genesis block
    if h1.is_none() && &h2.signed_header.header().hash().as_bytes().to_vec() != &genesis_hash {
        panic!("h1 is none but h2 hash does not match genesis hash");
    }
    // commit h2 hash
    sp1_zkvm::io::commit(&h2.signed_header.header().hash().as_bytes().to_vec());
    
    if let Some(h1) = h1 {

        let vp = ProdVerifier::default();
        let opt = Options {
            trust_threshold: Default::default(),
            // 2 week trusting period.
            trusting_period: Duration::from_secs(14 * 24 * 60 * 60),
            clock_drift: Default::default(),
        };

        // Get verification time (target block time + some buffer)
        let verify_time = (h2.time() + Duration::from_secs(20))
            .expect("Failed to calculate verify time");

        let verdict = vp.verify_update_header(
            h2.as_untrusted_state(),
            h1.as_trusted_state(),
            &opt,
            verify_time,
        );

        if verdict != Verdict::Success {
            panic!("Verification failed");
        }
    }

}
