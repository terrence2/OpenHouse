#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from mcp.abode import Abode
from mcp.environment import Environment
from mcp.filesystem import FileSystem, Directory, File
import mcp
import mcp.fs_reflector as reflector
import mcp.network as network

from eyrie_controller import EyrieController

from apscheduler.scheduler import Scheduler
import llfuse

import logging
import os
import os.path
import subprocess
import sys


def add_data_recorders(abode: Abode, args):
    def make_recorder(room_name, input_name):
        database_file = os.path.join(args.db_path, room_name + '-' + input_name + '.rrd')

        def recorder(event):
            assert event.property_name == input_name
            log.debug("Recording {} for {} - {}".format(input_name, room_name, int(event.property_value)))
            subprocess.check_output(["rrdtool", "update", database_file, "--",
                                     "N:{}".format(int(event.property_value))])
        return recorder

    for room in ('bedroom', 'office', 'livingroom'):
        for input in ('temperature', 'humidity', 'motion'):
            abode.lookup('/eyrie/' + room).listen(input, 'propertyTouched', make_recorder(room, input))


def main():
    import argparse
    parser = argparse.ArgumentParser(description='Master Control Program')
    parser.add_argument('--db-path', default=os.path.expanduser("~/.local/var/db/mcp/"),
                        help='Where to store our data.')
    parser.add_argument('--log-level', '-l', default='INFO',
                        help="Set the log level (default: INFO).")
    args = parser.parse_args()

    global log
    mcp.enable_logging(level=args.log_level)
    log = logging.getLogger('eyrie')

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
    from eyrie.abode import build_abode
    abode = build_abode(filesystem, environment)
    from eyrie.devices import build_devices
    devices = build_devices(abode, bus, controller, filesystem)

    # Side channel data recording.
    add_data_recorders(abode, args)

    # Finally, really initialize our controller.
    controller.init(abode, devices, environment, filesystem, bus, scheduler)

    bus.start()
    scheduler.start()

    filesystem.run()

    scheduler.shutdown()
    bus.exit()
    bus.join()
    return 0


if __name__ == '__main__':
    sys.exit(main())
