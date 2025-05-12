#!/usr/bin/env python3
"""
push.py — sends a series of text messages over a PUSH socket.
"""

import time
import zmq

def main():
    context = zmq.Context()
    # Create a PUSH socket and bind to TCP port 5555
    socket = context.socket(zmq.PUSH)
    socket.bind("tcp://127.0.0.1:5555")

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

