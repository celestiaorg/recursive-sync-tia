use clap::Parser;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use sp1_verifier;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tendermint_light_client_verifier::{
    options::Options, types::LightBlock, ProdVerifier, Verdict, Verifier,
};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const CONSENSUS_VERIFIER_RECURSION_ELF: &[u8] =
    include_elf!("program-verify-consensus-recursion");

/// Prove consensus verification from headers directory
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to directory containing header JSON files
    #[arg(short = 'd', long, value_name = "PATH")]
    headers_dir: PathBuf,

    #[arg(short = 'e', long)]
    execute: bool,

    #[arg(short = 'p', long)]
    prove: bool,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse the command line arguments.
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Verify headers directory exists
    if !args.headers_dir.exists() {
        eprintln!("Error: Headers directory does not exist: {:?}", args.headers_dir);
        std::process::exit(1);
    }

    println!("Reading headers from: {:?}", args.headers_dir);

    // Read all block files from the directory
    let mut block_files: Vec<(PathBuf, String)> = fs::read_dir(&args.headers_dir)
        .expect("Failed to read headers directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_file() && path.extension()? == "json" {
                let filename = path.file_name()?.to_str()?;
                if filename.starts_with("block_") {
                    Some((path.clone(), filename.to_string()))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    // Sort by block number
    block_files.sort_by_key(|(_, filename)| {
        filename
            .trim_start_matches("block_")
            .trim_end_matches(".json")
            .parse::<u64>()
            .unwrap_or(0)
    });

    let blocks: Vec<LightBlock> = block_files.iter().map(|(path, _filename)| {
        let content = fs::read_to_string(path).expect("Failed to read header file");
        let block: LightBlock = serde_json::from_str(&content).unwrap();
        block
    }).collect();

    let vp = ProdVerifier::default();
    let opt = Options {
        trust_threshold: Default::default(),
        // 2 week trusting period.
        trusting_period: Duration::from_secs(14 * 24 * 60 * 60),
        clock_drift: Default::default(),
    };

    let _orphan: Option<&LightBlock> = if blocks.len() % 2 != 0 {
        Some(&blocks[blocks.len() - 1])
    } else { None };

    for window in blocks.windows(2) {
        let prev = &window[0];
        let next = &window[1];
        let time = next.time();
        let verdict = vp.verify_update_header(
            next.as_untrusted_state(), 
            prev.as_trusted_state(), 
            &opt, 
            (time + Duration::from_secs(20)).unwrap()
        );
        println!("{:?}", verdict);
    }

    /*

    // Setup the prover client.
    let client = ProverClient::from_env();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();

    // TODO: Read and process the header files into the stdin
    // For now, this is a placeholder - you'll need to adapt based on your program's input format
    for (path, filename) in &block_files {
        let content = fs::read_to_string(path).expect("Failed to read header file");
        println!("Processing: {}", filename);
        // stdin.write(&content); // Adapt this based on your needs
    }

    if args.execute {
        // Execute the program
        let (output, report) = client
            .execute(CONSENSUS_VERIFIER_RECURSION_ELF, &stdin)
            .run()
            .unwrap();
        println!("Program executed successfully.");

        // Record the number of cycles executed.
        println!("Number of cycles: {}", report.total_instruction_count());
    } else {
        // Setup the program for proving.
        let (pk, vk) = client.setup(CONSENSUS_VERIFIER_RECURSION_ELF);

        // Generate the proof
        let proof = client
            .prove(&pk, &stdin)
            .run()
            .expect("failed to generate proof");

        println!("Successfully generated proof!");

        // Verify the proof.
        client.verify(&proof, &vk).expect("failed to verify proof");
        println!("Successfully verified proof!");
    }
    */
}