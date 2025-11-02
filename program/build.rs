use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Read TALLY_PROGRAM_ID from environment
    let program_id = env::var("TALLY_PROGRAM_ID").expect(
        "TALLY_PROGRAM_ID environment variable must be set. \
         Example: export TALLY_PROGRAM_ID=YourProgramIdHere111111111111111111111111111",
    );

    // Parse the base58 string into a Pubkey to validate it and get bytes
    let pubkey_bytes = bs58::decode(&program_id)
        .into_vec()
        .expect("TALLY_PROGRAM_ID must be a valid base58-encoded public key");

    if pubkey_bytes.len() != 32 {
        panic!(
            "TALLY_PROGRAM_ID must decode to exactly 32 bytes, got {} bytes",
            pubkey_bytes.len()
        );
    }

    // Write the bytes to a file that can be included at compile time
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("program_id.bin");
    fs::write(&dest_path, &pubkey_bytes).expect("Failed to write program ID bytes");

    println!("cargo:rerun-if-env-changed=TALLY_PROGRAM_ID");
}
