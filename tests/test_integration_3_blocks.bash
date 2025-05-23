#!/bin/bash
set -e

# Allow override of the byteproc executable path via environment variable
# Default to debug build if not specified
BYTEPROC_BIN="${BYTEPROC_BIN:-./target/debug/byteproc}"
echo "Using byteproc binary: $BYTEPROC_BIN"

# Set up log files
LOG_DIR="$(pwd)/tests/zmq_test_logs"
mkdir -p "$LOG_DIR"

# Test parameters
TEST_DATA="deadbeefcafebabe"
PORT1="5551"
PORT2="5552"

# Clean up prior test processes if any
pkill -f "byteproc.*zmq" || true
sleep 1
echo "running from $(pwd)"
echo "Cleaning up old logs..."
rm -f "$LOG_DIR"/*.log
rm -f "$LOG_DIR"/*.txt
echo "Starting test with data: $TEST_DATA"
echo "Ports: $PORT1 -> $PORT2"
echo "Log directory: $LOG_DIR"

# Start the chain from the end
echo "Starting final receiver..."
"$BYTEPROC_BIN" \
  --input-type zmq_pull \
  --input-zmq-socket "tcp://*:$PORT2" \
  --input-zmq-bind \
  --log-file "$LOG_DIR/receiver.log" \
  --zmq-receive-timeout-ms 5000 \
  > "$LOG_DIR/output.txt" &
RECEIVER_PID=$!
sleep 1

echo "Starting middle processor..."
"$BYTEPROC_BIN" \
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
echo "$TEST_DATA" | "$BYTEPROC_BIN" \
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