#!/usr/bin/python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.

from arduino import Arduino
import argparse
import datetime
import struct
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
            if op == 'GENERIC':
                r, g, b = [json[i] for i in "rgb"]
                t = int(round(json['t'] * 1000))
                print("{} Color: ({} {} {}) in {}".format(datetime.datetime.now(), r, g, b, t))
                msg = struct.pack('>HHHH', r << 8, g << 8, b << 8, t)
                arduino.write(msg)
            elif op == 'TEST':
                print("Hello world!")
            else:
                print("UNKNOWN MESSAGE: {}".format(op))
    except KeyboardInterrupt:
        arduino.close()

    return 0

if __name__ == '__main__':
    sys.exit(main())

