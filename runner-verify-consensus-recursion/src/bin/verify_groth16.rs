use clap::Parser;
use std::fs;
use std::path::PathBuf;

/// Export or verify Groth16 verification key
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Export the current sp1_verifier Groth16 VK to a file
    #[arg(short = 'e', long, value_name = "OUTPUT_PATH")]
    export: Option<PathBuf>,

    /// Compare a Groth16 VK file with the current sp1_verifier VK
    #[arg(short = 'c', long, value_name = "VK_FILE")]
    compare: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    // Get the current Groth16 VK from sp1_verifier
    let current_vk = sp1_verifier::GROTH16_VK_BYTES.clone();

    if let Some(export_path) = args.export.clone() {
        // Export the VK to a file
        fs::write(&export_path, current_vk.to_vec())
            .expect("Failed to write Groth16 VK to file");
        println!("Groth16 VK exported to {:?}", export_path);
        println!("VK size: {} bytes", current_vk.len());
    }

    if let Some(compare_path) = args.compare.clone() {
        // Read the VK file
        if !compare_path.exists() {
            eprintln!("Error: VK file does not exist: {:?}", compare_path);
            std::process::exit(1);
        }

        let file_vk = fs::read(&compare_path)
            .expect("Failed to read VK file");

        // Compare
        if file_vk == *current_vk {
            println!("✓ VK file matches sp1_verifier::GROTH16_VK_BYTES");
            println!("  Size: {} bytes", file_vk.len());
        } else {
            println!("✗ VK mismatch!");
            println!("  File VK size: {} bytes", file_vk.len());
            println!("  Current sp1_verifier VK size: {} bytes", current_vk.len());

            if file_vk.len() == current_vk.len() {
                // Find first differing byte
                for (i, (a, b)) in file_vk.iter().zip(current_vk.iter()).enumerate() {
                    if a != b {
                        println!("  First difference at byte {}: file={:#04x}, current={:#04x}", i, a, b);
                        break;
                    }
                }
            }
            std::process::exit(1);
        }
    }

    if args.export.is_none() && args.compare.is_none() {
        println!("No action specified. Use --export or --compare.");
        println!("Current sp1_verifier::GROTH16_VK_BYTES size: {} bytes", current_vk.len());
    }
}
