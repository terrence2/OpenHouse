#!/usr/bin/env python3
__author__ = 'terrence'

import mcp
import mcp.network as network
from mcp.abode import Abode
from mcp.actuators.hue import HueBridge, HueLight
from mcp.filesystem import FileSystem
from mcp.sensors.nerve import Nerve
from mcp.dimension import Coord, Size
import mcp.fs_reflector as reflector

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


def add_devices(abode: Abode, bus: network.Bus):
    bedroom_nerve = Nerve('bedroom_nerve', ('rpi-nerve-bedroom', network.Bus.DefaultSensorPort))
    bedroom_nerve.listen_temperature(lambda val: abode.lookup('/eyrie/bedroom').set('temperature', val))
    bedroom_nerve.listen_humidity(lambda val: abode.lookup('/eyrie/bedroom').set('humidity', val))
    bedroom_nerve.listen_motion(lambda val: abode.lookup('/eyrie/bedroom').set('motion', val))
    bus.add_sensor(bedroom_nerve)

    bedroom_huebridge = HueBridge('hue-bedroom', 'MasterControlProgram')
    bed_hue = HueLight('hue-bedroom-bed', bedroom_huebridge, 1)
    desk_hue = HueLight('hue-bedroom-desk', bedroom_huebridge, 2)
    dresser_hue = HueLight('hue-bedroom-dresser', bedroom_huebridge, 3)
    return [bedroom_nerve, bed_hue, desk_hue, dresser_hue]


def add_reactions(abode):
    abode.lookup('/eyrie/bedroom').listen('temperature', 'propertyTouched', lambda event: print("TEMP:", event.property_value))
    #DatabaseLocation = "/storage/raid/data/var/db/mcp/{}.rrd"
    #subprocess.check_output(["rrdtool", "update", self.database_filename, "--",
    #                         "N:{}:{}".format(self.last_temperature, self.last_humidity)])


def main():
    log = mcp.enable_logging(level='DEBUG')
    filesystem = FileSystem('/things')
    bus = network.Bus()
    abode = build_abode()
    devices = add_devices(abode, bus)
    reflector.map_abode_to_filesystem(abode, filesystem)
    reflector.map_devices_to_filesystem(devices, filesystem)

    bus.start()

    filesystem.run()

    bus.exit()
    bus.join()
    return 0


if __name__ == '__main__':
    sys.exit(main())