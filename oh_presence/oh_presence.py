#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import functools
import logging
import shared.aiohome as home

from pprint import pformat

import shared.util as util

log = logging.getLogger("oh_databind")


def get_state(node: home.NodeData) -> bool:
    return parse_bool(node.attrs.get('state', 'false'))


def parse_bool(field: str) -> bool:
    assert field == 'true' or field == 'false'
    return field == 'true'


class RoomState:
    def __init__(self, name: str):
        self.name = name
        self.switches_ = {}
        # TODO: self.motions_ = {}

    def are_humans_present(self):
        return any(self.switches_.values())

    def add_switch(self, path: str, initial_state: bool):
        assert path not in self.switches_
        self.switches_[path] = initial_state

    def update_switch(self, S: home.Home, path: str, new_state: bool):
        assert path in self.switches_
        self.switches_[path] = new_state
        yield from S('room[name={}]'.format(self.name)).attr('humans_present', self.are_humans_present()).run()


@asyncio.coroutine
def handle_switch_change(S, room_state: RoomState, switch_path: str, switch_node: home.NodeData):
    log.info("switch {} state change: {}".format(switch_path, get_state(switch_node)))
    return room_state.update_switch(S, switch_path, get_state(switch_node))


@asyncio.coroutine
def main():
    util.enable_logging('output.log', 'DEBUG')
    S = yield from home.connect(('localhost', 8080))

    room_states = {}
    rooms = yield from S('room').run()
    for room_path, room_node in rooms.items():
        room_name = room_node.attrs['name']
        room_state = room_states[room_name] = RoomState(room_name)

        switches = yield from S('room[name={}] > wemo-switch'.format(room_name)).run()
        for switch_path, switch_node in switches.items():
            room_state.add_switch(switch_path, get_state(switch_node))
            yield from S.subscribe(switch_path, functools.partial(handle_switch_change, S, room_states[room_name]))


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
