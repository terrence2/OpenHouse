#!/usr/bin/python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from arduino import Arduino
import argparse
import cmd
import struct
import sys


class Interp(cmd.Cmd):
    prompt = '> '

    def __init__(self, arduino, *args, **kwargs):
        cmd.Cmd.__init__(self, *args, **kwargs)
        self.arduino = arduino

    def postcmd(self, stop, line):
        return line == 'EOF'

    def do_EOF(self, line):
        "Exit the program."
        print()

    def do_color(self, line):
        """
        Input is:
            #rrggbb [time]
            rrr ggg bbb [time]

        With xx in hex [0,FF] and
        With xxx in int [0,255] and
        Time is in float seconds and is optional.
        """
        nums = [l.strip() for l in line.split()]

        if len(nums) == 0:
            return

        t = 0
        if nums[0].startswith('#'):
            clr = nums[0][1:]
            assert len(clr) == 6
            r = int(clr[0:2], 16)
            g = int(clr[2:4], 16)
            b = int(clr[4:6], 16)
            if len(nums) > 1:
                t = float(nums[1])

        elif len(nums) > 2:
            r, g, b = [int(c) for c in nums[:3]]
            if len(nums) > 3:
                t = float(nums[3])

        tMS = int(round(t * 1000))
        r <<= 8
        g <<= 8
        b <<= 8
        data = struct.pack('>HHHH', r, g, b, tMS)

        self.arduino.write(data)

    def do_c(self, line):
        self.do_color(line)

    def do_sleep(self, line):
        self.do_color("#000001")

    def do_work(self, line):
        self.do_color("#0061cf")

    def do_out(self, line):
        self.do_color("#000055")

    def do_on(self, line):
        self.do_color("#FFFFFF")

    def do_off(self, line):
        self.do_color("#000000")

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
