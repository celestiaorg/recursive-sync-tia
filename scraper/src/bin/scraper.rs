use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;
use std::fs;

use tendermint_light_client_verifier::{
    options::Options, types::LightBlock, ProdVerifier, Verdict, Verifier,
};

use scraper::tm_rpc_utils::TendermintRPCClient;

/// Celestia header scraper
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to store downloaded headers
    #[arg(short, long, value_name = "PATH")]
    output_path: PathBuf,
    
    /// Tendermint RPC URL
    #[arg(short, long, value_name = "URL", default_value = "http://localhost:26657")]
    rpc_url: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    
    println!("Celestia Scraper starting...");
    println!("Headers will be stored in: {:?}", args.output_path);
    println!("Using RPC URL: {}", args.rpc_url);
    
    let client = TendermintRPCClient::new(args.rpc_url);

    let peer_id = client.fetch_peer_id().await.unwrap();
    
    let latest_block_height = client.get_latest_block_height().await;
    
    let start = 1;
    let end = latest_block_height;

    let vp = ProdVerifier::default();
    let opt = Options {
        trust_threshold: Default::default(),
        // 2 week trusting period.
        trusting_period: Duration::from_secs(14 * 24 * 60 * 60),
        clock_drift: Default::default(),
    };

    // Create output directory if it doesn't exist
    fs::create_dir_all(&args.output_path).expect("Failed to create output directory");

    // Fetch genesis block (height 1) as our initial trusted block
    println!("Fetching genesis header...");
    let genesis_block = client.fetch_light_block(start, peer_id).await.unwrap();
    save_light_block(&genesis_block, start, &args.output_path);

    // Start binary search to find minimum verification path
    println!("Starting binary search to find minimum verification path from height {} to {}...", start, end);

    let mut verified_blocks = vec![genesis_block];
    let mut current_height = start;

    while current_height < end {
        let target_height = find_next_verifiable_block(
            &client,
            &vp,
            &opt,
            &verified_blocks.last().unwrap(),
            current_height + 1,
            end,
            peer_id,
            &args.output_path,
        ).await;

        if let Some(next_height) = target_height {
            println!("Successfully verified jump from {} to {}", current_height, next_height);
            let next_block = client.fetch_light_block(next_height, peer_id).await.unwrap();
            save_light_block(&next_block, next_height, &args.output_path);
            verified_blocks.push(next_block);
            current_height = next_height;
        } else {
            println!("Failed to find verifiable path from {}. Trying next block...", current_height);
            current_height += 1;
            let next_block = client.fetch_light_block(current_height, peer_id).await.unwrap();
            save_light_block(&next_block, current_height, &args.output_path);
            verified_blocks.push(next_block);
        }
    }

    println!("Verification complete! Found minimum path with {} blocks", verified_blocks.len());
}

/// Saves a LightBlock to a JSON file
fn save_light_block(block: &LightBlock, height: u64, output_path: &PathBuf) {
    let filename = format!("block_{}.json", height);
    let filepath = output_path.join(filename);

    let json = serde_json::to_string_pretty(block).expect("Failed to serialize LightBlock");
    fs::write(&filepath, json).expect("Failed to write LightBlock to file");

    println!("Saved block at height {} to {:?}", height, filepath);
}

/// Uses binary search to find the furthest block that can be verified from the trusted block
async fn find_next_verifiable_block(
    client: &scraper::tm_rpc_utils::TendermintRPCClient,
    verifier: &ProdVerifier,
    options: &Options,
    trusted_block: &LightBlock,
    start_height: u64,
    end_height: u64,
    peer_id: [u8; 20],
    _output_path: &PathBuf,
) -> Option<u64> {
    if start_height > end_height {
        return None;
    }

    // First, try to verify directly to the end
    println!("Attempting to verify from {} to {}...", trusted_block.height().value(), end_height);

    let target_block = match client.fetch_light_block(end_height, peer_id).await {
        Ok(block) => block,
        Err(e) => {
            println!("✗ Error fetching block at height {}: {}", end_height, e);
            return None;
        }
    };

    match try_verify(verifier, options, trusted_block, &target_block).await {
        Ok(true) => {
            println!("✓ Successfully verified jump to {}", end_height);
            return Some(end_height);
        }
        Ok(false) => {
            println!("✗ Failed to verify jump to {}", end_height);
        }
        Err(e) => {
            println!("✗ Error verifying jump to {}: {}", end_height, e);
        }
    }

    // If we can't verify to the end, do binary search
    let mut left = start_height;
    let mut right = end_height;
    let mut best_verifiable: Option<u64> = None;

    while left <= right {
        let mid = left + (right - left) / 2;

        println!("Binary search: trying height {} (range: {} to {})", mid, left, right);

        let target_block = match client.fetch_light_block(mid, peer_id).await {
            Ok(block) => block,
            Err(e) => {
                println!("✗ Error fetching block at height {}: {}", mid, e);
                if mid == 0 {
                    break;
                }
                right = mid - 1;
                continue;
            }
        };

        match try_verify(verifier, options, trusted_block, &target_block).await {
            Ok(true) => {
                println!("✓ Successfully verified jump to {}", mid);
                best_verifiable = Some(mid);
                // Try to find a further block
                left = mid + 1;
            }
            Ok(false) => {
                println!("✗ Failed to verify jump to {}", mid);
                // Try a closer block
                if mid == 0 {
                    break;
                }
                right = mid - 1;
            }
            Err(e) => {
                println!("✗ ERROR!!! verifying jump to {}: {}", mid, e);
                if mid == 0 {
                    break;
                }
                right = mid - 1;
            }
        }
    }

    best_verifiable
}

/// Attempts to verify a target block against a trusted block
async fn try_verify(
    verifier: &ProdVerifier,
    options: &Options,
    trusted_block: &LightBlock,
    target_block: &LightBlock,
) -> Result<bool, Box<dyn std::error::Error>> {

    // Get verification time (target block time + some buffer)
    let verify_time = (target_block.time() + Duration::from_secs(20))
        .map_err(|e| format!("Failed to calculate verify time: {:?}", e))?;

    // Attempt verification
    let verdict = verifier.verify_update_header(
        target_block.as_untrusted_state(),
        trusted_block.as_trusted_state(),
        options,
        verify_time,
    );

    match verdict {
        Verdict::Success => Ok(true),
        Verdict::NotEnoughTrust(_) => Ok(false),
        Verdict::Invalid(e) => {
            println!("  Invalid verdict: {:?}", e);
            Ok(false)
        }
    }
}
