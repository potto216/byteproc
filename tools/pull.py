#!/usr/bin/env python3
"""
pull.py — receives text or binary messages from a PULL socket.
"""

import zmq
import argparse

def main():
    parser = argparse.ArgumentParser(description="PUSH/PULL demo")
    parser.add_argument(
        '--mode',
        choices=['bind','connect'],
        default='connect',
        help='Socket mode: connect (default) or bind'
    )
    parser.add_argument(
        '--port',
        default='5555',
        help='TCP port to use (default: 5555)'
    )
    parser.add_argument(
        '--binary',
        action='store_true',
        help='Receive raw binary and print hex string'
    )
    args = parser.parse_args()

    mode = args.mode
    port = args.port
    binary = args.binary
    print(f"Mode: {mode}, Port: {port}, Binary: {binary}")

    context = zmq.Context()
    socket = context.socket(zmq.PULL)
    if mode == 'connect':
        socket.connect(f"tcp://127.0.0.1:{port}")
    else:
        socket.bind(f"tcp://127.0.0.1:{port}")

    print("Waiting to receive messages...")
    try:
        while True:
            if binary:
                data = socket.recv()               # raw bytes
                print(f"Received (hex): {data.hex()}")
            else:
                msg = socket.recv_string()         # text
                print(f"Received: {msg}")
    except KeyboardInterrupt:
        print("\nInterrupted by user.")
    finally:
        socket.close()
        context.term()

if __name__ == "__main__":
    main()

