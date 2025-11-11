//! A simple program that takes a number `n` as input, and writes the `n-1`th and `n`th fibonacci
//! number as an output.

// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use common::Groth16VkeyCheckpoint;
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
    println!("cycle-tracker-start: deserialize is_upgrade and checkpoints");
    let is_upgrade: bool = sp1_zkvm::io::read();
    let checkpoints: Vec<Groth16VkeyCheckpoint> = sp1_zkvm::io::read();
    sp1_zkvm::io::commit(&checkpoints);
    println!("cycle-tracker-end: deserialize is_upgrade and checkpoints");

    // Read genesis hash and commit it
    println!("cycle-tracker-start: read genesis hash");
    let genesis_hash = sp1_zkvm::io::read_vec();
    sp1_zkvm::io::commit(&genesis_hash);
    println!("cycle-tracker-end: read genesis hash");

    // read h1
    println!("cycle-tracker-start: read h1");
    let h1_bytes = sp1_zkvm::io::read_vec();
    let h1: LightBlock = serde_cbor::from_slice(&h1_bytes).expect("couldn't deserialize h1");
    println!("cycle-tracker-end: read h1");

    // Read h2 and commit its hash
    println!("cycle-tracker-start: read and commit h2");
    let h2_bytes = sp1_zkvm::io::read_vec();
    let h2: LightBlock = serde_cbor::from_slice(&h2_bytes).expect("couldn't deserialize h2");
    sp1_zkvm::io::commit(&h2.signed_header.header().hash().as_bytes().to_vec());
    println!("cycle-tracker-end: read and commit h2");

    println!("cycle-tracker-start: setup verifier and verify consensus");
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
    println!("cycle-tracker-end: setup verifier and verify consensus");

    println!("cycle-tracker-start: check if h1 is the genesis block");
    // if h1 is the genesis block, there won't be a previous proof, so just return.
    if h1.signed_header.header().hash().as_bytes().to_vec() == genesis_hash {
        println!("cycle-tracker-end: check if h1 is the genesis block");
        return
    }
    println!("cycle-tracker-end: check if h1 is the genesis block");

    println!("cycle-tracker-start: read vk digest");
    let vk_digest: [u32; 8] = sp1_zkvm::io::read();
    sp1_zkvm::io::commit(&vk_digest);
    let vk_digest_byte_slice: &[u8] = unsafe {
        core::slice::from_raw_parts(vk_digest.as_ptr() as *const u8, vk_digest.len() * core::mem::size_of::<u32>())
    };
    println!("cycle-tracker-end: read vk digest");

    println!("cycle-tracker-start: read previous groth16 proof");
    let previous_groth16_proof: Vec<u8> = sp1_zkvm::io::read();
    println!("cycle-tracker-end: read previous groth16 proof");

    println!("cycle-tracker-start: read pv digest");
    let pv_digest: [u8; 32] = sp1_zkvm::io::read();
    println!("cycle-tracker-end: read pv digest");

    println!("cycle-tracker-start: read and process public values");
    let public_values: Vec<u8> = sp1_zkvm::io::read();
    let mut public_values_buffer = Buffer::from(&public_values);
    let public_values_digest = Sha256::digest(&public_values);
    println!("cycle-tracker-end: read and process public values");

    println!("{:?}", public_values_buffer);

    println!("cycle-tracker-start: read previous proof checkpoints");
    let previous_proof_checkpoints: Vec<Groth16VkeyCheckpoint> = public_values_buffer.read();
    println!("cycle-tracker-end: read previous proof checkpoints");
    
    println!("cycle-tracker-start: read previous proof genesis hash");
    let previous_proof_genesis_hash: Vec<u8> = public_values_buffer.read();
    println!("cycle-tracker-end: read previous proof genesis hash");
    
    println!("cycle-tracker-start: read previous proof h2 hash");
    let previous_proof_h2_hash: Vec<u8> = public_values_buffer.read();
    println!("cycle-tracker-end: read previous proof h2 hash");
    
    println!("cycle-tracker-start: read previous proof vkey digest");
    let previous_proof_vkey_digest: [u32; 8] = public_values_buffer.read();
    println!("cycle-tracker-end: read previous proof vkey digest");

    println!("cycle-tracker-start: check if previous proof's genesis hash matches");
    if previous_proof_genesis_hash != genesis_hash {
        panic!("Genesis hash must match previous proof's genesis hash");
    }
    println!("cycle-tracker-end: check if previous proof's genesis hash matches");

    if !is_upgrade {
        println!("cycle-tracker-start: verify previous proof for non-upgrade");
        if previous_proof_vkey_digest != vk_digest {
            panic!("Vkey must match previous proof's vkey, except for upgrades");
        }

        if previous_proof_checkpoints != checkpoints[..previous_proof_checkpoints.len()] {
            panic!("Checkpoints must match previous proof's checkpoints, except for upgrades");
        }

        sp1_zkvm::lib::verify::verify_sp1_proof(&vk_digest, &pv_digest);
        println!("cycle-tracker-end: verify previous proof for non-upgrade");
    } else {
        println!("cycle-tracker-start: verify previous proof for upgrade");
        if previous_proof_checkpoints.len()+1 != checkpoints.len() {
            panic!("During upgrade, the number of checkpoints must increase by 1");
        }

        let incoming_checkpoint = &checkpoints[checkpoints.len() - 1];

        if incoming_checkpoint.program_vk_hash != previous_proof_vkey_digest {
            panic!("Program vkey hash must match previous proof's program vkey hash");
        }
        match &incoming_checkpoint.groth16_vk {
                Some(vk) => {
                    println!("cycle-tracker-start: verify previous groth16 proof for upgrade");
                    // Convert program_vk_hash [u32; 8] to hex string
                    let vk_hash_bytes: Vec<u8> = incoming_checkpoint
                        .program_vk_hash
                        .iter()
                        .flat_map(|&word| word.to_le_bytes())
                        .collect();
                    let vk_hash_hex = hex::encode(&vk_hash_bytes);

                    Groth16Verifier::verify(
                        &previous_groth16_proof,
                        &public_values_digest,
                        &vk_hash_hex,
                        &vk
                    ).expect("Failed to verify previous groth16 proof");
                    println!("cycle-tracker-end: verify previous groth16 proof for upgrade");
                },
                None => {
                    println!("cycle-tracker-start: verify previous sp1 proof for upgrade");
                    sp1_zkvm::lib::verify::verify_sp1_proof(&vk_digest, &pv_digest);
                    println!("cycle-tracker-end: verify previous sp1 proof for upgrade");
                }
            }
        println!("cycle-tracker-end: verify previous proof for upgrade");
    }

}
