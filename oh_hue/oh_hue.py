#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import logging
from hue.bridge import Bridge
from hue.light import Light
from oh_shared.args import parse_default_args
from oh_shared.log import enable_logging
from oh_shared.db import Connection, EventKind, Tree
from pathlib import PurePosixPath

log = logging.getLogger('oh_hue')


async def make_connection(args):
    tree = await Tree.connect((args.home_address, args.home_port),
                              args.ca_chain, args.certificate, args.private_key)
    return tree


def names_from_color_paths(paths: [str]):
    return [PurePosixPath(path).parent.name for path in paths]


async def main():
    args = parse_default_args('Map light style changes to hue commands.')
    enable_logging(args.log_target, args.log_level)

    tree = await make_connection(args)

    # Create the bridge.
    bridge = await Bridge.create(tree)

    # Subscribe to all light changes.
    async def on_color_changed(paths: [str], _: EventKind, context: str):
        names = names_from_color_paths(paths)
        log.info("changed color of {} to {}".format(names, context))
        await bridge.set_lights_to_color(names, context)
    await tree.subscribe("/room/*/hue-*/*/color", on_color_changed)




    """
    res = yield from home.query("light[kind=hue], light[kind=hue-livingcolors]").run()
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

if __name__ == '__main__':
    tree = asyncio.get_event_loop().run_until_complete(main())
    asyncio.get_event_loop().run_forever()
