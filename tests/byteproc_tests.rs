use byteproc::processor::{Passthrough, ByteProcessor};
use byteproc::processor::{Config, ModuleRegistry}; // Removed ByteProcError as it's unused
use clap::Parser; // Import the Parser trait
use hex;
// tempfile is used in the ignored test, ensure it's in Cargo.toml [dev-dependencies]
use tempfile;

#[test]
fn test_passthrough() {
    let module = Passthrough;
    assert_eq!(module.process(&[1,2,3,4]).unwrap(), vec![1,2,3,4]);
}


#[test]
fn test_xor_processing_via_config() {
    // 1. Setup CLI args to enable XOR with a specific key
    let cli_args_vec = vec![
        "byteproc", // Program name, required by parse_from
        "--xor-enabled",
        "--xor-key",
        "abcd1234",
    ];

    // 2. Create Config from CLI arguments using Config::parse_from()
    let config = Config::parse_from(cli_args_vec.iter());
    // Note: Config::parse_from does not run the `validate()` method by default.
    // If these args were invalid, `parse_from` would panic or return an error
    // depending on clap's behavior for the specific parsing issue.
    // For this test, we assume the args are valid enough to construct a Config
    // for testing ModuleRegistry.

    // 3. Create ModuleRegistry from the Config
    // The ModuleRegistry::new might itself return an error if the config is invalid for it
    // (e.g. xor_enabled but xor_key is None, which is handled by Config::validate,
    // but parse_from doesn't call validate).
    // For this test to be robust, ensure that the config produced by parse_from
    // is valid for ModuleRegistry::new.
    // The current Config::validate checks for xor_key if xor_enabled.
    // Config::parse_from will set xor_key to Some("abcd1234")
    let registry = ModuleRegistry::new(&config).expect("ModuleRegistry creation should succeed");

    let input_bytes = hex::decode("00112233").expect("Failed to decode input hex");
    let expected_output_bytes = vec![0xab, 0xdc, 0x30, 0x07];

    let processed_bytes = registry.process_all(input_bytes).expect("Processing failed");

    assert_eq!(processed_bytes, expected_output_bytes);
}

#[test]
#[ignore] // This requires actual running processes, so we only run it selectively
// Run with cargo test test_zmq_passthrough_chain -- --nocapture --ignored
fn test_zmq_passthrough_chain() {
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::Command;
    
    // Create test directory for artifacts
    let test_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let test_path = test_dir.path();
    let script_path = test_path.join("zmq_test.sh");
    
    // Create test script
    let script_content = r#"#!/bin/bash
set -e

# Set up log files
LOG_DIR="$(pwd)/zmq_test_logs"
mkdir -p "$LOG_DIR"

# Test parameters
TEST_DATA="deadbeefcafebabe"
PORT1="5551"
PORT2="5552"

# Clean up prior test processes if any
pkill -f "byteproc.*zmq" || true
sleep 1

# Start the chain from the end
echo "Starting final receiver..."
./target/debug/byteproc \
  --input-type zmq_pull \
  --input-zmq-socket "tcp://*:$PORT2" \
  --input-zmq-bind true \
  --log-file "$LOG_DIR/receiver.log" \
  --zmq-receive-timeout-ms 5000 \
  > "$LOG_DIR/output.txt" &
RECEIVER_PID=$!
sleep 1

echo "Starting middle processor..."
./target/debug/byteproc \
  --input-type zmq_pull \
  --input-zmq-socket "tcp://*:$PORT1" \
  --input-zmq-bind true \
  --output-type zmq_push \
  --output-zmq-socket "tcp://localhost:$PORT2" \
  --output-zmq-bind false \
  --log-file "$LOG_DIR/middle.log" \
  --zmq-receive-timeout-ms 5000 &
MIDDLE_PID=$!
sleep 1

echo "Sending test data..."
echo "$TEST_DATA" | ./target/debug/byteproc \
  --output-type zmq_push \
  --output-zmq-socket "tcp://localhost:$PORT1" \
  --output-zmq-bind false \
  --log-file "$LOG_DIR/sender.log"

# Wait for processing to complete
wait $RECEIVER_PID || true
wait $MIDDLE_PID || true

# Check the result
RESULT=$(cat "$LOG_DIR/output.txt")
if [ "$RESULT" = "$TEST_DATA" ]; then
  echo "TEST PASSED: Input matches output"
  echo "[$TEST_DATA] == [$RESULT]"
  exit 0
else
  echo "TEST FAILED: Input does not match output"
  echo "Expected: [$TEST_DATA]"
  echo "Got:      [$RESULT]"
  exit 1
fi
"#;

    // Write script to temp directory and make executable
    let mut file = File::create(&script_path).expect("Failed to create script file");
    file.write_all(script_content.as_bytes()).expect("Failed to write script content");
    
    // Make the script executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).expect("Failed to get metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("Failed to set permissions");
    }
    
    // Build the debug binary first
    Command::new("cargo")
        .args(["build"])
        .status()
        .expect("Failed to build debug binary");
    
    // Run the test script
    let output = Command::new(&script_path)
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")))
        .output()
        .expect("Failed to execute test script");
    
    // Check if the test passed
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    println!("Test script stdout:\n{}", stdout);
    println!("Test script stderr:\n{}", stderr);
    
    assert!(
        stdout.contains("TEST PASSED"),
        "ZMQ chain test failed. stdout: {}, stderr: {}",
        stdout, stderr
    );
}