# Byteproc

**Byteproc** is a modular, configurable command-line tool for processing byte streams in Rust. It supports hex input/output, modular transformations (XOR, Base64, passthrough), and can chain processing via Linux pipes or ZeroMQ sockets. Configuration is possible via CLI flags or a JSON file.

---

## Table of Contents

- [Overview](#overview)
- [Setup](#setup)
- [Building](#building)
- [Configuration](#configuration)
- [Examples](#examples)
  - [Basic Usage (stdin/stdout)](#basic-usage-stdinstdout)
  - [XOR Module Example](#xor-module-example)
  - [Custom Logging](#custom-logging)
  - [ZeroMQ Integration](#zeromq-integration)
- [Supported CLI Flags](#supported-cli-flags)
- [Testing](#testing)
- [License](#license)

---

## Overview

**Byteproc** processes byte streams using a configurable pipeline of modules. It is designed for flexibility, allowing you to:
- Read/write data via stdin/stdout or ZeroMQ sockets
- Apply transformations (XOR, Base64, passthrough)
- Chain multiple instances for complex pipelines
- Configure all behavior via CLI or JSON

---

## Setup

Install Rust (if not already):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup install stable
rustup default stable
```

---

## Building

Clone and build the project:

```bash
git clone https://github.com/youruser/byteproc.git
cd byteproc
cargo build --release
```

The binary will be at `./target/release/byteproc`.

---

## Configuration

Byteproc can be configured via:
- **Command-line flags** (highest priority)
- **JSON config file** (default: `byteproc.json` in the current directory)
- **Environment variable** (`BYTEPROC_CONFIG`)

Example `byteproc.json`:

```json
{
  "schema_version": "1.0",
  "log_enabled": true,
  "log_level": "info",
  "log_file": "./logs/byteproc.log",
  "log_append": true,
  "xor_enabled": true,
  "xor_key": "abcd1234",
  "base64_enabled": false
}
```

---

## Examples

### Basic Usage (stdin/stdout)

Process a hex string from stdin and output the result to stdout:

```bash
echo "deadbeef" | ./target/release/byteproc
```

### XOR Module Example

Enable XOR processing with a key:

```bash
echo "00112233" | ./target/release/byteproc \
  --xor-enabled true \
  --xor-key abcd1234
```

### Custom Logging

Specify log level, log file location, and append mode:

```bash
echo "48656c6c6f20776f726c64" | ./target/release/byteproc \
  --log-enabled true \
  --log-level debug \
  --log-file ./logs/byteproc-$(date +%Y%m%d).log \
  --log-append true
```

**Log levels:** `error`, `warn`, `info`, `debug`, `trace`

### ZeroMQ Integration

#### Terminal A (Sink / PULL)

```bash
./target/release/byteproc \
  --input-type zmq_pull \
  --input-zmq-socket tcp://*:5555 \
  --input-zmq-bind true
```

#### Terminal B (Source / PUSH)

```bash
echo "cafebabe" | ./target/release/byteproc \
  --output-type zmq_push \
  --output-zmq-socket tcp://localhost:5555 \
  --output-zmq-bind false
```

---

## Supported CLI Flags

| Flag                   | Description                                      |
|------------------------|--------------------------------------------------|
| `--input-type`         | `"stdin"` or `"zmq_pull"`                        |
| `--input-zmq-socket`   | ZeroMQ endpoint (e.g., `tcp://*:5555`)           |
| `--input-zmq-bind`     | `true` (bind) or `false` (connect)               |
| `--output-type`        | `"stdout"` or `"zmq_push"`                       |
| `--output-zmq-socket`  | ZeroMQ endpoint (e.g., `tcp://localhost:5555`)   |
| `--output-zmq-bind`    | `true` (bind) or `false` (connect)               |
| `--log-enabled`        | Enable or disable logging                        |
| `--log-level`          | Log verbosity: `error`, `warn`, `info`, etc.     |
| `--log-file`           | Log file path                                    |
| `--log-append`         | Append to log file (`true`) or overwrite (`false`)|
| `--xor-enabled`        | Enable XOR processing                            |
| `--xor-key`            | Hexadecimal XOR key                              |
| `--xor-pad`            | Optional 1-byte hex pad                          |
| `--base64-enabled`     | Enable Base64 processing                         |
| `--base64-mode`        | `"encode"` or `"decode"`                         |
| `--base64-padding`     | Include (`true`) or omit (`false`) padding       |
| `--config`             | Path to JSON config file                         |

See `./target/release/byteproc --help` for the full list.

---

## Testing

Run all tests:

```bash
cargo test
```

---

## License
See [LICENSE.txt](LICENSE.txt) for details.

---