# Python ZMQ Tools

This directory contains simple test programs for verifying ZeroMQ (ZMQ) PUSH/PULL socket communication in Python.

## Setup Python Environment

Ensure you have [pyenv](https://github.com/pyenv/pyenv) installed. Then run:

```sh
cd tools
pyenv update
pyenv install 3.12.10
pyenv local 3.12.10
python -m venv .venv
source .venv/bin/activate
pip install --upgrade pip
pip install -r requirements.txt
```

## Testing PUSH/PULL Communication

### Default Behavior

By default:
- `push.py` acts as a **server** (binds to port 5555).
- `pull.py` acts as a **client** (connects to port 5555).

#### 1. Start the PUSH server

```sh
python push.py
```

#### 2. In another terminal, start the PULL client

```sh
python pull.py
```

You should see messages sent from `push.py` and received by `pull.py`.

### Customizing Mode and Port

Both scripts accept `--mode` (`bind` or `connect`) and `--port` (default: `5555`):

- `push.py` defaults to `--mode bind`
- `pull.py` defaults to `--mode connect`

Example: Run `pull.py` as a server and `push.py` as a client on port 6000:

```sh
python pull.py --mode bind --port 6000
python push.py --mode connect --port 6000
```

## Testing with `byteproc`

You can also test ZMQ integration with the Rust `byteproc` tool:

```sh
# Start pull.py as a server
python ./tools/pull.py --mode bind

# In another terminal, send data using byteproc
echo "cafebabe" | ./target/release/byteproc --output-type zmq_push --output-zmq-socket tcp://localhost:5555

# You should see output from pull.py
```

## Testing with GNU Radio

First run the GNU Radio script zmq_push_pull_test.grc

Run 
```
python ./tools/pull.py --mode connect --port 5556 --binary
```
To wait for the messages from GNU Radio

Now set the execute flag on the push script to interact with gnuradio. This assumes the python path it references what GNU Radio uses
```
chmod +x ./tools/push_gnuradio.py
```
Now send the messages
```
./tools/push_gnuradio.py --mode bind --port 5555
```

---
**Tip:** Use `--help` with either script to see all available options:

```sh
python push.py --help
python pull.py --help
```