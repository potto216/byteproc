#!/usr/bin/env python3
"""
pull.py — receives text messages from a PULL socket.
"""

import zmq

def main():
    context = zmq.Context()
    # Create a PULL socket and connect to the sender
    socket = context.socket(zmq.PULL)
    socket.connect("tcp://127.0.0.1:5555")

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

