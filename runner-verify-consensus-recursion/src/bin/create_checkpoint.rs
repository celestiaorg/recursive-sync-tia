use clap::Parser;
use common::Groth16VkeyCheckpoint;
use sp1_sdk::{ProverClient, SP1ProofWithPublicValues, HashableKey};
use std::fs;
use serde::{Deserialize, Serialize};

/// A buffer of serializable/deserializable objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Buffer {
    pub data: Vec<u8>,
    #[serde(skip)]
    pub ptr: usize,
}

impl Buffer {
    pub fn from(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
            ptr: 0,
        }
    }

    /// Read the serializable object from the buffer.
    pub fn read<T: Serialize + serde::de::DeserializeOwned>(&mut self) -> T {
        let result: T =
            bincode::deserialize(&self.data[self.ptr..]).expect("failed to deserialize");
        let nb_bytes = bincode::serialized_size(&result).expect("failed to get serialized size");
        self.ptr += nb_bytes as usize;
        result
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the old checkpoint JSON file (required)
    #[arg(short = 'o', long, value_name = "OLD_CHECKPOINTS")]
    old_checkpoints: std::path::PathBuf,

    /// Path to the previous proof file
    #[arg(short = 'p', long, value_name = "PREVIOUS_PROOF")]
    previous_proof: std::path::PathBuf,

    /// Path to the groth16 vkey
    #[arg(short = 'v', long, value_name = "VKEY")]
    groth16_vkey: std::path::PathBuf,

    /// Path to the ELF
    #[arg(short = 'e', long, value_name = "ELF")]
    elf: std::path::PathBuf,

    /// Path to the new checkpoint JSON file (required)
    #[arg(short = 'n', long, value_name = "NEW_CHECKPOINTS")]
    new_checkpoints: std::path::PathBuf,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    let args = Args::parse();

    // Read old checkpoints
    let mut old_checkpoints: Vec<Groth16VkeyCheckpoint> = serde_json::from_reader(
        std::fs::File::open(&args.old_checkpoints).expect("Failed to open old checkpoints file")
    ).expect("Failed to parse old checkpoints JSON");

    // Read the previous proof
    if !args.previous_proof.exists() {
        eprintln!("Error: previous_proof file does not exist: {:?}", args.previous_proof);
        std::process::exit(1);
    }
    let previous_proof_content = fs::read_to_string(&args.previous_proof)
        .expect("Failed to read previous proof file");
    let previous_proof: SP1ProofWithPublicValues = serde_json::from_str(&previous_proof_content)
        .expect("Failed to parse previous proof JSON");

    // Read the ELF file
    if !args.elf.exists() {
        eprintln!("Error: ELF file does not exist: {:?}", args.elf);
        std::process::exit(1);
    }
    let elf_bytes = fs::read(&args.elf)
        .expect("Failed to read ELF file");

    // Read the groth16 vkey file
    if !args.groth16_vkey.exists() {
        eprintln!("Error: groth16_vkey file does not exist: {:?}", args.groth16_vkey);
        std::process::exit(1);
    }
    let groth16_vk_bytes = fs::read(&args.groth16_vkey)
        .expect("Failed to read groth16 vkey file");

    // Setup the prover client to compute the vk hash from the ELF
    let client = ProverClient::new();
    let (_pk, vk) = client.setup(&elf_bytes);
    let program_vk_hash = vk.vk.hash_u32();

    // Extract block hash from previous proof's public values using Buffer
    // Public values structure (in order):
    // 1. checkpoints: Vec<Groth16VkeyCheckpoint>
    // 2. genesis_hash: Vec<u8>
    // 3. h2_hash: Vec<u8> (this is the block hash we want!)
    // 4. vk_digest: [u32; 8]
    let public_values = previous_proof.public_values.to_vec();
    let mut public_values_buffer = Buffer::from(&public_values);

    // Read and discard the checkpoints
    let _previous_proof_checkpoints: Vec<Groth16VkeyCheckpoint> = public_values_buffer.read();

    // Read and discard the genesis hash
    let _previous_proof_genesis_hash: Vec<u8> = public_values_buffer.read();

    // Read the h2 hash (block hash)
    let previous_proof_h2_hash: Vec<u8> = public_values_buffer.read();

    if previous_proof_h2_hash.len() != 32 {
        eprintln!("Error: h2 hash is not 32 bytes, got {} bytes", previous_proof_h2_hash.len());
        std::process::exit(1);
    }

    let mut block_hash = [0u8; 32];
    block_hash.copy_from_slice(&previous_proof_h2_hash);

    // Create the new checkpoint
    let new_checkpoint = Groth16VkeyCheckpoint {
        block_hash,
        groth16_vk: Some(groth16_vk_bytes),
        program_vk_hash,
    };

    // Add the new checkpoint to the old checkpoints
    old_checkpoints.push(new_checkpoint);

    // Write the updated checkpoints to the new checkpoint file
    let new_checkpoints_json = serde_json::to_string_pretty(&old_checkpoints)
        .expect("Failed to serialize new checkpoints as JSON");
    fs::write(&args.new_checkpoints, &new_checkpoints_json)
        .expect("Failed to write new checkpoints JSON to output location");

    println!("New checkpoint successfully created and saved to {:?}", args.new_checkpoints);
    println!("Total checkpoints: {}", old_checkpoints.len());
}