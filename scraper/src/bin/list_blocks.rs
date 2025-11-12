use clap::Parser;
use std::path::PathBuf;
use std::fs;

/// List block JSON files in numerical order
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to directory containing block JSON files
    #[arg(short, long, value_name = "PATH")]
    input_path: PathBuf,
}

fn main() {
    let args = Args::parse();

    // Read directory entries
    let entries = match fs::read_dir(&args.input_path) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading directory {:?}: {}", args.input_path, e);
            return;
        }
    };

    // Collect block filenames and their heights
    let mut blocks: Vec<(u64, String)> = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();

        // Parse filenames like "block_123.json"
        if filename_str.starts_with("block_") && filename_str.ends_with(".json") {
            let height_str = &filename_str[6..filename_str.len()-5]; // Extract number between "block_" and ".json"

            if let Ok(height) = height_str.parse::<u64>() {
                blocks.push((height, filename_str.to_string()));
            }
        }
    }

    // Sort by height (numerical order)
    blocks.sort_by_key(|&(height, _)| height);

    // Print filenames in order
    for (_height, filename) in blocks {
        println!("{}", filename);
    }
}
