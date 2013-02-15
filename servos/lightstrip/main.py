#!/usr/bin/python3
import argparse
import glob
import serial
import socket
import sys
import time
import zmq

DefaultControllerHost = 'gorilla'
DefaultServoPort = 31978

class Arduino:
    def __init__(self, devglob, baud):
        self.devglob = devglob
        self.baud = baud
        self.tty = None
        self.tty = self.connect()

    def __del__(self):
        self.close()

    def close(self):
        if self.tty:
            self.tty.close()
            self.tty = None

    def find_device_names(self):
        i = 0
        while True:
            devices = glob.glob(self.devglob)
            if devices:
                return devices
            if i == 0:
                print("No devices like {} found: retrying every 10s".format(self.devglob))
            i += 1
            time.sleep(10)

    def connect(self):
        devnames = self.find_device_names()
        assert devnames
        tty = None
        for name in devnames:
            print("Trying to open arduino at: {}".format(name))
            tty = serial.Serial(name, self.baud)
            if not tty:
                continue

        if not tty:
            print("Waiting 10s for arduino to appear in /dev")
            time.sleep(10)
            return self.connect()

        print("Waiting 3s for arduino to reboot...")
        time.sleep(3)
        return tty

    def write(self, data:bytes):
        assert self.tty
        try:
            self.tty.write(data)
        except serial.SerialException as e:
            self.tty = self.connect()
            return self.write(data)


def main():
    parser = argparse.ArgumentParser(description='Control a LED lightstrip attached to an Arduino.')
    parser.add_argument('name', metavar='NAME', type=str, help='The name of this LightStrip')
    parser.add_argument('--tty', metavar='TTY', type=str, default='/dev/tty????', help='The TTY the arduino is connected on.')
    parser.add_argument('--host', metavar='NAME', type=str, default=DefaultControllerHost, help='Which controller to connect to.')
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

