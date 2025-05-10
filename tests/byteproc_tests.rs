
use byteproc::processor::{Passthrough, ByteProcessor};
use byteproc::processor::{Cli, Config, ByteProcError, ModuleRegistry};
use clap::Parser;
use hex; // Added for hex decoding

#[test]
fn test_passthrough() {
    let module = Passthrough;
    assert_eq!(module.process(&[1,2,3,4]).unwrap(), vec![1,2,3,4]);
}

#[test]
fn test_config_validation_missing_zmq_input() {
    let cli = Cli::parse_from(&["byteproc", "--input-type", "zmq_pull"]);
    let res = Config::from(cli);
    assert!(res.is_err());
    if let Err(ByteProcError::InvalidConfiguration(msg)) = res {
        assert!(msg.contains("input_zmq_socket"));
    } else {
        panic!("Expected InvalidConfiguration error");
    }
}

#[test]
fn test_xor_processing_via_config() {
    // 1. Setup CLI args to enable XOR with a specific key
    let cli_args = &[
        "byteproc", // Program name, required by parse_from
        "--xor-enabled",
        "true",
        "--xor-key",
        "abcd1234",
    ];
    let cli = Cli::parse_from(cli_args);

    // 2. Create Config from CLI arguments
    let config = Config::from(cli).expect("Config creation should succeed");

    // 3. Create ModuleRegistry from the Config
    // This will initialize the XorModule based on the config.
    let registry = ModuleRegistry::new(&config).expect("ModuleRegistry creation should succeed");

    // 4. Define input and expected output
    // Input hex: "00112233" -> Bytes: [0x00, 0x11, 0x22, 0x33]
    // XOR key hex: "abcd1234" -> Bytes: [0xab, 0xcd, 0x12, 0x34]
    // Expected XORed result:
    // 0x00 ^ 0xab = 0xab
    // 0x11 ^ 0xcd = 0xdc
    // 0x22 ^ 0x12 = 0x30
    // 0x33 ^ 0x34 = 0x07
    // Expected bytes: [0xab, 0xdc, 0x30, 0x07]
    let input_bytes = hex::decode("00112233").expect("Failed to decode input hex");
    let expected_output_bytes = vec![0xab, 0xdc, 0x30, 0x07];

    // 5. Process data through the registry
    let processed_bytes = registry.process_all(input_bytes).expect("Processing failed");

    // 6. Assert that the processed bytes match the expected output
    assert_eq!(processed_bytes, expected_output_bytes);
}