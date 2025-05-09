# Byteproc CLI Application Specification

## 1. Overall Purpose

This Linux command-line Rust application processes byte streams using configurable parameters specified by the user. The application supports hexadecimal string formats for input/output, modular transformation operations (e.g., encryption, encoding, error correction), and chaining of multiple processing modules via Linux pipes or ZeroMQ message queues.

## 2. Functional Requirements

### 2.1 Input and Output Handling
The program input byte streams and output byte streams are limited to a maximum configurable size of 

{
  "max_stream_size_kb": 64
}
If this size is exceeded on the input or output an error will be generated.

#### Supported Input Methods
The program shall accept input byte streams via:
- **Standard input (`stdin`)**
- **ZeroMQ PULL socket (`zmq_pull`)**

#### Supported Output Methods
The program shall output byte streams via:
- **Standard output (`stdout`)**
- **ZeroMQ PUSH socket (`zmq_push`)**

#### Hexadecimal Byte Conversion
- Input streams represented as hexadecimal strings (two hex characters per byte) must be converted internally to byte arrays (or vectors).
- Output byte arrays (or vectors) must be converted to hexadecimal strings.

#### Configuration Parameters

| JSON Key              | CLI Flag              | Description                                  |
|-----------------------|-----------------------|----------------------------------------------|
| `input_type`          | `--input-type`        | `"stdin"` or `"zmq_pull"`                    |
| `input_zmq_socket`    | `--input-zmq-socket`  | ZeroMQ socket endpoint (e.g., `tcp://*:5555`)|
| `input_zmq_bind`      | `--input-zmq-bind`    | `true`=bind, `false`=connect                 |
| `output_type`         | `--output-type`       | `"stdout"` or `"zmq_push"`                   |
| `output_zmq_socket`   | `--output-zmq-socket` | ZeroMQ socket endpoint (e.g., `tcp://*:5556`)|
| `output_zmq_bind`     | `--output-zmq-bind`   | `true`=bind, `false`=connect                 |

ZMQ Binding Behavior: 
- Use bind = true when this instance should be the ZeroMQ server (e.g., a known endpoint others connect to).
- Use bind = false when this instance should be a client (e.g., initiates connection to a known endpoint).

Clearly specify ZeroMQ socket options (LINGER, RECONNECT_INTERVAL, RECONNECT_MAX_ATTEMPTS, RECEIVE_TIMEOUT, SEND_TIMEOUT).

Provide explicit parameters and default values:
```
{
  "zmq_reconnect_interval_ms": 1000,
  "zmq_max_reconnect_attempts": 5,
  "zmq_send_timeout_ms": 5000,
  "zmq_receive_timeout_ms": 5000,
  "zmq_linger_ms": 0
}
```

#### Input/Output Validation
Enforce that stdin/stdout cannot be combined with a missing zmq_* parameter when the corresponding type is zmq_push or zmq_pull.
- Enforce presence of ZeroMQ parameters when ZeroMQ input/output types are chosen.
- Explicit error on missing or incompatible parameter configurations.

### 2.2 Configuration Management

Configuration parameters can be specified in two ways:
* JSON configuration file. JSON configuration file shall clearly specify all supported parameters in documented format.
* Command-line arguments (which override JSON file settings). Long-form command-line parameter names (--parameter-name) shall match the parameter keys in the JSON file.

Parameters specified via:

- **JSON configuration file** (`byteproc.json`)
- **CLI arguments** (`--parameter-name`), overriding JSON values

Validation logic should specifying behavior on conflicting or missing configurations. For example:
```
fn validate_config(cfg: &Config) -> Result<(), ByteProcError> {
    if cfg.input_type == "zmq_pull" {
        if cfg.input_zmq_socket.is_none() {
            return Err(ByteProcError::InvalidConfiguration(
                "input_zmq_socket must be specified for zmq_pull".to_string()));
        }
    }
    if cfg.output_type == "zmq_push" {
        if cfg.output_zmq_socket.is_none() {
            return Err(ByteProcError::InvalidConfiguration(
                "output_zmq_socket must be specified for zmq_push".to_string()));
        }
    }
    Ok(())
}
```

The json config file shall include explicit versioning of a configuration schema to handle backward compatibility issues. For example:
```
{
  "schema_version": "1.0"
}
```

### 2.3 Modularity and Extensibility

The architecture must enable modular byte transformation modules using a standardized Rust interface:

```rust
pub trait ByteProcessor {
    fn name(&self) -> &'static str;
    fn process(&self, input: &[u8]) -> Result<Vec<u8>, Box<dyn Error>>;
}
```
* The system architecture shall be modular, allowing easy integration of additional byte processing rust code modules in the program. Module integration via clear functions in the program code
* Each processing module shall define a standardized interface for byte transformation, clearly documented for developers.
* The application shall support chaining via Linux command-line pipes, enabling multiple byte processing modules running as separate instance programs to be sequentially composed. Chaining supported by Linux pipes (`|`) and ZeroMQ PUSH/PULL sockets.
* The application shall support multiple instances chained via any combination of Linux command-line pipes or ZeroMQ message queues, enabling multiple byte processing modules running as separate programs to be sequentially composed.

### 2.4 Logging and Diagnostics
The application shall provide detailed internal logging. There shall be centralized logging across instances. Users must be able to configurable log verbosity via a configuration parameter (log_level), with common levels such as DEBUG, INFO, WARN, and ERROR.

Log entries must include:
- Timestamp (ISO 8601 format, UTC recommended)
- Instance PID
- Log level
- Message content

Log message content must include at minimum:
- Input/output byte stream summaries (e.g., length, start/end sequences).
- Configuration parameters being used.
 -Error and exception details.

 Use structured JSON logging (optional, configurable). Include:
```
{
  "log_structured": true,
  "log_max_file_size_mb": 10,
  "log_rotation_count": 5
}
```

#### Logging Configuration Parameters

| JSON Key      | CLI Flag        | Type    | Description                                   |
|---------------|-----------------|---------|-----------------------------------------------|
| `log_enabled` | `--log-enabled` | boolean | Enable or disable logging                     |
| `log_level`   | `--log-level`   | string  | `error`, `warn`, `info`, `debug`, or `trace`  |
| `log_file`    | `--log-file`    | string  | Shared log file path                          |
| `log_append`  | `--log-append`  | boolean | Append (`true`) or overwrite (`false`) logs   |

### 2.5 Robustness and Error Handling

Implement comprehensive error detection and reporting, including:
- Malformed hex input or incomplete streams
- Invalid/conflicting configuration
- ZeroMQ socket failures (timeouts, retries with configurable intervals)

For example use an explicit enum-based error type in Rust:
```
#[derive(Debug)]
pub enum ByteProcError {
    MalformedHexInput,
    IncompleteInput,
    InvalidConfiguration(String), // with details
    ZeroMQSocketError(String),    // ZeroMQ error description
    ProcessingModuleError { module: String, description: String },
}
```
Each error shall clearly describe the cause and context in the log entry.


### 2.6 Testing and Validation

Automated testing required for each processing module, verifying:
- Roundtrip byte transformations (e.g., encode→decode). Tests shall include scenarios where one instance applies transformation and another reverses it (e.g., encrypt → decrypt), comparing results for identity verification.
- Edge cases: empty streams, large streams, invalid inputs
- Integration with CI/CD pipelines (`cargo test`, GitHub Actions)

## 3. Sample Byte Processing Modules
A simple static registry approach will be used to implement the modules. For example:

```
pub struct ModuleRegistry {
    modules: HashMap<&'static str, Box<dyn ByteProcessor>>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        let mut modules = HashMap::new();
        modules.insert("xor", Box::new(XorModule::new()));
        modules.insert("base64", Box::new(Base64Module::new()));
        Self { modules }
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn ByteProcessor>> {
        self.modules.get(name)
    }
}
```

### 3.1 Passthrough (Identity Module)
- Straight copy from input to output
- For module interface validation and pipeline testing

### 3.2 XOR Encryption/Decryption Module
A simple XOR encryption module.
- Encryption/decryption parameters (xor_key) shall be configurable via command-line and JSON file.
Should use the following parameters:

| JSON Key      | CLI Flag        | Type    | Description                                      |
|---------------|-----------------|---------|--------------------------------------------------|
| `xor_enabled` | `--xor-enabled` | boolean | Enable XOR processing                            |
| `xor_key`     | `--xor-key`     | string  | Hexadecimal XOR key (required, even length)      |
| `xor_pad`     | `--xor-pad`     | string  | Optional 1-byte hex pad (default to cycle key)   |

Note: `xor_mode` | `--xor-mode` of  `"encrypt"` or `"decrypt"` is not needed because xor inverts the previous operation

Use secure memory handling (e.g., zeroizing keys after use). For example:
```
use zeroize::Zeroize;

struct XorKey {
    key: Vec<u8>,
}

impl Drop for XorKey {
    fn drop(&mut self) {
        self.key.zeroize();
   
 }
}
```

This is a learning program so it is okay to log sensitive parameters like keys.

### 3.3 Base64 Encode/Decode Module
A base64 encoding function which uses the RFC 4648 encoding standard.
- Base64 encoding and base64 decoding modules should include validation checks.

Should use the following parameters:
| JSON Key          | CLI Flag           | Type    | Description                                                         |
|-------------------|--------------------|---------|---------------------------------------------------------------------|
| `base64_enabled`  | `--base64-enabled` | boolean | Enable Base64 processing                                            |
| `base64_mode`     | `--base64-mode`    | string  | `"encode"` or `"decode"`                                            |
| `base64_padding`  | `--base64-padding` | boolean | Include (`true`) or omit (`false`) = padding characters in encoding |


## 4. Parameter Naming Convention

Use consistent format: `moduleName_parameterName`. For example, XOR encryption parameter: xor_key, Base encoding parameter: base64_padding.

## 5. ZeroMQ Message Queue Architecture

Use PUSH/PULL socket pairs exclusively for linear data pipelines. The advantages of them are:
- Pipeline processing: PUSH/PULL sockets support one-way message flow, which can be used with modular pipeline processing model (multiple chained instances).
- Fault-tolerance: If a consumer (PULL) temporarily goes offline, messages queue at the PUSH side without message loss.

A typical scenario is: Producer instance (PUSH) → Processor instance (PULL) → (processing) → (PUSH) → Next Processor instance (PULL), and so forth.

## 6. Non-Functional Requirements

- **Performance:** Efficient byte stream processing (real-time/batch). Use rust benchmarking tools to measure such as crate.
- **Portability:** Compatible across major Linux distributions.
- **Documentation:** Comprehensive CLI help, JSON schema documentation, and Rust API examples.

## 7. Configuration Resolution Order

1. CLI (`--config <filename>`)
2. Environment variable (`BYTEPROC_CONFIG`)
3. Local default (`./byteproc.json`)
4. User config fallback (`~/.config/byteproc/config.json`)

## 8. Supported Rust Versions and Toolchains
The required Rust version and minimum toolchain is stable Rust >= 1.75
