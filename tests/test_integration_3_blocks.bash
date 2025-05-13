#!/bin/bash
set -e

# Set up log files
LOG_DIR="$(pwd)/zmq_test_logs"
mkdir -p "$LOG_DIR"

# Test parameters
TEST_DATA="deadbeefcafebabe"
PORT1="5551"
PORT2="5552"

# Clean up prior test processes if any
pkill -f "byteproc.*zmq" || true
sleep 1

# Start the chain from the end
echo "Starting final receiver..."
../target/debug/byteproc \
  --input-type zmq_pull \
  --input-zmq-socket "tcp://*:$PORT2" \
  --input-zmq-bind \
  --log-file "$LOG_DIR/receiver.log" \
  --zmq-receive-timeout-ms 5000 \
  > "$LOG_DIR/output.txt" &
RECEIVER_PID=$!
sleep 1

echo "Starting middle processor..."
../target/debug/byteproc \
  --input-type zmq_pull \
  --input-zmq-socket "tcp://*:$PORT1" \
  --input-zmq-bind \
  --output-type zmq_push \
  --output-zmq-socket "tcp://localhost:$PORT2" \
  --log-file "$LOG_DIR/middle.log" \
  --zmq-receive-timeout-ms 5000 &
MIDDLE_PID=$!
sleep 1

echo "Sending test data..."
echo "$TEST_DATA" | ../target/debug/byteproc \
  --output-type zmq_push \
  --output-zmq-socket "tcp://localhost:$PORT1" \
  --log-file "$LOG_DIR/sender.log"

# Wait for processing to complete
wait $RECEIVER_PID || true
wait $MIDDLE_PID || true

# Check the result
RESULT=$(cat "$LOG_DIR/output.txt")
if [ "$RESULT" = "$TEST_DATA" ]; then
  echo "TEST PASSED: Input matches output"
  echo "[$TEST_DATA] == [$RESULT]"
  exit 0
else
  echo "TEST FAILED: Input does not match output"
  echo "Expected: [$TEST_DATA]"
  echo "Got:      [$RESULT]"
  exit 1
fi