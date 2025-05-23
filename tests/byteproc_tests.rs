use byteproc::processor::{Passthrough, ByteProcessor};
use byteproc::processor::{Config, ModuleRegistry}; // Removed ByteProcError as it's unused
use clap::Parser; // Import the Parser trait
use hex;
use std::str::FromStr;
use byteproc::processor::{
    InputType, OutputType, Base64Mode, Base64Module, XorModule,
    ByteProcError,
};


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

// this is a long test so only run when specified 
#[test]
#[ignore]
fn test_integration_3_blocks_via_bash_script() {
    let script_name = "tests/test_integration_3_blocks.bash";
    let status = std::process::Command::new("bash")
        .arg(script_name)
        .status()
        .expect(&format!("Failed to execute test script: {}", script_name));
    
    assert!(
        status.success(), 
        "System test script failed: {} (exit code: {})",
        script_name,
        status.code().unwrap_or(-1)
    );

}

#[test]
fn test_input_type_from_str_valid() {
    assert_eq!(InputType::from_str("stdin").unwrap(), InputType::Stdin);
    assert_eq!(InputType::from_str("zmq_pull").unwrap(), InputType::ZmqPull);
    // case‐insensitive
    assert_eq!(InputType::from_str("STDIN").unwrap(), InputType::Stdin);
}

#[test]
fn test_input_type_from_str_invalid() {
    assert!(InputType::from_str("unknown").is_err());
}

#[test]
fn test_output_type_from_str_and_display() {
    assert_eq!(OutputType::from_str("stdout").unwrap(), OutputType::Stdout);
    assert_eq!(OutputType::from_str("zmq_push").unwrap(), OutputType::ZmqPush);
    assert_eq!(format!("{}", OutputType::Stdout), "stdout");
    assert_eq!(format!("{}", OutputType::ZmqPush), "zmq_push");
    assert!(OutputType::from_str("invalid").is_err());
}

#[test]
fn test_config_max_stream_size_overflow() {
    let mut cfg = Config::default();
    cfg.max_stream_size_kb = usize::MAX;
    let err = cfg.max_stream_size().unwrap_err();
    // should be an InvalidConfiguration error
    assert!(matches!(err, ByteProcError::InvalidConfiguration(_)));
    assert!(err.to_string().contains("max_stream_size_kb too large"));
}

#[test]
fn test_config_base64_encode_flag() {
    let mut cfg = Config::default();
    cfg.base64_mode = Base64Mode::Decode;
    assert!(!cfg.base64_encode());
    cfg.base64_mode = Base64Mode::Encode;
    assert!(cfg.base64_encode());
}

#[test]
fn test_config_xor_pad_byte_parsing() {
    let mut cfg = Config::default();
    // default_xor_pad is "00"
    assert_eq!(cfg.xor_pad_byte(), Some(0));
    cfg.xor_pad = "ff".into();
    assert_eq!(cfg.xor_pad_byte(), Some(0xff));
    // invalid hex
    cfg.xor_pad = "GG".into();
    assert_eq!(cfg.xor_pad_byte(), None);
}

#[test]
fn test_config_validate_conditions() {
    let mut cfg = Config::default();
    // missing ZMQ pull socket
    cfg.input_type = InputType::ZmqPull;
    assert!(matches!(cfg.validate(), Err(ByteProcError::InvalidConfiguration(_))));
    // missing ZMQ push socket
    let mut cfg = Config::default();
    cfg.output_type = OutputType::ZmqPush;
    assert!(cfg.validate().is_err());
    // missing XOR key
    let mut cfg = Config::default();
    cfg.xor_enabled = true;
    assert!(cfg.validate().is_err());

    // default config is valid
    let cfg = Config::default();
    assert!(cfg.validate().is_ok());
}

#[test]
fn test_xor_module_new_empty_key() {
    let err = XorModule::new("", None).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Invalid configuration: xor_key cannot be empty"
    );
}

#[test]
fn test_xor_module_process_only() {
    // set up a single‐byte key of 0xff
    let module = XorModule::new("ff", Some(0)).unwrap();
    // XOR each byte against 0xff
    let data = vec![0x00, 0x0f, 0xff];
    let out = module.process(&data).unwrap();
    assert_eq!(out, vec![0xff, 0xf0, 0x00]);
}

#[test]
fn test_base64_module_roundtrip_and_error() {
    let plaintext = b"hello world";
    // encode with padding
    let enc = Base64Module::new(true, true).process(plaintext).unwrap();
    assert_eq!(enc, b"aGVsbG8gd29ybGQ=".to_vec());

    // decode back
    let dec = Base64Module::new(false, true).process(&enc).unwrap();
    assert_eq!(&dec, plaintext);

    // decode invalid input → Module error
    let err = Base64Module::new(false, true)
        .process(b"!!! not base64 !!!")
        .unwrap_err();
    assert!(matches!(err, ByteProcError::Module(_)));
}

#[test]
fn test_module_registry_only_xor() {
    let mut cfg = Config::default();
    cfg.xor_enabled = true;
    cfg.xor_key = Some("0f".into());
    let registry = ModuleRegistry::new(&cfg).unwrap();

    // XOR with 0x0f: [1,2,3] → [0e,0d,0c]
    let out = registry.process_all(vec![1, 2, 3]).unwrap();
    assert_eq!(out, vec![0x0e, 0x0d, 0x0c]);
}

#[test]
fn test_module_registry_only_base64() {
    let mut cfg = Config::default();
    cfg.base64_enabled = true;
    cfg.base64_mode = Base64Mode::Encode;
    cfg.base64_padding = false;
    let registry = ModuleRegistry::new(&cfg).unwrap();

    let out = registry.process_all(b"foo".to_vec()).unwrap();
    // "foo" → "Zm9v" (no padding)
    assert_eq!(out, b"Zm9v".to_vec());
}

#[test]
fn test_module_registry_xor_then_base64() {
    let mut cfg = Config::default();
    cfg.xor_enabled = true;
    cfg.xor_key = Some("ff".into());
    cfg.base64_enabled = true;
    cfg.base64_mode = Base64Mode::Encode;
    cfg.base64_padding = false;
    let registry = ModuleRegistry::new(&cfg).unwrap();

    // byte 0xff → XOR → 0x00 → base64 no‐pad → "AA"
    let out = registry.process_all(vec![0xff]).unwrap();
    assert_eq!(out, b"AA".to_vec());
}
