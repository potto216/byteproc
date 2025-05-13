#!/usr/bin/env python3
"""
pull.py — receives text messages from a PULL socket.
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
    args = parser.parse_args()

    mode = args.mode
    port = args.port
    print(f"Mode: {mode}, Port: {port}")

    context = zmq.Context()

    # Create a PULL socket and connect to the sender
    socket = context.socket(zmq.PULL)
    if mode == 'connect':
        socket.connect(f"tcp://127.0.0.1:{port}")
    else:
        socket.bind(f"tcp://127.0.0.1:{port}")

    print("Waiting to receive messages...")
    try:
        while True:
            msg = socket.recv_string()
            print(f"Received: {msg}")
    except KeyboardInterrupt:
        print("\nInterrupted by user.")
    finally:
        socket.close()
        context.term()

if __name__ == "__main__":
    main()

