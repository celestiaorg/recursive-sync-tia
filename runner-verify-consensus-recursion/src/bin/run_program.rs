use clap::Parser;
use sp1_verifier;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin, SP1ProofWithPublicValues, Prover, HashableKey, SP1Proof,
    network::{FulfillmentStrategy, NetworkMode},
};
use std::fs;
use std::path::PathBuf;
use tendermint_light_client_verifier::types::LightBlock;
use common::Groth16VkeyCheckpoint;

pub const CONSENSUS_VERIFIER_RECURSION_ELF: &[u8] =
    include_elf!("program-verify-consensus-recursion");

/// Run program with header JSON files
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to genesis header JSON file (required)
    #[arg(short = 'g', long, value_name = "PATH")]
    genesis: PathBuf,

    #[arg(short = 'u', long, value_name = "PATH")]
    upgrade_history: Option<PathBuf>,

    /// Path to first header JSON file (required)
    #[arg(long, value_name = "PATH")]
    h1: PathBuf,

    /// Path to second header JSON file (required)
    #[arg(long, value_name = "PATH")]
    h2: PathBuf,

    /// Private key for the prover client
    #[arg(short = 'k', long, value_name = "PRIVATE_KEY")]
    private_key: String,

    /// previous proof file
    #[arg(short = 'p', long, value_name = "PATH")]
    previous_proof: Option<PathBuf>,

    /// Path to output proof file
    #[arg(short = 'o', long, value_name = "PATH")]
    output_proof: PathBuf,

    /// dry run mode
    #[arg(short = 'd', long, default_value_t = false)]
    dry_run: bool,

    /// Use groth16: useful for upgrades, especially upgrading SP1 versions, or different zkVMs.
    #[arg(short = 'r', long, default_value_t = false)]
    groth16: bool,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    let groth16_vk_bytes = sp1_verifier::GROTH16_VK_BYTES.clone();
    println!("Groth16 VK bytes length: {:?}", groth16_vk_bytes.len());

    // Parse the command line arguments.
    let args = Args::parse();

    // Read and deserialize genesis (required)
    if !args.genesis.exists() {
        eprintln!("Error: genesis file does not exist: {:?}", args.genesis);
        std::process::exit(1);
    }
    let content = fs::read_to_string(&args.genesis)
        .unwrap_or_else(|e| {
            eprintln!("Error reading genesis file: {}", e);
            std::process::exit(1);
        });
    let genesis: LightBlock = serde_json::from_str(&content)
        .unwrap_or_else(|e| {
            eprintln!("Error deserializing genesis JSON: {}", e);
            std::process::exit(1);
        });

    // Read and deserialize h1 (required)
    if !args.h1.exists() {
        eprintln!("Error: h1 file does not exist: {:?}", args.h1);
        std::process::exit(1);
    }
    let content = fs::read_to_string(&args.h1)
        .unwrap_or_else(|e| {
            eprintln!("Error reading h1 file: {}", e);
            std::process::exit(1);
        });
    let h1: LightBlock = serde_json::from_str(&content)
        .unwrap_or_else(|e| {
            eprintln!("Error deserializing h1 JSON: {}", e);
            std::process::exit(1);
        });

    // Read and deserialize h2 (required)
    if !args.h2.exists() {
        eprintln!("Error: h2 file does not exist: {:?}", args.h2);
        std::process::exit(1);
    }
    let content = fs::read_to_string(&args.h2)
        .unwrap_or_else(|e| {
            eprintln!("Error reading h2 file: {}", e);
            std::process::exit(1);
        });
    let h2: LightBlock = serde_json::from_str(&content)
        .unwrap_or_else(|e| {
            eprintln!("Error deserializing h2 JSON: {}", e);
            std::process::exit(1);
        });

    let upgrade_history: Vec<Groth16VkeyCheckpoint> = match args.upgrade_history {
        Some(path) => {
            if !path.exists() {
                eprintln!("Error: upgrade history file does not exist: {:?}", path);
                std::process::exit(1);
            }
            let content = fs::read_to_string(&path).unwrap_or_else(|e| {
                eprintln!("Error reading upgrade history file: {}", e);
                std::process::exit(1);
            });
            serde_json::from_str(&content).unwrap_or_else(|e| {
                eprintln!("Error deserializing upgrade history JSON: {}", e);
                std::process::exit(1);
            })
        },
        None => Vec::new(),
    };

    // Check if h1 is the same as genesis
    let genesis_hash = genesis.signed_header.header().hash();
    let h1_hash = h1.signed_header.header().hash();
    let h1_is_genesis = genesis_hash == h1_hash;

    let mut previous_proof: Option<SP1ProofWithPublicValues> = None;
    if !h1_is_genesis {
        if args.previous_proof.is_none() {
            eprintln!("Error: previous_proof is required when h1 is not the same as genesis");
            std::process::exit(1);
        }
        let previous_proof_path = args.previous_proof.as_ref().unwrap();
        if !previous_proof_path.exists() {
            eprintln!("Error: previous_proof file does not exist: {:?}", previous_proof_path);
            std::process::exit(1);
        }
        let previous_proof_content = fs::read_to_string(previous_proof_path).unwrap();
        previous_proof = Some(serde_json::from_str(&previous_proof_content).unwrap());
    }

    // Setup the prover client.
    let client = ProverClient::builder()
        .network_for(NetworkMode::Mainnet)
        .private_key(&args.private_key)
        .build();

    let (pk, vk) = client.setup(CONSENSUS_VERIFIER_RECURSION_ELF);

    let mut stdin = SP1Stdin::new();

    // Write is_upgrade flag
    stdin.write(&false);

    // Write checkpoints
    stdin.write(&upgrade_history);

    // Write genesis hash
    stdin.write_vec(genesis.signed_header.header().hash().as_bytes().to_vec());

    // Write h1
    let h1_bytes = serde_cbor::to_vec(&h1).unwrap();
    stdin.write_vec(h1_bytes);

    // Write h2
    let h2_bytes = serde_cbor::to_vec(&h2).unwrap();
    stdin.write_vec(h2_bytes);

    // Write vk digest
    stdin.write(&vk.vk.hash_u32());

    if let Some(previous_proof) = previous_proof {
        // Check proof type and write groth16 proof if applicable
        match &previous_proof.proof {
            SP1Proof::Compressed(compressed_stark_proof) => {
                stdin.write(&Vec::<u8>::new());
                stdin.write_proof(compressed_stark_proof.as_ref().clone(), vk.vk.clone());
            },
            SP1Proof::Groth16(groth16_proof) => {
                stdin.write(&groth16_proof.raw_proof.as_bytes().to_vec());
            },
            _ => {
                panic!("Unsupported proof type");
            }
        }

        // write pv digest
        stdin.write(&previous_proof.public_values.hash());

        // In the old version i use write instead of write_vec
        // don't remember why.
        stdin.write(&previous_proof.public_values.to_vec());
    }


    if !args.dry_run {

        let proof: SP1ProofWithPublicValues;

        if args.groth16 {
            proof = client
                .prove(&pk, &stdin)
                .strategy(FulfillmentStrategy::Auction)
                .groth16()
                .run()
                .expect("failed to generate proof");
        } else {
            proof = client
                .prove(&pk, &stdin)
                .strategy(FulfillmentStrategy::Auction)
                .compressed()
                .run()
                .expect("failed to generate proof");
        }

        // Save proof to output location as JSON
        let output_path = &args.output_proof;
        let proof_json = serde_json::to_string_pretty(&proof).expect("failed to serialize proof as JSON");
        fs::write(output_path, &proof_json).expect("failed to write proof JSON to output location");
        println!("Proof successfully saved to {:?}", output_path);

    } else {
        let result = client
            .execute(CONSENSUS_VERIFIER_RECURSION_ELF, &stdin)
            .run()
            .expect("failed to execute program");

        let (_public_values, execution_report) = result;
        println!("Execution report: {:?}", execution_report);
    }
}