#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
"""
A <switch> or <button> node may have one of the following attrs attached to it:
    design="<name>"
       When the switch or button state becomes "true", the design is set to name on the owning room.
       When the switch or button state becomes "false", the design is unset from the owning room.

    scene="<name>"
       When the switch or button state becomes "true", the scene is set on the owning dwelling.
       No action is taken for "false".

  #### IMPLEMENT ME ####
    design="__activity__"
      Set the design based on the activity[name=yes] of the current scene.
"""
import asyncio
import logging
from oh_shared.args import parse_default_args
from oh_shared.home import Home, NodeData
from oh_shared.log import enable_logging
from pathlib import PurePath

log = logging.getLogger("oh_apply_sensor")


def is_truthy(state: str) -> bool:
    state = state.lower()
    return state in ('true', 'on', '1', 'yes', 'enable', 'enabled')


def make_sensor_update_handler(home: Home):
    @asyncio.coroutine
    def on_sensor_updated(path: str, node: NodeData):
        room_name = str(PurePath(path).parent.name)
        raw_state = node.attrs.get("state", None)
        new_state = raw_state and is_truthy(raw_state)

        log.debug("Sensor {} in room {} updated to {}".format(path, room_name, new_state))

        want_design = node.attrs.get("design", None)
        want_scene = node.attrs.get("scene", None)
        if new_state:
            if want_scene:
                log.debug("Applying scene {}".format(want_scene))
                yield from home.query("home").attr("scene", want_scene).run()
            if want_design:
                log.debug("Applying design {} to room {}".format(want_design, room_name))
                yield from home.query("room[name='{}']".format(room_name)).attr("design", want_design).run()
        else:
            if want_design:
                log.debug("Unsetting all designs in room {}".format(room_name))
                yield from home.query("room[name='{}']".format(room_name)).attr("design", '').run()

    return on_sensor_updated


@asyncio.coroutine
def main():
    args = parse_default_args('Interpret switch and button states and apply design or scene as appropriate.')
    enable_logging(args.log_target, args.log_level)
    home = yield from Home.connect((args.home_address, args.home_port))
    handler = make_sensor_update_handler(home)

    # Find and subscribe to all switches and buttons.
    sensors = yield from home.query('switch, button').run()
    for key in sensors.keys():
        log.debug("Subscribing to {}".format(key))
        yield from home.subscribe(key, handler)


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
