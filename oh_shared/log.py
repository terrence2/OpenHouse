# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging
import sys
from rainbow_logging_handler import RainbowLoggingHandler


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

    formatter = logging.Formatter(fmt='%(asctime)s.%(msecs)03d:%(levelname)s:%(name)s:%(message)s')

    # File logger captures everything.
    file_handler = logging.FileHandler(filename)
    file_handler.setLevel(logging.DEBUG)

    # Console output level is configurable.
    stream_handler = RainbowLoggingHandler(
        sys.stdout,
        color_asctime=('cyan', None, False),
        color_msecs=('cyan', None, False),
        color_levelname=('gray', None, False),
        color_module=('yellow', None, False),
        color_name=('blue', None, False),
        color_lineno=('green', None, False),
        datefmt="%Y-%m-%d %H:%M:%S"
    )
    #stream_handler = logging.StreamHandler(sys.stdout)
    stream_handler.setLevel(getattr(logging, level))
    stream_handler.addFilter(Squelch())

    # Set an output format.
    stream_handler.setFormatter(formatter)
    file_handler.setFormatter(formatter)
    file_handler.addFilter(Squelch())

    # Add handlers to root.
    root = logging.getLogger('')
    root.setLevel(logging.DEBUG)
    root.addHandler(stream_handler)
    root.addHandler(file_handler)
