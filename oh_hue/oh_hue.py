#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import shared.util as util
from shared.home import Home

from bridge import Bridge
from light import Light

import logging

from pprint import pprint
from threading import RLock

from prompt_toolkit.contrib.repl import embed

log = logging.getLogger('oh_hue')


class BridgeNotFound(Exception):
    def __init__(self, light_name):
        super().__init__()
        self.light_name = light_name


def find_bridge_owning_light(bridges: [Bridge], light_node) -> (Bridge, str):
    # FIXME: fix this name hack and rename the lights in my house.
    name = 'hue-' + light_node['attrs']['name']
    for bridge in bridges:
        if bridge.owns_light_named(name):
            return bridge, bridge.get_id_for_light_named(name)
    raise BridgeNotFound(name)


def main():
    util.enable_logging('events.log', 'DEBUG')
    with util.run_thread(Home((3, 0), RLock())) as home:

        # Find all configured bridges.
        res = home.query("[kind='hue-bridge']").run()
        bridges = []
        for name, node in res.items():
            bridges.append(Bridge(name, node['attrs']['ipv4'], node['attrs']['username'], home))

        # Find all configured lights.
        res = home.query("[kind='hue']").run()
        lights = []
        for path, node in res.items():
            try:
                bridge, light_id = find_bridge_owning_light(bridges, node)
                lights.append(Light(light_id, path, node['attrs']['name'], bridge, home))
            except BridgeNotFound as ex:
                log.error("Found light '{}' with no owning bridge. Please double-check the spelling."
                          .format(ex.light_name))

        # Show lights that may be unconfigured.
        for bridge in bridges:
            bridge.show_unqueried_lights()

        # Show the interactive prompt.
        embed(globals(), locals(), vi_mode=True)


if __name__ == '__main__':
    main()
