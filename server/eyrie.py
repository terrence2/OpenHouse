__author__ = 'terrence'

import mcp
import mcp.network as network
from mcp.abode import Abode
from mcp.sensors.nerve import Nerve
from mcp.dimension import Coord, Size

import sys
import time


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
    abode = Abode("eyrie")
    office = abode.create_room('office', Coord(0, 0), Size('10ft', '13ft', '8ft'))
    bedroom = abode.create_room('bedroom', Coord('13ft', 0), Size('12ft', '10ft', '8ft'))
    return abode


def add_devices(abode):
    bedroom_nerve = Nerve('bedroom_nerve', ('rpi-nerve-bedroom', network.Bus.DefaultSensorPort))
    bedroom_nerve.listen_temperature(lambda val: abode.lookup('/eyrie/bedroom').set('temperature', val))
    bedroom_nerve.listen_humidity(lambda val: abode.lookup('/eyrie/bedroom').set('humidity', val))
    bedroom_nerve.listen_motion(lambda val: abode.lookup('/eyrie/bedroom').set('motion', val))
    return [bedroom_nerve]


def add_reactions(abode):
    abode.lookup('/eyrie/bedroom').listen('temperature', 'propertyTouched', lambda event: print("TEMP:", event.property_value))
    #DatabaseLocation = "/storage/raid/data/var/db/mcp/{}.rrd"
    #subprocess.check_output(["rrdtool", "update", self.database_filename, "--",
    #                         "N:{}:{}".format(self.last_temperature, self.last_humidity)])

def main():
    log = mcp.enable_logging(level='INFO')

    abode = build_abode()
    devices = add_devices(abode)

    bus = network.Bus()
    for device in devices:
        bus.add_device(device)
    bus.start()

    while True:
        time.sleep(1)

    bus.exit()
    bus.join()
    return 0


if __name__ == '__main__':
    sys.exit(main())