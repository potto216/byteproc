https://www.rust-lang.org/tools/install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

rustup install stable
rustup default stable

Create a New Cargo Project

```
cargo new byteproc --bin
cd byteproc
```
Build the Project
```
cargo build --release
```
Run the Binary

Using stdin/stdout:
```
echo "deadbeef" | ./target/release/byteproc
```
With XOR module:
```
echo "00112233" | ./target/release/byteproc \
  --xor-enabled true \
  --xor-mode encrypt \
  --xor-key abcd1234
```

Using ZeroMQ (in separate terminals):

# Terminal A (sink)

```
./target/release/byteproc \
  --input-type zmq_pull \
  --input-zmq-socket tcp://*:5555 \
  --input-zmq-bind true
```
# Terminal B (source)

```
echo "cafebabe" | ./target/release/byteproc \
  --output-type zmq_push \
  --output-zmq-socket tcp://localhost:5555 \
  --output-zmq-bind false
```