# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import argparse
import logging
import socket
import sys
import time

from prompt_toolkit.contrib.repl import embed
from rainbow_logging_handler import RainbowLoggingHandler


def add_common_args(parser: argparse.ArgumentParser):
    group = parser.add_argument_group('OpenHouse Common Arguments')
    group.add_argument('--log-level', '-l', default='DEBUG',
                       help="The logging level. (default DEBUG)")
    group.add_argument('--log-target', '-L', default='events.log',
                       help="The logging target. (default events.log)")
    group.add_argument('--home-address', '-H', default='localhost', type=str,
                       help="The HOMe daemon's ipv4 address.")
    group.add_argument('--home-port', '-P', default=8080, type=int,
                       help="The HOMe daemon's ipv4 port.")


def enable_logging(filename: str, level: str):
    class Squelch(logging.Filter):
        def filter(self, record):
            if record.levelno == logging.DEBUG:
                return not record.name.startswith('asyncio') and \
                       not record.name.startswith('websockets') and \
                       not record.name.startswith('aiohttp')
            return True

    #formatter = logging.Formatter('%(pathname)s [%(module)s] - %(funcName)s:L%(lineno)d : %(message)s')
    formatter = logging.Formatter(fmt='%(asctime)s:%(levelname)s:%(name)s:%(message)s')

    # File logger captures everything.
    file_handler = logging.FileHandler(filename)
    file_handler.setLevel(logging.DEBUG)

    # Console output level is configurable.
    stream_handler = RainbowLoggingHandler(
        sys.stdout,
        color_asctime=('cyan', None, False),
        color_levelname=('gray', None, False),
        color_module=('yellow', None, False),
        color_name=('blue', None, False),
        color_lineno=('green', None, False),
    )
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


def wait_for_exit(daemonize, global_env, local_env):
    if daemonize:
        try:
            while True:
                time.sleep(60)
        except KeyboardInterrupt:
            pass
    else:
        embed(global_env, local_env, vi_mode=True)


def get_own_internal_ip_slow() -> str:
    """
    Discovering the active internal interface that new connections will get spawned on -- e.g. that local peers can
    (in typical networks) call back on -- is actually quite hard. We spawn a connection to an external resource and
    derive the internal network from that. A rather inelegant hack, but it gets the job done.
    """
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect(('8.8.8.8', 80))
        return s.getsockname()[0]
    except socket.error:
        return None
    finally:
        # Don't wait around for the GC.
        s.close()
        del s

