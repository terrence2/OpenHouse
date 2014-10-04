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

