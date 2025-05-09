// src/lib.rs

pub mod processor {
use clap::Parser;
use hex::FromHex;
use log::{ info, LevelFilter};
use serde::Deserialize;
use simplelog::{ConfigBuilder, WriteLogger};
use std::{
    collections::HashMap,
    error::Error,
    fmt,
    fs::{File, OpenOptions},
    io::{self, Read},
    path::PathBuf,
    str::FromStr,
};
use base64::Engine;
use zeroize::Zeroize;
use zmq::{Context, Socket};

// -------------- Error type --------------

#[derive(Debug)]
pub enum ByteProcError {
    Io(String),
    InvalidConfiguration(String),
    HexDecode(String),
    MaxSizeExceeded(usize, usize),
    Zmq(String),
    Module(String),
}

impl fmt::Display for ByteProcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ByteProcError::Io(e) => write!(f, "I/O error: {}", e),
            ByteProcError::InvalidConfiguration(e) => write!(f, "Invalid configuration: {}", e),
            ByteProcError::HexDecode(e) => write!(f, "Hex decode error: {}", e),
            ByteProcError::MaxSizeExceeded(max, got) => {
                write!(f, "Stream too large: max {} bytes, got {}", max, got)
            }
            ByteProcError::Zmq(e) => write!(f, "ZeroMQ error: {}", e),
            ByteProcError::Module(e) => write!(f, "Module processing error: {}", e),
        }
    }
}

impl Error for ByteProcError {}

// -------------- ByteProcessor trait --------------

pub trait ByteProcessor {
    fn name(&self) -> &'static str;
    fn process(&self, input: &[u8]) -> Result<Vec<u8>, ByteProcError>;
}

// -------------- Modules --------------

/// Passthrough
pub struct Passthrough;
impl ByteProcessor for Passthrough {
    fn name(&self) -> &'static str { "passthrough" }
    fn process(&self, input: &[u8]) -> Result<Vec<u8>, ByteProcError> {
        Ok(input.to_vec())
    }
}

/// XOR
pub struct XorModule {
    key: XorKey,
}
struct XorKey { key: Vec<u8> }
impl Drop for XorKey { fn drop(&mut self) { self.key.zeroize(); } }
impl XorModule {
    pub fn new(hex_key: &str, pad_byte: Option<u8>) -> Result<Self, ByteProcError> {
        let raw = Vec::from_hex(hex_key)
            .map_err(|e| ByteProcError::HexDecode(e.to_string()))?;
        if raw.is_empty() {
            return Err(ByteProcError::InvalidConfiguration("xor_key cannot be empty".into()));
        }
        // pad or cycle?
        let _pad = pad_byte.unwrap_or(0);
        // Note: we'll cycle if pad_byte is None; no further action here
        Ok(XorModule {
            key: XorKey { key: raw },
        })
    }
}
impl ByteProcessor for XorModule {
    fn name(&self) -> &'static str { "xor" }
    fn process(&self, input: &[u8]) -> Result<Vec<u8>, ByteProcError> {
        let mut out = Vec::with_capacity(input.len());
        let key = &self.key.key;
        for (i, &b) in input.iter().enumerate() {
            let k = key[i % key.len()];
            out.push(b ^ k);
        }
        Ok(out)
    }
}

/// Base64
pub struct Base64Module {
    encode: bool,
    padding: bool,
}
impl Base64Module {
    pub fn new(encode: bool, padding: bool) -> Self {
        Base64Module { encode, padding }
    }
}
impl ByteProcessor for Base64Module {
    fn name(&self) -> &'static str { "base64" }
    fn process(&self, input: &[u8]) -> Result<Vec<u8>, ByteProcError> {
        if self.encode {
            let cfg = if self.padding {
                base64::engine::general_purpose::STANDARD
            } else {
                base64::engine::general_purpose::STANDARD_NO_PAD
            };
            Ok(cfg.encode(input).into_bytes())
        } else {
            let cfg = if self.padding {
                base64::engine::general_purpose::STANDARD
            } else {
                base64::engine::general_purpose::STANDARD_NO_PAD
            };
            cfg.decode(input).map_err(|e| ByteProcError::Module(e.to_string()))
            }
    }
}

// -------------- Config structures --------------

#[derive(Deserialize)]
struct RawConfig {
    schema_version: Option<String>,
    max_stream_size_kb: Option<usize>,

    input_type: Option<String>,
    input_zmq_socket: Option<String>,
    input_zmq_bind: Option<bool>,
    output_type: Option<String>,
    output_zmq_socket: Option<String>,
    output_zmq_bind: Option<bool>,

    zmq_reconnect_interval_ms: Option<u32>,
    zmq_max_reconnect_attempts: Option<u32>,
    zmq_send_timeout_ms: Option<i32>,
    zmq_receive_timeout_ms: Option<i32>,
    zmq_linger_ms: Option<i32>,

    log_enabled: Option<bool>,
    log_level: Option<String>,
    log_file: Option<String>,
    log_append: Option<bool>,
    log_max_file_size_mb: Option<u64>,
    log_rotation_count: Option<usize>,

    xor_enabled: Option<bool>,
    xor_key: Option<String>,
    xor_pad: Option<String>,

    base64_enabled: Option<bool>,
    base64_mode: Option<String>,
    base64_padding: Option<bool>,
}

impl Default for RawConfig {
    fn default() -> Self {
        RawConfig {
            schema_version: Some("1.0".into()),
            max_stream_size_kb: Some(64),

            input_type: Some("stdin".into()),
            input_zmq_socket: None,
            input_zmq_bind: Some(false),
            output_type: Some("stdout".into()),
            output_zmq_socket: None,
            output_zmq_bind: Some(false),

            zmq_reconnect_interval_ms: Some(1000),
            zmq_max_reconnect_attempts: Some(5),
            zmq_send_timeout_ms: Some(5000),
            zmq_receive_timeout_ms: Some(5000),
            zmq_linger_ms: Some(0),

            log_enabled: Some(true),
            log_level: Some("info".into()),
            log_file: Some("byteproc.log".into()),
            log_append: Some(true),
            log_max_file_size_mb: Some(10),
            log_rotation_count: Some(5),

            xor_enabled: Some(false),
            xor_key: None,
            xor_pad: Some("00".into()),

            base64_enabled: Some(false),
            base64_mode: Some("encode".into()),
            base64_padding: Some(true),
        }
    }
}

#[derive(Parser)]
#[command(name = "byteproc")]
pub struct Cli {
    /// Path to config file
    #[arg(long)]
    config: Option<PathBuf>,

    #[arg(long)] input_type: Option<String>,
    #[arg(long)] input_zmq_socket: Option<String>,
    #[arg(long)] input_zmq_bind: Option<bool>,
    #[arg(long)] output_type: Option<String>,
    #[arg(long)] output_zmq_socket: Option<String>,
    #[arg(long)] output_zmq_bind: Option<bool>,

    #[arg(long)] zmq_reconnect_interval_ms: Option<u32>,
    #[arg(long)] zmq_max_reconnect_attempts: Option<u32>,
    #[arg(long)] zmq_send_timeout_ms: Option<i32>,
    #[arg(long)] zmq_receive_timeout_ms: Option<i32>,
    #[arg(long)] zmq_linger_ms: Option<i32>,

    #[arg(long)] log_enabled: Option<bool>,
    #[arg(long)] log_level: Option<String>,
    #[arg(long)] log_file: Option<String>,
    #[arg(long)] log_append: Option<bool>,
    #[arg(long)] log_structured: Option<bool>,

    #[arg(long)] max_stream_size_kb: Option<usize>,

    #[arg(long)] xor_enabled: Option<bool>,
    #[arg(long)] xor_key: Option<String>,
    #[arg(long)] xor_pad: Option<String>,

    #[arg(long)] base64_enabled: Option<bool>,
    #[arg(long)] base64_mode: Option<String>,
    #[arg(long)] base64_padding: Option<bool>,
}

#[derive(Debug)]
pub struct Config {
    max_stream_size: usize,

    input_type: String,
    input_zmq_socket: Option<String>,
    input_zmq_bind: bool,
    output_type: String,
    output_zmq_socket: Option<String>,
    output_zmq_bind: bool,

    zmq_reconnect_interval_ms: u32,
    zmq_max_reconnect_attempts: u32,
    zmq_send_timeout_ms: i32,
    zmq_receive_timeout_ms: i32,
    zmq_linger_ms: i32,

    log_enabled: bool,
    log_level: String,
    log_file: String,
    log_append: bool,

    xor_enabled: bool,
    xor_key: Option<String>,
    xor_pad: Option<u8>,

    base64_enabled: bool,
    base64_encode: bool,
    base64_padding: bool,
}

impl Config {
    pub fn from(cli: Cli) -> Result<Self, ByteProcError> {
        // 1) Determine config file path
        let path = if let Some(cfg) = cli.config {
            cfg
        } else if let Ok(env) = std::env::var("BYTEPROC_CONFIG") {
            PathBuf::from(env)
        } else if PathBuf::from("byteproc.json").exists() {
            PathBuf::from("byteproc.json")
        } else {
            PathBuf::from("./byteproc.json") // final fallback
        };

        // 2) Load JSON
        let mut raw = RawConfig::default();
        if let Ok(f) = File::open(&path) {
            let mut s = String::new();
            let mut rdr = io::BufReader::new(f);
            rdr.read_to_string(&mut s)
                .map_err(|e| ByteProcError::Io(e.to_string()))?;
            let file_cfg: RawConfig =
                serde_json::from_str(&s).map_err(|e| ByteProcError::Io(e.to_string()))?;
            // merge file_cfg into raw
            raw = RawConfig {
                schema_version: file_cfg.schema_version.or(raw.schema_version),
                max_stream_size_kb: file_cfg.max_stream_size_kb.or(raw.max_stream_size_kb),
                input_type: file_cfg.input_type.or(raw.input_type),
                input_zmq_socket: file_cfg.input_zmq_socket.or(raw.input_zmq_socket),
                input_zmq_bind: file_cfg.input_zmq_bind.or(raw.input_zmq_bind),
                output_type: file_cfg.output_type.or(raw.output_type),
                output_zmq_socket: file_cfg.output_zmq_socket.or(raw.output_zmq_socket),
                output_zmq_bind: file_cfg.output_zmq_bind.or(raw.output_zmq_bind),
                zmq_reconnect_interval_ms: file_cfg
                    .zmq_reconnect_interval_ms
                    .or(raw.zmq_reconnect_interval_ms),
                zmq_max_reconnect_attempts: file_cfg
                    .zmq_max_reconnect_attempts
                    .or(raw.zmq_max_reconnect_attempts),
                zmq_send_timeout_ms: file_cfg
                    .zmq_send_timeout_ms
                    .or(raw.zmq_send_timeout_ms),
                zmq_receive_timeout_ms: file_cfg
                    .zmq_receive_timeout_ms
                    .or(raw.zmq_receive_timeout_ms),
                zmq_linger_ms: file_cfg.zmq_linger_ms.or(raw.zmq_linger_ms),
                log_enabled: file_cfg.log_enabled.or(raw.log_enabled),
                log_level: file_cfg.log_level.or(raw.log_level),
                log_file: file_cfg.log_file.or(raw.log_file),
                log_append: file_cfg.log_append.or(raw.log_append),
                log_max_file_size_mb: file_cfg
                    .log_max_file_size_mb
                    .or(raw.log_max_file_size_mb),
                log_rotation_count: file_cfg
                    .log_rotation_count
                    .or(raw.log_rotation_count),
                xor_enabled: file_cfg.xor_enabled.or(raw.xor_enabled),
                xor_key: file_cfg.xor_key.or(raw.xor_key),
                xor_pad: file_cfg.xor_pad.or(raw.xor_pad),
                base64_enabled: file_cfg.base64_enabled.or(raw.base64_enabled),
                base64_mode: file_cfg.base64_mode.or(raw.base64_mode),
                base64_padding: file_cfg.base64_padding.or(raw.base64_padding),
            };
        }

        // 3) Override with CLI
        let override_str = |cli: Option<String>, raw: Option<String>, default: String| {
            cli.or(raw).unwrap_or(default)
        };
        let override_bool = |cli: Option<bool>, raw: Option<bool>, default: bool| {
            cli.or(raw).unwrap_or(default)
        };

        let max_stream_size =
            cli.max_stream_size_kb
                .or(raw.max_stream_size_kb)
                .unwrap()
                .checked_mul(1024)
                .ok_or_else(|| {
                    ByteProcError::InvalidConfiguration("max_stream_size_kb too large".into())
                })?;

        let input_type = override_str(cli.input_type, raw.input_type, "stdin".into());
        let input_zmq_socket = cli
            .input_zmq_socket
            .or(raw.input_zmq_socket.clone());
        let input_zmq_bind =
            override_bool(cli.input_zmq_bind, raw.input_zmq_bind, false);

        let output_type = override_str(cli.output_type, raw.output_type, "stdout".into());
        let output_zmq_socket = cli
            .output_zmq_socket
            .or(raw.output_zmq_socket.clone());
        let output_zmq_bind =
            override_bool(cli.output_zmq_bind, raw.output_zmq_bind, false);

        let zmq_reconnect_interval_ms = cli
            .zmq_reconnect_interval_ms
            .or(raw.zmq_reconnect_interval_ms)
            .unwrap();
        let zmq_max_reconnect_attempts = cli
            .zmq_max_reconnect_attempts
            .or(raw.zmq_max_reconnect_attempts)
            .unwrap();
        let zmq_send_timeout_ms =
            cli.zmq_send_timeout_ms.or(raw.zmq_send_timeout_ms).unwrap();
        let zmq_receive_timeout_ms = cli
            .zmq_receive_timeout_ms
            .or(raw.zmq_receive_timeout_ms)
            .unwrap();
        let zmq_linger_ms = cli.zmq_linger_ms.or(raw.zmq_linger_ms).unwrap();

        let log_enabled = override_bool(cli.log_enabled, raw.log_enabled, true);
        let log_level =
            override_str(cli.log_level, raw.log_level, "info".into());
        let log_file =
            override_str(cli.log_file, raw.log_file, "byteproc.log".into());
        let log_append =
            override_bool(cli.log_append, raw.log_append, true);

        let xor_enabled = override_bool(cli.xor_enabled, raw.xor_enabled, false);
        let xor_key = cli.xor_key.or(raw.xor_key.clone());
        let xor_pad = cli
            .xor_pad
            .or(raw.xor_pad.clone())
            .and_then(|s| u8::from_str_radix(&s, 16).ok());

        let base64_enabled =
            override_bool(cli.base64_enabled, raw.base64_enabled, false);
        let base64_mode =
            override_str(cli.base64_mode, raw.base64_mode, "encode".into());
        let base64_encode = base64_mode == "encode";
        let base64_padding =
            override_bool(cli.base64_padding, raw.base64_padding, true);

        // 4) Validate
        if input_type == "zmq_pull" && input_zmq_socket.is_none() {
            return Err(ByteProcError::InvalidConfiguration(
                "input_zmq_socket must be set for zmq_pull".into(),
            ));
        }
        if output_type == "zmq_push" && output_zmq_socket.is_none() {
            return Err(ByteProcError::InvalidConfiguration(
                "output_zmq_socket must be set for zmq_push".into(),
            ));
        }
        if xor_enabled && xor_key.is_none() {
            return Err(ByteProcError::InvalidConfiguration(
                "xor_key must be set if xor_enabled".into(),
            ));
        }

        Ok(Config {
            max_stream_size,
            input_type,
            input_zmq_socket,
            input_zmq_bind,
            output_type,
            output_zmq_socket,
            output_zmq_bind,
            zmq_reconnect_interval_ms,
            zmq_max_reconnect_attempts,
            zmq_send_timeout_ms,
            zmq_receive_timeout_ms,
            zmq_linger_ms,
            log_enabled,
            log_level,
            log_file,
            log_append,
            xor_enabled,
            xor_key,
            xor_pad,
            base64_enabled,
            base64_encode,
            base64_padding,
        })
    }
}

// -------------- Module registry --------------

pub struct ModuleRegistry {
    modules: HashMap<&'static str, Box<dyn ByteProcessor>>,
}

impl ModuleRegistry {
    pub fn new(cfg: &Config) -> Result<Self, ByteProcError> {
        let mut modules: HashMap<&'static str, Box<dyn ByteProcessor>> = HashMap::new();
        // Passthrough always present
        modules.insert("passthrough", Box::new(Passthrough));

        // XOR
        if cfg.xor_enabled {
            let m = XorModule::new(
                cfg.xor_key.as_ref().unwrap(),
                cfg.xor_pad,
            )?;
            modules.insert("xor", Box::new(m));
        }

        // Base64
        if cfg.base64_enabled {
            let m = Base64Module::new(cfg.base64_encode, cfg.base64_padding);
            modules.insert("base64", Box::new(m));
        }

        Ok(ModuleRegistry { modules })
    }

    /// process through all enabled modules in insertion order:
    pub fn process_all(
        &self,
        mut data: Vec<u8>,
    ) -> Result<Vec<u8>, ByteProcError> {
        for (name, module) in &self.modules {
            info!("Running module: {}", name);
            data = module.process(&data)?;
        }
        Ok(data)
    }
}

// -------------- Main --------------

pub(crate) fn main_internal(cfg: Config) -> Result<(), Box<dyn Error>> {

    if cfg.log_enabled {
        let level = LevelFilter::from_str(&cfg.log_level).unwrap_or(LevelFilter::Info);
        let file = OpenOptions::new()
            .append(cfg.log_append)
            .create(true)
            .open(&cfg.log_file)
            .map_err(|e| ByteProcError::Io(e.to_string()))?;
        let log_cfg = ConfigBuilder::new()
            .set_time_format_str("%+")
            .build();
        WriteLogger::init(level, log_cfg, file).unwrap();
    }

    // Prepare ZeroMQ if needed
    let context = Context::new();
    let mut input_socket: Option<Socket> = None;
    let mut output_socket: Option<Socket> = None;

    if cfg.input_type == "zmq_pull" {
        let sock = context.socket(zmq::PULL)?;
        sock.set_reconnect_ivl(cfg.zmq_reconnect_interval_ms as i32)?;
        sock.set_reconnect_ivl_max(cfg.zmq_max_reconnect_attempts as i32)?;
        sock.set_rcvtimeo(cfg.zmq_receive_timeout_ms)?;
        sock.set_linger(cfg.zmq_linger_ms)?;
        if cfg.input_zmq_bind {
            sock.bind(cfg.input_zmq_socket.as_ref().unwrap())?;
        } else {
            sock.connect(cfg.input_zmq_socket.as_ref().unwrap())?;
        }
        input_socket = Some(sock);
    }

    if cfg.output_type == "zmq_push" {
        let sock = context.socket(zmq::PUSH)?;
        sock.set_reconnect_ivl(cfg.zmq_reconnect_interval_ms as i32)?;
        sock.set_reconnect_ivl_max(cfg.zmq_max_reconnect_attempts as i32)?;
        sock.set_sndtimeo(cfg.zmq_send_timeout_ms)?;
        sock.set_linger(cfg.zmq_linger_ms)?;
        if cfg.output_zmq_bind {
            sock.bind(cfg.output_zmq_socket.as_ref().unwrap())?;
        } else {
            sock.connect(cfg.output_zmq_socket.as_ref().unwrap())?;
        }
        output_socket = Some(sock);
    }

    // Read input
    let raw_hex = if cfg.input_type == "stdin" {
        let mut s = String::new();
        io::stdin().read_to_string(&mut s)
            .map_err(|e| ByteProcError::Io(e.to_string()))?;
        s.trim().to_string()
    } else {
        let msg = input_socket
            .as_ref()
            .unwrap()
            .recv_msg(0)
            .map_err(|e| ByteProcError::Zmq(e.to_string()))?;
        msg.as_str()
            .ok_or_else(|| ByteProcError::HexDecode("Invalid UTF-8 from ZMQ".into()))?
            .trim()
            .to_string()
    };
    info!("Received hex input (len={} chars)", raw_hex.len());

    // Decode hex
    let bytes = Vec::from_hex(&raw_hex)
        .map_err(|e| ByteProcError::HexDecode(e.to_string()))?;
    if bytes.len() > cfg.max_stream_size {
        return Err(ByteProcError::MaxSizeExceeded(cfg.max_stream_size, bytes.len()).into());
    }

    // Process modules
    let registry = ModuleRegistry::new(&cfg)?;
    let processed = registry.process_all(bytes)?;

    if processed.len() > cfg.max_stream_size {
        return Err(ByteProcError::MaxSizeExceeded(cfg.max_stream_size, processed.len()).into());
    }

    // Encode hex
    let out_hex = hex::encode(&processed);

    // Write output
    if cfg.output_type == "stdout" {
        println!("{}", out_hex);
    } else {
        output_socket
            .as_ref()
            .unwrap()
            .send(&out_hex, 0)
            .map_err(|e| ByteProcError::Zmq(e.to_string()))?;
    }

    Ok(())
}


}

use clap::Parser; // Add this import at the top of the file

/// A convenient entrypoint for the binary:
pub fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    let cli = crate::processor::Cli::parse();
    let cfg = crate::processor::Config::from(cli)?;
    
    crate::processor::main_internal(cfg)?; // Call the refactored main logic
    Ok(())
}