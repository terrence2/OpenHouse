#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import mcp
from eyrie import Eyrie

import argparse
import logging
import os
import os.path

parser = argparse.ArgumentParser(description='Master Control Program')
parser.add_argument('--db-path', default=os.path.expanduser("~/.local/var/db/mcp/"),
                    help='Where to store our data.')
parser.add_argument('--log-level', '-l', default='INFO',
                    help="Set the log level (default: INFO).")
args = parser.parse_args()

mcp.enable_logging(level=args.log_level)
log = logging.getLogger('eyrie')

eyrie = Eyrie(args.db_path)
eyrie.run()
eyrie.cleanup()


old = """
def main():


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

    # Finally, really initialize our controller.
    controller.init(abode, devices, environment, filesystem, bus, scheduler)


    return 0


#if __name__ == '__main__':
#    sys.exit(main())
    """
