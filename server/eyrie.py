__author__ = 'terrence'

import mcp
import mcp.network as network
from mcp.abode import Abode
from mcp.dimension import Coord, Size

import sys


"""
Horizontal: 4/ft
Vert: 2/ft
Reference is upper-left corner.
X is left-to-right.
Y is top-to-bottom.
Z is floor-to-ceiling.

-----------------------------------------------------------------------------------------------+
|                                        |    .            *                                   |
|                                        |    .                                                |
|                                        |    .                                                |
|                                        |    .                                                |
|                                        |    .                                                |
|                                        |    .                                                |
|                                        |    .                                                |
|                                        |    .                                                |
|                                        |    .                                                |
|                         10ftx13ft      |____.                              12ftx10ft         |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        +____+@@@@@@@@@@--------------------------------------+
|                                        @                   @
|                                        @                   @
|                                        @                   @
|                                        @                   @
|@@@@@@----------------------------------+               +---+
@                                        @               @   |
@                                        @               @   |
@                                        @               @   |
@                                        @               @   |
@                                        @               @   |
+------+                                 +               +---+
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
+
"""
def build_abode():
    abode = Abode("Eyrie")
    bedroom = abode.create_room('bedroom', Coord('13ft', 0), Size('12ft', '10ft', '8ft'))
    devices = {}
    return abode, devices


def main():
    log = mcp.enable_logging()

    abode, devices = build_abode()

    bus = network.Bus()
    bus.start()

    bus.exit()
    bus.join()
    return 0


if __name__ == '__main__':
    sys.exit(main())