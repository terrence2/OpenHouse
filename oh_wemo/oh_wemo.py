#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import logging
from oh_shared.args import parse_default_args
from oh_shared.home import Home
from oh_shared.log import enable_logging
from wemo.device import WemoDevice
from wemo.discoverer import discover_local_wemos, WemoDeviceInfo
from wemo.event_server import listen_for_wemo_events

log = logging.getLogger('oh_wemo')


@asyncio.coroutine
def main():
    args = parse_default_args('Bridge between OpenHouse and local WeMo devices.')
    enable_logging(args.log_target, args.log_level)
    home = yield from Home.connect((args.home_address, args.home_port))

    nodes = yield from home.query('switch[kind=wemo], motion[kind=wemo]').run()
    config_devices = {node.name: node.tagName for node in nodes.values()}
    log.debug("Searching for configured devices:")
    for i, (name, type) in enumerate(config_devices.items()):
        log.debug("{}#{:<2}: {}".format(type.lower(), i, name))

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
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
