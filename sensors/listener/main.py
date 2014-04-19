#!/usr/bin/env python2
from __future__ import print_function, division

import argparse
import zmq

from listener import CaptureSpokenCommands


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='MCP Command Listener')
    parser.add_argument('--capture-device', '-c', default='default',
                        help='ALSA capture device to open.')
    args = parser.parse_args()

    ctx = zmq.Context()
    sock = ctx.socket(zmq.PUB)
    sock.bind("tcp://*:31975")

    def on_command(command):
        print("DispatchedCommand: {}".format(command))
        sock.send_json({'command': command})

    commands = {
        'HEY EYRIE TURN ON THE LIGHTS': 'ON',
        'HEY EYRIE TURN THE LIGHTS ON': 'ON',
        'HEY EYRIE TURN OFF THE LIGHTS': 'OFF',
        'HEY EYRIE TURN THE LIGHTS OFF': 'OFF',
        'HEY EYRIE ITS SLEEP TIME': 'SLEEP',
        'HEY EYRIE ITS SLEEPY TIME': 'SLEEP',
        'HEY EYRIE ITS BED TIME': 'SLEEP',
        'HEY EYRIE ITS TIME FOR BED': 'SLEEP',
        'HEY EYRIE ITS TIME TO SLEEP': 'SLEEP',
        'HEY EYRIE TIME TO SLEEP': 'SLEEP',
        'HEY EYRIE LOWER THE LIGHTS': 'LOW',
    }
    listener = CaptureSpokenCommands(args.capture_device, "corpus-0/9629", ["HEY EYRIE", "EYRIE"],
                                     commands, on_command)
    listener.run()

