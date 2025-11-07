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

pub fn main() {

    // Read checkpoints
    let is_upgrade: bool = sp1_zkvm::io::read();
    let last_checkpoints: Vec<Groth16VkeyCheckpoint> = sp1_zkvm::io::read();

    // Read genesis hash and commit it
    let genesis_hash = sp1_zkvm::io::read_vec();
    sp1_zkvm::io::commit(&genesis_hash);

    // read h1
    let h1_bytes = sp1_zkvm::io::read_vec();
    let h1: LightBlock = serde_cbor::from_slice(&h1_bytes).expect("couldn't deserialize h1");

    // Read h2 and commit its hash
    let h2_bytes = sp1_zkvm::io::read_vec();
    let h2: LightBlock = serde_cbor::from_slice(&h2_bytes).expect("couldn't deserialize h2");
    sp1_zkvm::io::commit(&h2.signed_header.header().hash().as_bytes().to_vec());
    
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

    // if h1 is the genesis block, there won't be a previous proof, so just return.
    if h1.signed_header.header().hash().as_bytes().to_vec() == genesis_hash {
        return
    }

    let proof_type: ProofType = sp1_zkvm::io::read();
    let vk_digest: [u32; 8] = sp1_zkvm::io::read();
    sp1_zkvm::io::commit(&vk_digest);
    let vk_digest_byte_slice: &[u8] = unsafe {
        core::slice::from_raw_parts(vk_digest.as_ptr() as *const u8, vk_digest.len() * core::mem::size_of::<u32>())
    };
    let pv_digest: [u8; 32] = sp1_zkvm::io::read();

    let public_values: Vec<u8> = sp1_zkvm::io::read();
    let mut public_values_buffer = Buffer::from(&public_values);
    let public_values_digest = Sha256::digest(&public_values);

    let previous_proof_genesis_hash: Vec<u8> = public_values_buffer.read();
    let previous_proof_h2_hash: Vec<u8> = public_values_buffer.read();
    let previous_proof_vkey_digest: Vec<u8> = public_values_buffer.read();

    if !is_upgrade && (previous_proof_vkey_digest != vk_digest_byte_slice) {
        panic!("Vkey must match previous proof's vkey, unless it's an upgrade");
    }

    match proof_type {
        ProofType::Stark => {
            sp1_zkvm::lib::verify::verify_sp1_proof(&vk_digest, &pv_digest);
        },
        _ => {
            panic!("Not supported yet");
        }
    }

}
