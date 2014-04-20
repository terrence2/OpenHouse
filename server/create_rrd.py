#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
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
                '-s', '30',  # Step size / base interval == 30 seconds.
                'DS:{}:GAUGE:90:{}:{}'.format(ds_name, minimum, maximum),  # Data source => DS:<name>:<type>:<heartbeat>:<min>:<max>
                # 100yrs @ 30s per sample === 105,120,000 samples
                'RRA:AVERAGE:.75:30:105120000'  # Archive format => RRA:<consolidation fn>:[<unknown "factor">:<step>:<rows>
            ]
            print("Running: {}".format(subprocess.list2cmdline(command)))
            if not args.dry_run:
                subprocess.check_call(command)

if __name__ == '__main__':
    sys.exit(main())
