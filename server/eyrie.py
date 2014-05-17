#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
__author__ = 'terrence'

import mcp
import mcp.network as network
from mcp.abode import Abode
from mcp.actuators.hue import HueBridge, HueLight
from mcp.devices import DeviceSet
from mcp.environment import Environment
from mcp.filesystem import FileSystem, Directory, File
from mcp.sensors.nerve import Nerve, NerveEvent
from mcp.sensors.listener import Listener, ListenerEvent
from mcp.dimension import Coord, Size
import mcp.fs_reflector as reflector

from eyrie_controller import EyrieController

from apscheduler.scheduler import Scheduler
import llfuse

import os
import os.path
import subprocess
import sys


house = \
"""
Horizontal: 4/ft
Vert: 2/ft
Reference is upper-left corner.
X is left-to-right.
Y is top-to-bottom.
Z is floor-to-ceiling.

----------------------------------------------------------------------------------------------------+
|                                        |        .            *                                   |
|                                        |        .                                                |
|                                        |        .                                                |
|                                        |        .                                                |
|                                        |        .                                                |
|                                        |        .                                                |
|                                        |        .                                                |
|                                        |        .                                                |
|         Office                         |        .                 Bedroom                        |
|            10ftx13ft                   |________.                    12ftx10ft                   |
|                                        .        |                                                |
|                                        .        |                                                |
|                                        .        |                                                |
|                                        .        |                                                |
|                                        .        |                                                |
|                                        .        |                                                |
|                                        .        |                                                |
|                                        .        |                                                |
|                                        +________+@@@@@@@@@@--------------------------------------+
|                                        @                         @                               |
|                                        @        Hall             @                               |
|                                        @          76" x 31"      @                               |
|                                        @                         @                               |
|@@@@@@-------------------------------------------+          +-----+                               |
@                                                 @          @     |                               |
@                                                 @          @     |                               |
@  Entry                                          @          @     |                               |
@    42" x 42"                                    @          @     |                               |
@                                                 @          @     |                               |
+--------------                                   +          +-----+-------------------------------+
|                                                 |                                                |
|                                                 |                                                |
|                                                 |                                                |
|                                                 |                                                |
|                                                 |                                                |
|                                                 |                                                |
|     Living Room                                 |                                                |
|        13' x 19'9"                              |                                                |
|                                                 |                                                |
|                                                 |                                                |
|                                                 |                                                |
|                                                 |                                                |
|                                                 |                                                |
|                                                 |                                                |
|                                                 |                                                |
|                                                 |                                                |
|                                                 |                                                |
|                                                 |                                                |
|                                                 +----------------@@@@@@@@@@@@---+@@@@@@@@@@------+
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
|                                                                                 |                |
+-------------------------------------@@@@@@@@@@@@--------------------------------+----------------+
"""


def build_abode(filesystem: FileSystem, environment: Environment):
    abode = Abode("eyrie")
    office = abode.create_room('office', Coord(0, 0), Size('10ft', '13ft', '8ft'))
    bedroom = abode.create_room('bedroom', Coord('13ft', 0), Size('12ft', '10ft', '8ft'))
    livingroom = abode.create_room('livingroom', Coord(0, '13ft'), Size('13ft', '19ft9in', '8ft'))
    entry = livingroom.create_subarea('entry', Coord(0, 0), Size('42in', '42in', '8ft'))

    # Create a directory structure out of abode.
    directories = reflector.map_abode_to_filesystem(abode, filesystem)

    # Add sensor nodes to the filesystem -- we cannot auto-detect their presence, since these will only get put
    # in the abode when we start receiving sensor data.
    for area in (office, bedroom, livingroom):
        reflector.add_rw_abode_properties(directories[area], area, ('temperature', 'humidity', 'motion'))

    # Show our environment data on the top-level abode node.
    reflector.add_ro_object_properties(directories[abode], environment,
                                       ('sunrise_twilight', 'sunrise', 'sunset', 'sunset_twilight'))

    return abode


def add_devices(abode: Abode, bus: network.Bus, controller: EyrieController, filesystem: FileSystem):
    devices = DeviceSet()

    for name, path in [('nerve-bedroom-north', '/eyrie/bedroom'),
                       ('nerve-office-north', '/eyrie/office'),
                       ('nerve-livingroom-south', '/eyrie/livingroom')]:
        log.info("Building nerve: {} at {}".format(name, path))
        nerve = Nerve(name, (name, network.Bus.DefaultSensorPort))

        def property_forwarder(path: str, propname: str):
            def handler(evt: NerveEvent):
                log.info("Forwarding message to: {}[{}]".format(path, propname))
                abode.lookup(path).set(propname, evt.value)
            return handler

        nerve.listen_temperature(property_forwarder(path, 'temperature'))
        nerve.listen_humidity(property_forwarder(path, 'humidity'))
        nerve.listen_motion(property_forwarder(path, 'motion'))
        bus.add_sensor(nerve.remote)
        devices.add(nerve)

    huebridge = HueBridge('hue-bedroom', 'MasterControlProgram')
    devices.add(HueLight('hue-bedroom-bed', huebridge, 1))
    devices.add(HueLight('hue-bedroom-desk', huebridge, 6))
    devices.add(HueLight('hue-bedroom-dresser', huebridge, 7))
    devices.add(HueLight('hue-bedroom-torch', huebridge, 3))
    devices.add(HueLight('hue-office-ceiling1', huebridge, 4))
    devices.add(HueLight('hue-office-ceiling2', huebridge, 5))
    devices.add(HueLight('hue-livingroom-torch', huebridge, 2))

    # Insert controllable devices into the filesystem and device list.
    directory = filesystem.root().add_entry("actuators", Directory())
    for light in devices.select("$hue"):
        reflector.add_hue_light(directory, light)

    # Add listeners.
    for (name, machine) in [('listener-bedroom-chimp', 'lemur')]:
        def command_forwarder(controller: EyrieController):
            def on_command(event: ListenerEvent):
                log.warning("Received command: {}".format(event.command))
                controller.apply_preset(event.command.lower(), "*")
            return on_command
        listener = Listener(name, (machine, network.Bus.DefaultSensorPort))
        listener.listen_for_commands(command_forwarder(controller))
        bus.add_sensor(listener.remote)
        devices.add(listener)

    return devices


def add_data_recorders(abode: Abode, args):
    def make_recorder(room_name, input_name):
        database_file = os.path.join(args.db_path, room_name + '-' + input_name + '.rrd')

        def recorder(event):
            assert event.property_name == input_name
            log.info("Recording {} for {} - {}".format(input_name, room_name, int(event.property_value)))
            subprocess.check_output(["rrdtool", "update", database_file, "--",
                                     "N:{}".format(int(event.property_value))])
        return recorder

    for room in ('bedroom', 'office', 'livingroom'):
        for input in ('temperature', 'humidity', 'motion'):
            abode.lookup('/eyrie/' + room).listen(input, 'propertyTouched', make_recorder(room, input))


def main():
    global log
    log = mcp.enable_logging(level='DEBUG')

    import argparse
    parser = argparse.ArgumentParser(description='Master Control Program')
    parser.add_argument('--db-path', default=os.path.expanduser("~/.local/var/db/mcp/"),
                        help='Where to store our data.')
    args = parser.parse_args()

    # The controller has to come first so that it can initialize the alarms that the scheduler
    # is going to be looking for in the global scope when we create it. We also want to be
    # able to forward device events to it instead of the abode in some cases.
    controller = EyrieController()
    controller.build_alarms()

    # Platform services.
    scheduler = Scheduler({'apscheduler.jobstore.default.class': 'apscheduler.jobstores.shelve_store:ShelveJobStore',
                           'apscheduler.jobstore.default.path': os.path.join(args.db_path, 'scheduled_jobs.db')})
    filesystem = FileSystem('/things')
    bus = network.Bus(llfuse.lock)
    environment = Environment()

    # Raw data.
    abode = build_abode(filesystem, environment)
    devices = add_devices(abode, bus, controller, filesystem)

    # Side channel data recording.
    add_data_recorders(abode, args)

    # Finally, really initialize our controller.
    controller.init(abode, devices, filesystem, bus, scheduler)

    bus.start()
    scheduler.start()

    filesystem.run()

    scheduler.shutdown()
    bus.exit()
    bus.join()
    return 0


if __name__ == '__main__':
    sys.exit(main())
