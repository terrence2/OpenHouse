#!/usr/bin/python3
from arduino import Arduino
import argparse
import cmd
import struct
import sys

class Interp(cmd.Cmd):
    prompt = '> '

    def __init__(self, arduino, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.arduino = arduino

    def postcmd(self, stop, line):
        return line == 'EOF'

    def do_EOF(self, line):
        "Exit the program."
        print()

    def do_color(self, line):
        """
        Input is: r g b time
        With rgb in int [0,255] and time in float seconds.
        """
        nums = line.split()
        r, g, b = [int(c) for c in nums[:3]]
        t = float(nums[3])
        tMS = int(round(t * 1000))
        tHi = tMS >> 8
        tLo = tMS & 0xFF
        for name, byte in [('r', r), ('g', g), ('b', b), ('tHi', tHi), ('tLo', tLo)]:
            if byte < 0 or byte > 255:
                print("Invalid byte supplied for: " + name)
                return
        data = struct.pack('!BBBBH', ord('G'), r, g, b, tMS)
        print(data)
        self.arduino.write(data)

CMDS = {
    'on': '1',
    'off': '0',
    'fadein': 'i',
    'fadeout': 'o',
    'red': 'r',
    'green': 'g',
    'blue': 'b'
}
for name, cmd in CMDS.items():
    setattr(Interp, 'do_' + name, lambda self, line, cmd=cmd: self.arduino.write(cmd.encode('ASCII')))

def main():
    parser = argparse.ArgumentParser(description='Control a LED lightstrip attached to an Arduino.')
    parser.add_argument('--tty', metavar='TTY', type=str, default='/dev/tty???*',
                        help='The TTY the arduino is connected on.')
    args = parser.parse_args()

    arduino = Arduino(args.tty, 9600)
    Interp(arduino).cmdloop("Interact with the arduino:")
    arduino.close()

    return 0

if __name__ == '__main__':
    sys.exit(main())
