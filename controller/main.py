#!/usr/bin/python3
import argparse
import contextlib
import curses
import locale
import logging
import socket
import sys
import threading
import time
import zmq

# The next two need to be ordered
import readline
import cmd

from pprint import pprint

from network import Network


@contextlib.contextmanager
def enter_console():
    locale.setlocale(locale.LC_ALL, '')
    stdscr = curses.initscr()
    stdscr.keypad(1)
    curses.noecho()
    curses.cbreak()
    curses.start_color()
    try:
        yield stdscr
    finally:
        stdscr.keypad(0)
        curses.nocbreak()
        curses.echo()
        curses.endwin()

def tui_mode(sock):
    with enter_console() as stdscr:
        height, width = stdscr.getmaxyx()
        halfh = height // 2
        halfw = width // 2
        winraw = curses.newwin(halfh, halfw, height - halfh, 0)
        winreal = curses.newwin(halfh, width - halfw, height - halfh, width - halfw)

        stdscr.nodelay(1)
        curses.curs_set(0)
        while curses.ERR == stdscr.getch():
            # Get current status.
            sock.send_json({'name': 'status'})
            status = sock.recv_json()
            if 'error' in status:
                return status['traceback'] + '\n' + status['error']

            # Put raw users on raw window.
            h = 0
            for sensorName, users in status['sensorUsers'].items():
                winraw.addstr(h, 0, sensorName)
                h += 1
                for uid, pos in users.items():
                    winraw.addstr(h, 4, '{:5}: {}'.format(uid, pos))
                    h += 1
            winraw.refresh()

            # Put transformed users.
            h = 0
            for uid, data in status['realUsers'].items():
                winreal.addstr(h, 0, 'User-{}'.format(uid))
                h += 1
                rooms = []
                for trackname, trackdata in data['tracks'].items():
                    winreal.addstr(h, 4, '{}: {}'.format(trackname, trackdata['position']))
                    rooms.append(trackdata['room'])
                    h += 1
                winreal.addstr(h, 8, 'Rooms: ' + ', '.join(rooms))
                winreal.addstr(h+1, 8, 'Zones: ' + ', '.join(data['zones']))
                h += 2
            winreal.refresh()

            # Sleep a bit.
            stdscr.refresh()
            time.sleep(0.1)

    return 'TUI FINISHED'

class CommandLoop(cmd.Cmd):
    prompt = '> '

    def __init__(self):
        super().__init__()
        self.ctx = zmq.Context()
        self.ctl = self.ctx.socket(zmq.REQ)
        self.ctl.connect("tcp://localhost:{}".format(Network.DefaultControlPort))

    def do_exit(self, line):
        self.ctl.send_json({'name': 'exit'})
        self.ctl.recv_json()
        print("END OF TRANSMISSION")
        return True

    def do_EOF(self, line):
        return self.do_exit(line)

    def do_status(self, line):
        self.ctl.send_json({'name': 'status'})
        status = self.ctl.recv_json()
        pprint(status)

    def do_tui(self, line):
        rv = tui_mode(self.ctl)
        print(rv)


def input_loop():
    """Runs in thread 0 to control the system using the local terminal."""
    cmdloop = CommandLoop()
    cmdloop.cmdloop('LOCAL CONTROL ACTIVE END OF LINE')

def mcp_loop():
    """Runs in thread 1 to drive the house based on network events."""
    from instances.trainedmonkeystudios import build_floorplan
    floorplan = build_floorplan()
    network = Network(floorplan)
    return network.run()

def main():
    parser = argparse.ArgumentParser(description='I AM THE MASTER CONTROL PROGRAM END OF LINE\n' +
                                                 'YOU WILL BE FOUND AND YOUR NEEDS FULLFILLED END OF LINE\n')
    parser.add_argument('-V', '--loglevel', dest='loglevel', type=str, default='DEBUG',
                        help='Set the log level (default:DEBUG).')
    args = parser.parse_args()

    global log
    loglevel = getattr(logging, args.loglevel.upper())
    logging.basicConfig(format='%(asctime)s:%(levelname)s:%(name)s:%(message)s',
                        filename='eventlog.log', level=loglevel)
    log = logging.getLogger('home')

    mcp = threading.Thread(name='MCP', target=mcp_loop)
    mcp.start()

    ctl = threading.Thread(name='Control', target=input_loop)
    ctl.start()

    mcp.join()
    ctl.join()

if __name__ == '__main__':
    sys.exit(main())
