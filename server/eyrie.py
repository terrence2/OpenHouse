#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
__author__ = 'terrence'

import mcp
import mcp.network as network
from mcp.abode import Abode
from mcp.actuators.hue import HueBridge, HueLight
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
|                                        @                         @
|                                        @        Hall             @
|                                        @          76" x 31"      @
|                                        @                         @
|@@@@@@-------------------------------------------+          +-----+
@                                                 @          @     |
@                                                 @          @     |
@  Entry                                          @          @     |
@    42" x 42"                                    @          @     |
@                                                 @          @     |
+--------------                                   +          +-----+
|
|
|
|
|
|
|     Living Room
|        13' x 19'9"
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


def build_abode(filesystem: FileSystem):
    abode = Abode("eyrie")
    office = abode.create_room('office', Coord(0, 0), Size('10ft', '13ft', '8ft'))
    bedroom = abode.create_room('bedroom', Coord('13ft', 0), Size('12ft', '10ft', '8ft'))
    livingroom = abode.create_room('livingroom', Coord(0, '13ft'), Size('13ft', '19ft9in', '8ft'))
    entry = livingroom.create_subarea('entry', Coord(0, 0), Size('42in', '42in', '8ft'))

    directories = reflector.map_abode_to_filesystem(abode, filesystem)
    for area in (office, bedroom, livingroom):
        reflector.add_properties(directories[area], area, ('temperature', 'humidity', 'motion'))

    return abode


def add_devices(abode: Abode, bus: network.Bus, controller: EyrieController, filesystem: FileSystem):
    devices = {}

    for name in ['rpi-nerve-bedroom', 'rpi-nerve-office', 'rpi-nerve-livingroom']:
        path = name.replace('rpi-nerve-', '/eyrie/')
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
        devices[nerve.name] = nerve

    bedroom_huebridge = HueBridge('hue-bedroom', 'MasterControlProgram')
    bed_hue = HueLight('hue-bedroom-bed', bedroom_huebridge, 1)
    desk_hue = HueLight('hue-bedroom-desk', bedroom_huebridge, 2)
    dresser_hue = HueLight('hue-bedroom-dresser', bedroom_huebridge, 3)
    office_hue1 = HueLight('hue-office-ceiling1', bedroom_huebridge, 4)
    office_hue2 = HueLight('hue-office-ceiling2', bedroom_huebridge, 5)

    # Insert controllable devices into the filesystem and device list.
    directory = filesystem.root().add_entry("actuators", Directory())
    for light in (bed_hue, desk_hue, dresser_hue, office_hue1, office_hue2):
        reflector.add_hue_light(directory, light)
        devices[light.name] = light

    # Add listeners.
    for (name, machine) in [('listener-bedroom', 'lemur'), ('listener-avahi', 'avahi-wifi')]:
        def command_forwarder(controller: EyrieController):
            def on_command(event: ListenerEvent):
                log.warning("Received command: {}".format(event.command))
                controller.apply_preset(event.command.lower())
            return on_command
        listener = Listener(name, (machine, network.Bus.DefaultSensorPort))
        listener.listen_for_commands(command_forwarder(controller))
        bus.add_sensor(listener.remote)
        devices[listener.name] = listener

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

    # Raw data.
    abode = build_abode(filesystem)
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
