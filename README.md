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

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup install stable
rustup default stable
```

---

## Building

Clone and build the project:

```
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

```
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

```
echo "deadbeef" | ./target/release/byteproc
```

### XOR Module Example

Enable XOR processing with a key:

```
echo "00112233" | ./target/release/byteproc \
  --xor-enabled \
  --xor-key abcd1234
```

### Custom Logging

Specify log level, log file location, and append mode:

```
echo "48656c6c6f20776f726c64" | ./target/release/byteproc \
  --log-enabled true \
  --log-level debug \
  --log-file ./logs/byteproc-$(date +%Y%m%d).log \
  --log-append
```

**Log levels:** `error`, `warn`, `info`, `debug`, `trace`

### ZeroMQ Integration

Byteproc operates as a **single-shot processor** when using ZeroMQ - each instance processes exactly one message and then exits. This is important to understand when setting up ZMQ communication.

#### Basic ZeroMQ Setup

##### Terminal A (Sink / PULL)
```bash
# This instance will wait for ONE message, process it, then exit
./target/release/byteproc \
  --input-type zmq_pull \
  --input-zmq-socket tcp://*:5555 \
  --input-zmq-bind \
  --zmq-receive-timeout-ms 10000  # timeout to 10 seconds
```

##### Terminal B (Source / PUSH)
```bash
# Send a single message
echo "cafebabe" | ./target/release/byteproc \
  --output-type zmq_push \
  --output-zmq-socket tcp://localhost:5555
```

#### Understanding Bind vs. Connect

The `--input-zmq-bind` and `--output-zmq-bind` flags control how the ZeroMQ socket is initialized:

- **Bind**: Socket acts like a "server" - it opens a port and waits for connections
- **Connect (no argument)**: Socket acts like a "client" - it reaches out to connect to a bound socket

##### When to Use Bind:

1. **Stable Endpoints**: Use bind for stable, long-lived services that other nodes need to find
2. **Public-Facing Services**: Bind when your service needs to be discoverable by many clients
3. **Socket Ownership**: The process that creates/owns the address should bind to it
4. **Central Nodes**: In star topologies, the central node usually binds

##### When to Use Connect:

1. **Ephemeral Processes**: Short-lived processes should connect to stable endpoints
2. **Multiple Senders**: When multiple processes send to a single receiver
3. **Dynamic Endpoints**: When the number of endpoints can change at runtime
4. **Outbound Requests**: When initiating a request to an existing service

##### Common Patterns:

1. **Server-Client**: Server binds, clients connect
   ```
   Server: --input-zmq-bind
   Client: # No --output-zmq-bind
   ```

2. **Pipeline**: Receivers bind, senders connect
   ```
   Receiver: --input-zmq-bind
   Sender: # No --output-zmq-bind
   ```

3. **Multi-stage Pipeline**:
   ```
   Stage 3: --input-zmq-bind true
   Stage 2: --input-zmq-bind true --output-zmq-bind false
   Stage 1: --output-zmq-bind false
   ```

#### Important Timing Considerations

1. **Start the PULL instance first** - The receiving side must be ready before sending any messages.
2. **Send within the timeout period** - By default, the PULL socket times out after 5 seconds if no message arrives.
3. **One message per instance** - Each byteproc instance only processes a single message before exiting.

#### Avoiding Timeouts

To avoid the "Resource temporarily unavailable" error:

1. **Increase the timeout**:
   ```bash
   --zmq-receive-timeout-ms 30000  # Set to 30 seconds
   ```

2. **Use unlimited timeout** (wait forever):
   ```bash
   --zmq-receive-timeout-ms -1  # Wait indefinitely
   ```

#### Continuous Processing with ZMQ

For continuous processing, use a shell loop:

```bash
# Terminal A: Continuous receiver
while true; do
  ./target/release/byteproc \
    --input-type zmq_pull \
    --input-zmq-socket tcp://*:5555 \
    --input-zmq-bind
  echo "Waiting for next message..."
  sleep 0.1  # Small delay between instances
done
```

#### Flow Control

When chaining multiple byteproc instances:

1. **Start all receivers first** - Work backwards from the final consumer.
2. **Start senders last** - Send data only after all receivers are ready.
3. **Add delays between messages** - Allow time for processing between messages.

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

```
cargo test
```

## Debugging tips

To capture all traffic on all interfaces and save it to a pcapng file using tcpdump, use the following command:

Explanation:

sudo: Required for packet capture privileges
tcpdump: The packet capture utility
-i any: Tells tcpdump to listen on all interfaces
-w capture.pcapng: Writes raw packets to the specified file in pcapng format
For additional useful options:

Add -n to avoid DNS lookups (faster capture)
Add -s 0 to capture full packets (not just headers)
Add -B 4096 to increase buffer size for busy networks
Full example with these options:

`sudo tcpdump -i any -n -s 0 -B 4096 -w capture.pcapng`

Press Ctrl+C to stop the capture when you're done.


---

## License
See [LICENSE.txt](LICENSE.txt) for details.

---