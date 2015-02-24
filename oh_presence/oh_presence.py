#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
#
#  Use switch and motion states to infer human presence and activity.
#
#      Default logic for switches is:
#           /home/room[humans] :- any(/home/room/switch[state=true])
#
#      This can be overridden by using the switches on<state> properties, e.g.:
#          <switch kind="wemo" state="foo" onfoo="/path[attr=value]"></switch>

import argparse
import asyncio
import functools
import logging
import re
import shared.aiohome as aiohome
import shared.util as util

log = logging.getLogger("oh_presence")


def get_state(node: aiohome.NodeData) -> str:
    return node.attrs.get('state', 'false')


def parse_bool(field: str) -> bool:
    assert field == 'true' or field == 'false'
    return field == 'true'


class ActionFormatError(Exception):
    pass


class SwitchState:
    def __init__(self, room: 'RoomState', path: str, node: aiohome.NodeData):
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

        if action_path == self.path:
            log.warn("invalid action format: using 'self' as the target would recurse")
            raise ActionFormatError(action)

        query = aiohome.Home.path_to_query(action_path)
        return query, attr, value

    @asyncio.coroutine
    def on_change(self, home: aiohome.Home, room: 'RoomState', path: str, new_node: aiohome.NodeData):
        assert path == self.path
        assert room is self.owner
        self.last_node = new_node

        state = self.last_node.attrs.get('state', 'false')
        action = self.last_node.attrs.get('on' + state, None)

        # By default the switch state should just notify the room and let is_enabled control the 'humans' property.
        if not action:
            log.info("switch {} state change -> {}".format(path, get_state(new_node)))
            yield from self.owner.state_changed(home)
            return

        # If an action is set for the current state, act on it.
        try:
            log.info("switch {} applying action: {}".format(path, action))
            query, attr, value = self.parse_action(action)
            yield from home(query).attr(attr, value).run()
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
    def state_changed(self, home: aiohome.Home):
        next_state = 'yes' if self.are_humans_present() else 'no'
        yield from home('room[name={}]'.format(self.name)).attr('humans', next_state).run()


@asyncio.coroutine
def main():
    parser = argparse.ArgumentParser(description='Interpret switch and motion states to infer human activity.')
    util.add_common_args(parser)
    args = parser.parse_args()

    util.enable_logging(args.log_target, args.log_level)
    S = yield from aiohome.connect(('localhost', 8080))

    rooms = yield from S('room').run()
    for room_path, room_node in rooms.items():
        room_name = room_node.attrs['name']
        room = RoomState(room_path, room_name)

        switches = yield from S('room[name={}] > switch'.format(room_name)).run()
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
