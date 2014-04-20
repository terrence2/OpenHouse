# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
__author__ = 'terrence'

import logging


def enable_logging(level):
    log_level = getattr(logging, level)
    logging.basicConfig(
        format='%(asctime)s:%(levelname)s:%(name)s:%(message)s',
        filename='mcp-eventlog.log', level=log_level)
    log = logging.getLogger('home')
    return log

