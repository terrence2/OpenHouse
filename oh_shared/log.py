# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging
import sys
import coloredlogs


def enable_logging(filename: str, level: str):
    class Squelch(logging.Filter):
        def filter(self, record):
            if record.levelno == logging.DEBUG:
                return not record.name.startswith('asyncio') and \
                       not record.name.startswith('websockets') and \
                       not record.name.startswith('aiohttp')
            elif record.levelno == logging.INFO and record.name.startswith('asyncio'):
                return False
            return True


    root = logging.getLogger('')
    root.setLevel(logging.DEBUG)

    # File logger captures everything.
    file_handler = logging.FileHandler(filename)
    file_handler.setLevel(logging.DEBUG)
    formatter = logging.Formatter(fmt='%(asctime)s.%(msecs)03d:%(levelname)s:%(name)s[%(process)d]:%(message)s')
    file_handler.setFormatter(formatter)
    file_handler.addFilter(Squelch())
    root.addHandler(file_handler)

    coloredlogs.install(level=getattr(logging, level))

