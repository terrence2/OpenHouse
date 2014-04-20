#!/usr/bin/python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import argparse
import logging

from filesystem import FileSystem
from network import Network
from sensormodel import SensorModel


def main():
    parser = argparse.ArgumentParser(
        description='I AM THE MASTER CONTROL PROGRAM END OF LINE')
    parser.add_argument('-V', '--loglevel', dest='loglevel', default='DEBUG',
                        help='Set the log level (default:DEBUG).')
    args = parser.parse_args()

    global log
    log_level = getattr(logging, args.loglevel.upper())
    logging.basicConfig(
        format='%(asctime)s:%(levelname)s:%(name)s:%(message)s',
        filename='mcp-eventlog.log', level=log_level)
    log = logging.getLogger('home')

    from instances.foothill import build_floorplan
    floorplan = build_floorplan()
    sensor_model = SensorModel(floorplan)

    network = Network(floorplan, sensor_model)
    network.start()

    #mountpoint = sys.argv[1]
    #operations = Operations()
    fs = FileSystem(floorplan)
    fs.run()

    print("Waiting for Network to exit...")
    network.ready_to_exit = True
    network.join(10)


if __name__ == '__main__':
    main()
