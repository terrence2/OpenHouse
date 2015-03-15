#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import argparse
import logging

from pprint import pprint
from threading import RLock

import shared.util as util
from shared.home import Home

from bridge import Bridge
from light import Light

import shared.aiohome as aiohome


log = logging.getLogger('oh_hue')


class BridgeNotFound(Exception):
    def __init__(self, light_name):
        super().__init__()
        self.light_name = light_name


def find_bridge_owning_light(bridges: [Bridge], light_node: aiohome.NodeData) -> (Bridge, str):
    name = light_node.attrs['name']
    for bridge in bridges:
        if bridge.owns_light_named(name):
            return bridge, bridge.get_id_for_light_named(name)
    raise BridgeNotFound(name)


@asyncio.coroutine
def main():
    parser = argparse.ArgumentParser(description='Map light style changes to hue commands.')
    util.add_common_args(parser)
    args = parser.parse_args()

    util.enable_logging(args.log_target, args.log_level)
    home = yield from aiohome.connect(('localhost', 8080))

    # Find all configured bridges.
    res = yield from home("hue-bridge").run()
    bridges = []
    for path, node in res.items():
        bridge = yield from Bridge.create(path, node.attrs['ipv4'], node.attrs['username'], home)
        bridges.append(bridge)

    # Find all configured lights.
    res = yield from home("light[kind=hue], light[kind=hue-livingcolors]").run()
    lights = []
    for path, node in res.items():
        try:
            bridge, light_id = find_bridge_owning_light(bridges, node)
            light = yield from Light.create(light_id, path, node, bridge, home)
            lights.append(light)
        except BridgeNotFound as ex:
            log.error("Found light '{}' with no owning bridge. "
                      "Please double-check the spelling."
                      .format(ex.light_name))

    # Show lights that may be unconfigured.
    for bridge in bridges:
        bridge.show_unqueried_lights()

    """
    # Start all bridge threads.
    for bridge in bridges:
        bridge.start()

    # Show the interactive prompt.
    util.wait_for_exit(args.daemonize, globals(), locals())

    # Stop all bridge threads.
    for bridge in bridges:
        bridge.quit()
    for bridge in bridges:
        bridge.join()
    """


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
