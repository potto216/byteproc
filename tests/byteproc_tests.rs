
use byteproc::processor::{Passthrough, ByteProcessor};
use byteproc::processor::{Cli, Config};
use clap::Parser;

#[test]
fn test_passthrough() {
    let module = Passthrough;
    assert_eq!(module.process(&[1,2,3,4]).unwrap(), vec![1,2,3,4]);
}

// … (other tests from before) …

#[test]
fn test_config_validation_missing_zmq_input() {
    let cli = Cli::parse_from(&["byteproc", "--input-type", "zmq_pull"]);
    let res = Config::from(cli);
    assert!(res.is_err());
    let msg = format!("{}", res.unwrap_err());
    assert!(msg.contains("input_zmq_socket"));
}
