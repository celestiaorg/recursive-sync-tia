use clap::Parser;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin, Prover};
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

    /// Path to first header JSON file (optional)
    #[arg(long, value_name = "PATH")]
    h1: Option<PathBuf>,

    /// Path to second header JSON file (required)
    #[arg(long, value_name = "PATH")]
    h2: PathBuf,

    /// Private key for the prover client
    #[arg(short = 'k', long, value_name = "PRIVATE_KEY")]
    private_key: String,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();

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

    // Read and deserialize h1 (optional)
    let h1: Option<LightBlock> = args.h1.map(|path| {
        if !path.exists() {
            eprintln!("Error: h1 file does not exist: {:?}", path);
            std::process::exit(1);
        }
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| {
                eprintln!("Error reading h1 file: {}", e);
                std::process::exit(1);
            });
        serde_json::from_str(&content)
            .unwrap_or_else(|e| {
                eprintln!("Error deserializing h1 JSON: {}", e);
                std::process::exit(1);
            })
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

    // Setup the prover client.
    let client = ProverClient::builder()
        .network()
        .private_key(&args.private_key)
        .build();

    let mut stdin = SP1Stdin::new();

    // Write genesis hash
    stdin.write(&genesis.signed_header.header().hash().as_bytes().to_vec());

    // Write h1 presence flag and optional value
    match &h1 {
        Some(block) => {
            stdin.write(&true);
            let h1_bytes = serde_cbor::to_vec(block).unwrap();
            stdin.write_vec(h1_bytes);
        }
        None => {
            stdin.write(&false);
            stdin.write_vec(Vec::<u8>::new()); // Write empty vec as placeholder
        }
    }

    // Write h2
    let h2_bytes = serde_cbor::to_vec(&h2).unwrap();
    stdin.write_vec(h2_bytes);

    let (pk, vk) = client.setup(CONSENSUS_VERIFIER_RECURSION_ELF);
    /*let proof = client
        .prove(&pk, &stdin)
        .compressed()
        .run()
        .expect("failed to generate proof");*/
    
    let result = client
        .execute(CONSENSUS_VERIFIER_RECURSION_ELF, &stdin)
        .run()
        .expect("failed to execute program");

    let (public_values, execution_report) = result;

    //println!("Proof: {:?}", proof);

    //client.verify(&proof, &vk).expect("failed to verify proof");

}