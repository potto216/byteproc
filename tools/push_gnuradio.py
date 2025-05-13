#!/usr/bin/python3
"""
push_gnuradio.py — sends a series of text messages over a PUSH socket,
packaged as GNU Radio PMT byte arrays for use with GNU Radio's ZMQ Pull message source.
Assumes the python version that gnuradio uses is in /usr/bin/python3
"""

import time
import zmq
import argparse
import pmt

def main():
    parser = argparse.ArgumentParser(description="PUSH/PULL demo for GNU Radio")
    parser.add_argument(
        '--mode',
        choices=['bind','connect'],
        default='bind',
        help='Socket mode: bind (default) or connect'
    )
    parser.add_argument(
        '--port',
        default='5555',
        help='TCP port to use (default: 5555)'
    )
    args = parser.parse_args()

    mode = args.mode
    port = args.port
    print(f"Mode: {mode}, Port: {port}")

    context = zmq.Context()
    socket = context.socket(zmq.PUSH)
    if mode == 'bind':
        socket.bind(f"tcp://127.0.0.1:{port}")
    else:
        socket.connect(f"tcp://127.0.0.1:{port}")

    messages = [
        "Hello, world!",
        "This is a PUSH/PULL demo.",
        "Goodbye!"
    ]

    for msg in messages:
        print(f"Sending: {msg}")
        # Encode the string to raw bytes
        raw = msg.encode('utf-8')
        # Build a PMT u8vector (uint8_t array) from those bytes
        pmt_msg = pmt.init_u8vector(len(raw), list(raw))
        # Serialize the PMT object for transport over ZMQ
        packed = pmt.serialize_str(pmt_msg)
        socket.send(packed)
        time.sleep(1)  # pause so you can observe the flow

    socket.close()
    context.term()

if __name__ == "__main__":
    main()

