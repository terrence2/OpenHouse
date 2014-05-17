# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
__author__ = 'terrence'

import logging


class Device:
    def __init__(self, name):
        self.name = name
        self.device_type, self.room_name, self.device_name = name.split('-')


def enable_logging(level):
    # File logger captures everything.
    fh = logging.FileHandler('mcp-events.log')
    fh.setLevel(logging.DEBUG)

    # Console output level is configurable.
    ch = logging.StreamHandler()
    ch.setLevel(getattr(logging, level))

    # Set an output format.
    formatter = logging.Formatter('%(asctime)s:%(levelname)s:%(name)s:%(message)s')
    ch.setFormatter(formatter)
    fh.setFormatter(formatter)

    # Add handlers to root.
    root = logging.getLogger('')
    root.setLevel(logging.DEBUG)
    root.addHandler(ch)
    root.addHandler(fh)
