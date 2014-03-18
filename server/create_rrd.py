#!/usr/bin/env python3
__author__ = 'terrence'


import argparse
import os
import os.path
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description='Create the rrd db files for MCP.')
    parser.add_argument('--rrd-path', default=os.path.expanduser("~/.local/var/db/mcp/"),
                        help='Where to find rrd records and record their data.')
    parser.add_argument('--dry-run', action='store_true',
                        help="Don't actually do anything, just tell us what we would do.")
    args = parser.parse_args()

    rooms = ('bedroom', 'office', 'livingroom')
    data_sources = (
        ('temperature', -30, 100),
        ('humidity', 0, 100),
        ('motion', 0, 1),
    )
    if not os.path.isdir(args.rrd_path):
        print("Making: {}".format(args.rrd_path))
        if not args.dry_run:
            os.makedirs(args.rrd_path)
    for room in rooms:
        for ds_name, minimum, maximum in data_sources:
            path = os.path.join(args.rrd_path, '{}-{}.rrd'.format(room, ds_name))
            command = [
                'rrdtool', 'create', path,
                '-s', '3',  # Step size / base interval == 3 seconds.
                'DS:{}:GAUGE:10:-30:100'.format(ds_name),  # Data source => DS:<name>:<type>:<heartbeat>:<min>:<max>
                'RRA:AVERAGE:.75:3:504576000'  # Archive format => RRA:<consolidation fn>:[<unknown "factor">:<step>:<rows>
            ]
            # 504,576,000 samples == 1,513,728,000 seconds == 48 years @ ~3.8GiB
            print("Running: {}".format(subprocess.list2cmdline(command)))
            if not args.dry_run:
                subprocess.check_call(command)

if __name__ == '__main__':
    sys.exit(main())
