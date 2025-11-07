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
    // commit h2 hash
    sp1_zkvm::io::commit(&h2.signed_header.header().hash().as_bytes().to_vec());
    
    // is the previous iteration a Groth16, Stark, or neither
    let proof_type: ProofType = sp1_zkvm::io::read::<ProofType>();
    
    let groth16_vk: [u8; 32] = sp1_zkvm::io::read();
    let program_vk: [u32; 8] = sp1_zkvm::io::read();
    let program_vk_byte_slice: &[u8] = unsafe {
        core::slice::from_raw_parts(program_vk.as_ptr() as *const u8, program_vk.len() * core::mem::size_of::<u32>())
    };

    let public_values: Vec<u8> = sp1_zkvm::io::read_vec();
    let mut public_values_buffer = Buffer::from(&public_values);
    let public_values_hash: [u8; 32] = Sha256::digest(&public_values).into();

    if let Some(h1) = h1 {

        let previous_genesis: Vec<u8> = public_values_buffer.read();
        let previous_h2_hash: Vec<u8> = public_values_buffer.read();

        if previous_genesis != genesis_hash {
            panic!("Previous genesis hash does not match current genesis hash");

        }
        if previous_h2_hash != h1.signed_header.header().hash().as_bytes().to_vec() {
            panic!("Previous h2 hash does not match current h2 hash");
        }

        // Determine if we're at an upgrade boundary
        let at_upgrade_boundary = is_at_upgrade_boundary(&h1, &last_checkpoints);

        // Verify is_upgrade input matches actual upgrade boundary
        if is_upgrade && !at_upgrade_boundary {
            panic!("is_upgrade is true but h1 is not at an upgrade boundary according to last_checkpoints");
        }
        if !is_upgrade && at_upgrade_boundary {
            panic!("is_upgrade is false but h1 is at an upgrade boundary according to last_checkpoints");
        }

        match proof_type {
            ProofType::Stark => {
                sp1_zkvm::lib::verify::verify_sp1_proof(&program_vk, &public_values_hash);

                // Verify that the verification key matches unless we're at an upgrade boundary
                if !is_upgrade {
                    let previous_program_vk_hash: [u8; 32] = public_values_buffer.read();
                    let current_program_vk_hash: [u8; 32] = Sha256::digest(program_vk_byte_slice).into();
                    if previous_program_vk_hash != current_program_vk_hash {
                        panic!("Program verification key changed but is_upgrade is false");
                    }
                }
            }
            ProofType::Groth16 => {
                // Read Groth16 proof from IO
                let groth16_proof: Vec<u8> = sp1_zkvm::io::read_vec();

                // Convert program_vk hash to hex string for sp1_vkey_hash parameter
                let program_vk_hash = Sha256::digest(program_vk_byte_slice);
                let sp1_vkey_hash_hex = hex::encode(program_vk_hash);

                // Verify the Groth16 proof
                Groth16Verifier::verify(&groth16_proof, &public_values_hash, &sp1_vkey_hash_hex, &groth16_vk)
                    .expect("failed to verify groth16 proof");

                // Verify that the verification key matches unless we're at an upgrade boundary
                if !is_upgrade {
                    let previous_groth16_vk: [u8; 32] = public_values_buffer.read();
                    if previous_groth16_vk != groth16_vk {
                        panic!("Groth16 verification key changed but is_upgrade is false");
                    }
                }
            }
        }

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
