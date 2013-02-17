#!/usr/bin/python3
from arduino import Arduino
import argparse
import socket
import sys
import zmq

DefaultControllerHost = 'gorilla'
DefaultServoPort = 31978


def main():
    parser = argparse.ArgumentParser(description='Control a LED lightstrip attached to an Arduino.')
    parser.add_argument('name', metavar='NAME', type=str, help='The name of this LightStrip')
    parser.add_argument('--tty', metavar='TTY', type=str, default='/dev/tty???*',
                        help='The TTY the arduino is connected on.')
    parser.add_argument('--host', metavar='NAME', type=str, default=DefaultControllerHost,
                        help='Which controller to connect to.')
    args = parser.parse_args()

    arduino = Arduino(args.tty, 9600)
    arduino.write(b'0')

    ctx = zmq.Context()
    sock = ctx.socket(zmq.SUB)
    addr = "tcp://" + args.host + ":" + str(DefaultServoPort)
    print("Connecting to: {}".format(addr))
    sock.connect(addr)
    sock.setsockopt(zmq.SUBSCRIBE, b'')

    try:
        while True:
            json = sock.recv_json()
            assert json['name'] == args.name
            op = json['type']
            if op == 'ON':
                arduino.write(b'i')
                print("TURN ON")
            elif op == 'OFF':
                arduino.write(b'o')
                print("TURN OFF")
            else:
                print("UNKNOWN MESSAGE: {}".format(op))
    except KeyboardInterrupt:
        arduino.close()

    return 0

if __name__ == '__main__':
    sys.exit(main())

