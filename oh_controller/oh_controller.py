#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import functools
import logging
import shared.aiohome as home
import shared.util as util


class RoomState:
    def __init__(self, name: str, node: home.NodeData):
        self.log = logging.getLogger('oh_controller.' + name)
        self.name = name
        self.humans_present = self.parse_boolean(node.attrs.get('humans_present', 'false'))
        self.present_scene = 'on'
        self.absent_scene = 'auto'

    @staticmethod
    def parse_boolean(value: str) -> bool:
        assert value in ('true', 'false')
        return value == 'true'

    @asyncio.coroutine
    def on_changed(self, S: home.Home, path: str, node: home.NodeData):
        new_present = self.parse_boolean(node.attrs.get('humans_present', 'false'))

        # Skip the callback we get when changing the state, or other unrelated changes.
        if new_present == self.humans_present:
            self.log.debug("skipping update of office with duplicate humans_present")
            return
        self.log.debug("room {} state transition {} -> {}".format(self.name, self.humans_present, new_present))
        self.humans_present = new_present

        next_scene = self.present_scene if self.humans_present else self.absent_scene
        self.log.info("Updating {} to scene {}".format(self.name, next_scene))
        yield from S('room[name={}]'.format(node.attrs['name'])).attr('scene', next_scene).run()


@asyncio.coroutine
def main():
    util.enable_logging('output.log', 'DEBUG')
    S = yield from home.connect(('localhost', 8080))

    room_states = {}
    rooms = yield from S('room').run()
    for room_path, room_node in rooms.items():
        room_name = room_node.attrs['name']
        state = room_states[room_name] = RoomState(room_name, room_node)
        yield from S.subscribe(room_path, functools.partial(state.on_changed, S))


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
