#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from oh_shared.args import make_parser
from oh_shared.db import Connection, Tree
from oh_shared.log import enable_logging
from pathlib import Path
import ast
import asyncio
import logging
from collections import defaultdict
from pathlib import PurePosixPath as Path


log = logging.getLogger("oh_color")


class Color:
    """
    Note: this is a simple stub so that we can support button events, but little else.
    """
    @classmethod
    async def create(cls, path: Path, value: str, tree: Tree):
        self = cls()
        self.path = path
        self.value = value
        self.light_kind = path.parent.name
        async def on_change(changes: {str: [str]}):
            self.value = next(iter(changes.keys()))
        await tree.watch_matching_files(str(path), on_change)
        return self


def make_room_color_handler(palette: {str: {str: Color}}, tree: Tree):
    async def on_room_color_changed(changes: {str: [str]}):
        log.info("color change detected: {}".format(changes))
        for color_name, changed_paths in changes.items():
            if color_name not in palette:
                log.warning("Unknown color '{}' set on: {}".format(color_name, changed_paths))
                return

            colors_by_light_kind = palette[color_name]

            rooms_changed = '{' + ','.join([Path(p).parent.name for p in changed_paths]) + '}'
            for light_kind, color in colors_by_light_kind.items():
                lights_glob = Path("/room") / rooms_changed / light_kind / "*" / "color"
                log.info("updating {} to {}".format(lights_glob, color.value))
                await tree.set_matching_files(str(lights_glob), color.value)

    return on_room_color_changed


async def main():
    parser = make_parser("Map room colors into light colors.")
    args = parser.parse_args()

    enable_logging(args.log_target, args.log_level)

    async with Connection.from_args(args) as tree:
        palette = defaultdict(dict)  # {color_name: {light_kind: Color}}
        colors = await tree.get_matching_files("/global/palette/*/*light/color")
        for path, value in colors.items():
            color_name = Path(path).parent.parent.name
            color = await Color.create(Path(path), value, tree)
            palette[color_name][color.light_kind] = color

        await tree.watch_matching_files("/room/*/color", make_room_color_handler(palette, tree))

        while True:
            try:
                await asyncio.sleep(500)
            except KeyboardInterrupt:
                return


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
