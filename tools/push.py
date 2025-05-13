#!/usr/bin/env python3
"""
push.py — sends a series of text messages over a PUSH socket. Assumes push is the server and pull is the client.
"""

import time
import zmq
import argparse

def main():
    parser = argparse.ArgumentParser(description="PUSH/PULL demo")
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
    # Create a PUSH socket and bind to TCP port 5555
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
        socket.send_string(msg)
        time.sleep(1)  # pause so you can observe the flow

    socket.close()
    context.term()

if __name__ == "__main__":
    main()

