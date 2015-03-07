#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import argparse
import asyncio
import logging
import shared.util as util

from wemo_discoverer import discover_local_wemos, WemoDeviceInfo
from wemo_event_server import listen_for_wemo_events
from wemo_device import WemoDevice

import shared.aiohome as aiohome

log = logging.getLogger('oh_wemo')


def main():
    parser = argparse.ArgumentParser(description='Bridge between OpenHouse and local WeMo devices.')
    util.add_common_args(parser)
    args = parser.parse_args()

    util.enable_logging(args.log_target, args.log_level)
    try:
        asyncio.get_event_loop().run_until_complete(manage_devices())
    except KeyboardInterrupt:
        return 0


@asyncio.coroutine
def manage_devices():
    home = yield from aiohome.connect(('localhost', 8080))

    nodes = yield from home('switch[kind=wemo], motion[kind=wemo]').run()
    config_devices = {node.name: node.tagName for node in nodes.values()}

    # Start the reply server, sending events to the wemo devices in |devices|.
    device_map = {}
    callback_address = yield from listen_for_wemo_events(device_map, home)

    # Start looking for wemos, passing them our server's address.
    @asyncio.coroutine
    def found_new_device(info: WemoDeviceInfo):
        if info.name in config_devices:
            device = WemoDevice(info.name, info.location, config_devices[info.name], callback_address)
            log.info("following wemo {} at {}".format(info.name, info.location))
            yield from device.follow_device(device_map)
        else:
            log.warning("not following WeMo {} -- add it to configuration?".format(info.name))
    yield from discover_local_wemos(found_new_device)


if __name__ == '__main__':
    main()
