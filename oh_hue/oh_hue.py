#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import logging
from hue.bridge import Bridge
from oh_shared.args import parse_default_args
from oh_shared.log import enable_logging
from oh_shared.db import Connection, EventKind, Tree
from pathlib import PurePosixPath

log = logging.getLogger('oh_hue')


async def make_connection(args):
    tree = await Tree.connect((args.db_address, args.db_port),
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
        await bridge.set_lights_to_color(names, context)
    await tree.subscribe("/room/*/hue-*/*/color", on_color_changed)

    return tree, bridge.task


if __name__ == '__main__':
    tree, task = asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_until_complete(task)
    except KeyboardInterrupt:
        asyncio.get_event_loop().run_until_complete(tree.close())
        task.cancel()
        try:
            asyncio.get_event_loop().run_until_complete(task)
        except asyncio.CancelledError:
            pass
