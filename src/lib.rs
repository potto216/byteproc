// src/lib.rs

pub mod processor {
use clap::Parser;
use hex::FromHex;
use log::{ info,error, LevelFilter};
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
use std::sync::OnceLock;

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

#[derive(Parser, Deserialize, Debug, Clone)]
#[command(name = "byteproc")]
pub struct Config {
    /// Path to config file
    #[arg(long)]
    #[serde(skip)]
    pub config: Option<PathBuf>,

    /// Maximum stream size in KB
    #[arg(long, default_value_t = 64)]
    #[serde(default = "default_max_stream_size_kb")]
    pub max_stream_size_kb: usize,

    // Input/Output options
    #[arg(long, default_value = "stdin")]
    #[serde(default = "default_input_type")]
    pub input_type: String,

    #[arg(long)]
    #[serde(default)]
    pub input_zmq_socket: Option<String>,

    #[arg(long, default_value_t = false)]
    #[serde(default)]
    pub input_zmq_bind: bool,

    #[arg(long, default_value = "stdout")]
    #[serde(default = "default_output_type")]
    pub output_type: String,

    #[arg(long)]
    #[serde(default)]
    pub output_zmq_socket: Option<String>,

    #[arg(long, default_value_t = false)]
    #[serde(default)]
    pub output_zmq_bind: bool,

    // ZMQ options
    #[arg(long, default_value_t = 1000)]
    #[serde(default = "default_zmq_reconnect_interval_ms")]
    pub zmq_reconnect_interval_ms: u32,

    #[arg(long, default_value_t = 5)]
    #[serde(default = "default_zmq_max_reconnect_attempts")]
    pub zmq_max_reconnect_attempts: u32,

    #[arg(long, default_value_t = 5000)]
    #[serde(default = "default_zmq_send_timeout_ms")]
    pub zmq_send_timeout_ms: i32,

    #[arg(long, default_value_t = 5000)]
    #[serde(default = "default_zmq_receive_timeout_ms")]
    pub zmq_receive_timeout_ms: i32,

    #[arg(long, default_value_t = 0)]
    #[serde(default = "default_zmq_linger_ms")]
    pub zmq_linger_ms: i32,

    // Logging options
    #[arg(long, default_value_t = true)]
    #[serde(default = "default_log_enabled")]
    pub log_enabled: bool,

    #[arg(long, default_value = "info")]
    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[arg(long, default_value = "byteproc.log")]
    #[serde(default = "default_log_file")]
    pub log_file: String,

    #[arg(long, default_value_t = true)]
    #[serde(default = "default_log_append")]
    pub log_append: bool,

    // Processing modules
    #[arg(long, default_value_t = false)]
    #[serde(default)]
    pub xor_enabled: bool,

    #[arg(long)]
    #[serde(default)]
    pub xor_key: Option<String>,

    #[arg(long, default_value = "00")]
    #[serde(default = "default_xor_pad")]
    pub xor_pad: String,

    #[arg(long, default_value_t = false)]
    #[serde(default)]
    pub base64_enabled: bool,

    #[arg(long, default_value = "encode")]
    #[serde(default = "default_base64_mode")]
    pub base64_mode: String,

    #[arg(long, default_value_t = true)]
    #[serde(default = "default_base64_padding")]
    pub base64_padding: bool,
}

// Default function implementations
fn default_max_stream_size_kb() -> usize { 64 }
fn default_input_type() -> String { "stdin".into() }
fn default_output_type() -> String { "stdout".into() }
fn default_zmq_reconnect_interval_ms() -> u32 { 1000 }
fn default_zmq_max_reconnect_attempts() -> u32 { 5 }
fn default_zmq_send_timeout_ms() -> i32 { 5000 }
fn default_zmq_receive_timeout_ms() -> i32 { 5000 }
fn default_zmq_linger_ms() -> i32 { 3000 }
fn default_log_enabled() -> bool { true }
fn default_log_level() -> String { "info".into() }
fn default_log_file() -> String { "byteproc.log".into() }
fn default_log_append() -> bool { true }
fn default_xor_pad() -> String { "00".into() }
fn default_base64_mode() -> String { "encode".into() }
fn default_base64_padding() -> bool { true }

// Implement the Default trait for Config
impl Default for Config {
    fn default() -> Self {
        Config {
            config: None,
            max_stream_size_kb: default_max_stream_size_kb(),
            input_type: default_input_type(),
            input_zmq_socket: None,
            input_zmq_bind: false, // Default for bool
            output_type: default_output_type(),
            output_zmq_socket: None,
            output_zmq_bind: false, // Default for bool
            zmq_reconnect_interval_ms: default_zmq_reconnect_interval_ms(),
            zmq_max_reconnect_attempts: default_zmq_max_reconnect_attempts(),
            zmq_send_timeout_ms: default_zmq_send_timeout_ms(),
            zmq_receive_timeout_ms: default_zmq_receive_timeout_ms(),
            zmq_linger_ms: default_zmq_linger_ms(),
            log_enabled: default_log_enabled(),
            log_level: default_log_level(),
            log_file: default_log_file(),
            log_append: default_log_append(),
            xor_enabled: false, // Default for bool
            xor_key: None,
            xor_pad: default_xor_pad(),
            base64_enabled: false, // Default for bool
            base64_mode: default_base64_mode(),
            base64_padding: default_base64_padding(),
        }
    }
}

impl Config {
    /// Calculated field: Maximum stream size in bytes
    pub fn max_stream_size(&self) -> Result<usize, ByteProcError> {
        self.max_stream_size_kb
            .checked_mul(1024)
            .ok_or_else(|| ByteProcError::InvalidConfiguration("max_stream_size_kb too large".into()))
    }
    
    /// Calculated field: Base64 encode mode
    pub fn base64_encode(&self) -> bool {
        self.base64_mode == "encode"
    }
    
    /// Calculated field: XOR pad byte
    pub fn xor_pad_byte(&self) -> Option<u8> {
        u8::from_str_radix(&self.xor_pad, 16).ok()
    }
    
    /// Load configuration from command line and optional config file
    pub fn load() -> Result<Self, ByteProcError> {
        // Parse command line args first
        let cli_args = Self::parse(); // Renamed to cli_args to avoid confusion with config variable
        
        // Determine if we should load from a file
        let mut config_from_file = if let Some(path) = &cli_args.config {
            // Explicitly provided config file
            Self::from_file(path)?
        } else if let Ok(env_path) = std::env::var("BYTEPROC_CONFIG") {
            // Config file from environment variable
            Self::from_file(&PathBuf::from(env_path))?
        } else if PathBuf::from("byteproc.json").exists() {
            // Default config file in current directory
            Self::from_file(&PathBuf::from("byteproc.json"))?
        } else {
            // No config file, use defaults
            Self::default()
        };
        
        // Override file/default values with CLI values where provided
        // We check if the CLI arg was actually passed by clap or if it's using its own default.
        // This requires checking against the default values defined by clap for simple types,
        // or checking if Option types are Some.

        // Create a default instance of CLI args to compare against
        let default_cli_args = Self::try_parse_from(&["byteproc"]).unwrap_or_else(|_| Self::default());


        if cli_args.max_stream_size_kb != default_cli_args.max_stream_size_kb {
            config_from_file.max_stream_size_kb = cli_args.max_stream_size_kb;
        }
        if cli_args.input_type != default_cli_args.input_type {
            config_from_file.input_type = cli_args.input_type;
        }
        if cli_args.input_zmq_socket.is_some() { // For Option types, just check if Some
            config_from_file.input_zmq_socket = cli_args.input_zmq_socket;
        }
        if cli_args.input_zmq_bind != default_cli_args.input_zmq_bind {
             config_from_file.input_zmq_bind = cli_args.input_zmq_bind;
        }
        if cli_args.output_type != default_cli_args.output_type {
            config_from_file.output_type = cli_args.output_type;
        }
        if cli_args.output_zmq_socket.is_some() {
            config_from_file.output_zmq_socket = cli_args.output_zmq_socket;
        }
        if cli_args.output_zmq_bind != default_cli_args.output_zmq_bind {
            config_from_file.output_zmq_bind = cli_args.output_zmq_bind;
        }
        if cli_args.zmq_reconnect_interval_ms != default_cli_args.zmq_reconnect_interval_ms {
            config_from_file.zmq_reconnect_interval_ms = cli_args.zmq_reconnect_interval_ms;
        }
        if cli_args.zmq_max_reconnect_attempts != default_cli_args.zmq_max_reconnect_attempts {
            config_from_file.zmq_max_reconnect_attempts = cli_args.zmq_max_reconnect_attempts;
        }
        if cli_args.zmq_send_timeout_ms != default_cli_args.zmq_send_timeout_ms {
            config_from_file.zmq_send_timeout_ms = cli_args.zmq_send_timeout_ms;
        }
        if cli_args.zmq_receive_timeout_ms != default_cli_args.zmq_receive_timeout_ms {
            config_from_file.zmq_receive_timeout_ms = cli_args.zmq_receive_timeout_ms;
        }
        if cli_args.zmq_linger_ms != default_cli_args.zmq_linger_ms {
            config_from_file.zmq_linger_ms = cli_args.zmq_linger_ms;
        }
        if cli_args.log_enabled != default_cli_args.log_enabled {
            config_from_file.log_enabled = cli_args.log_enabled;
        }
        if cli_args.log_level != default_cli_args.log_level {
            config_from_file.log_level = cli_args.log_level;
        }
        if cli_args.log_file != default_cli_args.log_file {
            config_from_file.log_file = cli_args.log_file;
        }
        if cli_args.log_append != default_cli_args.log_append {
            config_from_file.log_append = cli_args.log_append;
        }
        if cli_args.xor_enabled != default_cli_args.xor_enabled {
            config_from_file.xor_enabled = cli_args.xor_enabled;
        }
        if cli_args.xor_key.is_some() {
            config_from_file.xor_key = cli_args.xor_key;
        }
        if cli_args.xor_pad != default_cli_args.xor_pad {
            config_from_file.xor_pad = cli_args.xor_pad;
        }
        if cli_args.base64_enabled != default_cli_args.base64_enabled {
            config_from_file.base64_enabled = cli_args.base64_enabled;
        }
        if cli_args.base64_mode != default_cli_args.base64_mode {
            config_from_file.base64_mode = cli_args.base64_mode;
        }
        if cli_args.base64_padding != default_cli_args.base64_padding {
            config_from_file.base64_padding = cli_args.base64_padding;
        }

        // The config path itself from CLI should always override
        if cli_args.config.is_some() {
            config_from_file.config = cli_args.config;
        }
        
        // Validate the final configuration
        config_from_file.validate()?;
        
        Ok(config_from_file)
    }
    
    /// Load configuration from a file
    fn from_file(path: &PathBuf) -> Result<Self, ByteProcError> {
        let file = File::open(path)
            .map_err(|e| ByteProcError::Io(format!("Failed to open config file: {}", e)))?;
            
        let mut reader = io::BufReader::new(file);
        let mut contents = String::new();
        reader.read_to_string(&mut contents)
            .map_err(|e| ByteProcError::Io(format!("Failed to read config file: {}", e)))?;
            
        serde_json::from_str(&contents)
            .map_err(|e| ByteProcError::Io(format!("Failed to parse config file: {}", e)))
    }
    
    /// Validate the configuration
    fn validate(&self) -> Result<(), ByteProcError> {
        // Check required fields for specific input/output types
        if self.input_type == "zmq_pull" && self.input_zmq_socket.is_none() {
            return Err(ByteProcError::InvalidConfiguration(
                "input_zmq_socket must be set for zmq_pull".into(),
            ));
        }
        
        if self.output_type == "zmq_push" && self.output_zmq_socket.is_none() {
            return Err(ByteProcError::InvalidConfiguration(
                "output_zmq_socket must be set for zmq_push".into(),
            ));
        }
        
        if self.xor_enabled && self.xor_key.is_none() {
            return Err(ByteProcError::InvalidConfiguration(
                "xor_key must be set if xor_enabled".into(),
            ));
        }
        
        Ok(())
    }
}

// -------------- Helpers --------------
// Static instance ID initialized on first access
static INSTANCE_ID: OnceLock<String> = OnceLock::new();

/// Generate a unique instance identifier for logging
/// The ID is generated only once per process and then reused
fn make_instance_id() -> &'static str {
    INSTANCE_ID.get_or_init(|| {
        format!("pid-{}-{:x}", 
            std::process::id(), 
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() & 0xFFFF)
    })
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
                cfg.xor_pad_byte(),
            )?;
            modules.insert("xor", Box::new(m));
        }

        // Base64
        if cfg.base64_enabled {
            let m = Base64Module::new(cfg.base64_encode(), cfg.base64_padding);
            modules.insert("base64", Box::new(m));
        }

        Ok(ModuleRegistry { modules })
    }

    /// process through all enabled modules in insertion order:
    pub fn process_all(
        &self,
        mut data: Vec<u8>,
    ) -> Result<Vec<u8>, ByteProcError> {
        let instance_id = make_instance_id();
        for (name, module) in &self.modules {
            info!("[{}] Processing with module: {}", instance_id, name);
            data = module.process(&data)?;
        }
        Ok(data)
    }
}

// -------------- Main --------------

pub(crate) fn main_internal(cfg: Config) -> Result<(), Box<dyn Error>> {
    // Generate a unique instance ID for this run
    let instance_id = make_instance_id();
    
    if cfg.log_enabled {
        let level = LevelFilter::from_str(&cfg.log_level).unwrap_or(LevelFilter::Info);
        let file = OpenOptions::new()
            .append(cfg.log_append)
            .create(true)
            .open(&cfg.log_file)
            .map_err(|e| ByteProcError::Io(e.to_string()))?;
        
        // Configure logger with instance ID in the format
        let log_cfg = ConfigBuilder::new()
            .set_time_format_str("%+")
            .set_thread_level(LevelFilter::Off)  // Turn off thread ID logging
            .set_target_level(LevelFilter::Off)  // Turn off target logging
            .set_location_level(LevelFilter::Off) // Turn off code location
            .add_filter_ignore_str("mio")  // Ignore noisy libraries
            .set_time_to_local(true)
            .build();
        
        WriteLogger::init(level, log_cfg, file).unwrap();
        
        // Log the start of this instance
        info!("[{}] Byteproc starting up", instance_id);
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
            info!("[{}] Binding PULL socket to {}", instance_id, 
                cfg.input_zmq_socket.as_ref().unwrap());
            sock.bind(cfg.input_zmq_socket.as_ref().unwrap())?;
        } else {
            info!("[{}] Connecting PULL socket to {}", instance_id,
                cfg.input_zmq_socket.as_ref().unwrap());
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
            info!("[{}] Binding PUSH socket to {}", instance_id, 
                cfg.output_zmq_socket.as_ref().unwrap());
            sock.bind(cfg.output_zmq_socket.as_ref().unwrap())?;
        } else {
            info!("[{}] Connecting PUSH socket to {}", instance_id,
                cfg.output_zmq_socket.as_ref().unwrap());
            sock.connect(cfg.output_zmq_socket.as_ref().unwrap())?;
        }
        output_socket = Some(sock);
    }

    // Read input
    let raw_hex = if cfg.input_type == "stdin" {
        let mut s = String::new();
        info!("[{}] Reading from stdin...", instance_id);
        io::stdin().read_to_string(&mut s)
            .map_err(|e| ByteProcError::Io(e.to_string()))?;
        info!("[{}] Finished reading from stdin ({} chars)", instance_id, s.trim().len());
        s.trim().to_string()
    } else {
        // This is the zmq_pull case
        let socket_ref = input_socket
            .as_ref()
            .ok_or_else(|| ByteProcError::InvalidConfiguration("Input socket not initialized for ZMQ".into()))?;

        info!(
            "[{}] Waiting for ZMQ message on PULL socket (timeout: {}ms)...",
            instance_id, cfg.zmq_receive_timeout_ms
        );
        let msg = socket_ref
            .recv_msg(0)
            .map_err(|e| {
                error!("[{}] ZMQ recv_msg error: {}", instance_id, e);
                ByteProcError::Zmq(e.to_string())
            })?;
        info!("[{}] Received ZMQ message ({} bytes)", instance_id, msg.len());

        let s = msg.as_str()
            .ok_or_else(|| {
                error!("[{}] Failed to convert ZMQ message to UTF-8 string", instance_id);
                ByteProcError::HexDecode("Invalid UTF-8 from ZMQ".into())
            })?;
        info!("[{}] Successfully converted ZMQ message to string ({} chars)", 
            instance_id, s.trim().len());
        s.trim().to_string()
    };
    info!("[{}] Received hex input (len={} chars)", instance_id, raw_hex.len());

    // Decode hex
    let bytes = Vec::from_hex(&raw_hex)
        .map_err(|e| ByteProcError::HexDecode(e.to_string()))?;
    if bytes.len() > cfg.max_stream_size()? {
        return Err(ByteProcError::MaxSizeExceeded(cfg.max_stream_size()?, bytes.len()).into());
    }

    // Process modules
    let registry = ModuleRegistry::new(&cfg)?;
    let processed = registry.process_all(bytes)?;

    if processed.len() > cfg.max_stream_size()? {
        return Err(ByteProcError::MaxSizeExceeded(cfg.max_stream_size()?, processed.len()).into());
    }

    // Encode hex
    let out_hex = hex::encode(&processed);

    // Write output
    if cfg.output_type == "stdout" {
        info!("[{}] Writing output to stdout", instance_id);
        println!("{}", out_hex);
    } else {
        info!("[{}] Sending output via ZMQ", instance_id);
        output_socket
            .as_ref()
            .unwrap()
            .send(&out_hex, 0)
            .map_err(|e| ByteProcError::Zmq(e.to_string()))?;
    }

    info!("[{}] Processing complete", instance_id);

    Ok(())
}


}

/// A convenient entrypoint for the binary:
pub fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = crate::processor::Config::load()?;
    crate::processor::main_internal(cfg)?;
    Ok(())
}