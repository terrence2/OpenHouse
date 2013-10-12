#!/usr/bin/python3
import argparse
import logging
import sys

from network import Network
from sensormodel import SensorModel


def mcp_loop():
    """Runs in thread 1 to drive the house based on network events."""


def main():
    parser = argparse.ArgumentParser(
        description='I AM THE MASTER CONTROL PROGRAM END OF LINE')
    parser.add_argument('-V', '--loglevel', dest='loglevel', default='DEBUG',
                        help='Set the log level (default:DEBUG).')
    args = parser.parse_args()

    global log
    loglevel = getattr(logging, args.loglevel.upper())
    logging.basicConfig(
        format='%(asctime)s:%(levelname)s:%(name)s:%(message)s',
        filename='eventlog.log', level=loglevel)
    log = logging.getLogger('home')

    from instances.foothill import build_floorplan
    floorplan = build_floorplan()
    smodel = SensorModel(floorplan)
    network = Network(floorplan, smodel)
    return network.run()

if __name__ == '__main__':
    sys.exit(main())
