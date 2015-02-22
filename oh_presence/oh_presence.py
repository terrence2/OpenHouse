#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
#
#  Map switch states => human states.
#
#      Default logic for switches is:
#           /home/room[humans] :- any(/home/room/switch[state=true])
#
#      This can be overridden by using the switches on<state> properties, e.g.:
#          <wemo-switch state="foo" onfoo="/path[attr=value]"></wemo-switch>
#        FIXME:
#          <switch kind="wemo" state="foo" onfoo="/path[attr=value]"></switch>

import asyncio
import functools
import logging
import re
import shared.aiohome as home

from pprint import pformat

import shared.util as util

log = logging.getLogger("oh_databind")


def get_state(node: home.NodeData) -> str:
    return node.attrs.get('state', 'false')


def parse_bool(field: str) -> bool:
    assert field == 'true' or field == 'false'
    return field == 'true'


class ActionFormatError(Exception):
    pass


class SwitchState:
    def __init__(self, room: 'RoomState', path: str, node: home.NodeData):
        self.owner = room
        self.path = path
        self.last_node = node

    def is_enabled(self):
        state = self.last_node.attrs.get('state', 'false')
        try:
            _ = self.last_node.attrs['on' + state]
            return False
        except KeyError:
            return parse_bool(state)

    def parse_action(self, action: str) -> (str, str, str):
        """
        Parse an action string into a query and attr/value pair.
        """
        match = re.search(r'(\w+)\[(\w+)=(\w+)\]', action)
        if not match:
            log.warn("invalid action format: expected <path>[<attr>=<value>]")
            raise ActionFormatError(action)
        action_path = match.group(1)
        attr = match.group(2)
        value = match.group(3)
        if action_path == 'self':
            action_path = self.path
        elif action_path == 'parent':
            action_path = self.owner.path
        query = home.Home.path_to_query(action_path)
        return query, attr, value

    @asyncio.coroutine
    def on_change(self, S: home.Home, room: 'RoomState', path: str, new_node: home.NodeData):
        log.info("{} == {}".format(path, self.path))
        assert path == self.path
        assert room is self.owner
        self.last_node = new_node

        state = self.last_node.attrs.get('state', 'false')
        action = self.last_node.attrs.get('on' + state, None)

        # By default the switch state should just notify the room and let is_enabled control the 'humans' property.
        if not action:
            log.info("switch {} state change -> {}".format(path, get_state(new_node)))
            yield from self.owner.state_changed(S)
            return

        # If an action is set for the current state, act on it.
        try:
            query, attr, value = self.parse_action(action)
            yield from S(query).attr(attr, value).run()
        except ActionFormatError:
            return


class RoomState:
    def __init__(self, path: str, name: str):
        self.path = path
        self.name = name

        self.switches_ = {}
        # TODO: self.motions_ = {}

    def are_humans_present(self):
        return any((switch.is_enabled() for switch in self.switches_.values()))

    def add_switch(self, switch: SwitchState):
        assert switch.path not in self.switches_
        self.switches_[switch.path] = switch

    @asyncio.coroutine
    def state_changed(self, S: home.Home):
        yield from S('room[name={}]'.format(self.name)).attr('humans_present', self.are_humans_present()).run()


@asyncio.coroutine
def handle_switch_change(S: home.Home, room_state: RoomState, switch_path: str, switch_node: home.NodeData):
    return room_state.update_switch(S, switch_path, switch_node)


@asyncio.coroutine
def main():
    util.enable_logging('output.log', 'DEBUG')
    S = yield from home.connect(('localhost', 8080))

    rooms = yield from S('room').run()
    for room_path, room_node in rooms.items():
        room_name = room_node.attrs['name']
        room = RoomState(room_path, room_name)

        switches = yield from S('room[name={}] > wemo-switch'.format(room_name)).run()
        for item in switches.items():
            switch = SwitchState(room, *item)
            room.add_switch(switch)
            yield from S.subscribe(switch.path, functools.partial(switch.on_change, S, room))


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
