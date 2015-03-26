#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import logging
from collections import namedtuple
from oh_shared.args import parse_default_args
from oh_shared.log import enable_logging
from oh_shared.home import Home, QueryGroup, NodeData

log = logging.getLogger('oh_apply_scene')


class RoomProxy:
    def __init__(self, name: str, path: str, node: NodeData):
        self.name = name
        self.path_ = path
        self.last_activity_ = node.attrs.get('activity', 'unknown')
        self.owner_ = None

    @property
    def activity(self):
        return self.last_activity_

    @classmethod
    @asyncio.coroutine
    def create(cls, home: Home, path: str, node: NodeData) -> 'RoomProxy':
        self = cls(node.name, path, node)
        yield from home.subscribe(path, self.on_room_changed)
        return self

    @asyncio.coroutine
    def on_room_changed(self, path: str, node: NodeData):
        assert path == self.path_
        new_activity = node.attrs.get('activity', 'unknown')
        if new_activity == self.last_activity_:
            return
        log.info("room state changed from {} to {}".format(self.last_activity_, new_activity))
        self.last_activity_ = new_activity
        if self.owner_:
            yield from self.owner_.on_room_changed(node.name)

    def set_home_proxy(self, owner: 'HomeProxy'):
        self.owner_ = owner


class SceneProxy:
    SceneRule = namedtuple('SceneRule', ['name', 'lookup', 'style'])
    RuleList = ['SceneProxy.SceneRule']

    def __init__(self, base_rules: 'RuleList', activity_rules: {str: 'RuleList'}, rooms: {str: str}):
        self.base_rules_ = base_rules
        self.activity_rules_ = activity_rules
        self.rooms_ = rooms

    @classmethod
    @asyncio.coroutine
    def create(cls, home: Home, path: str, rooms: {str: RoomProxy}):
        # Load base rules that are always applied.
        base_rules = yield from cls.get_scene_rules_at(home, path)

        # Get per-activity rules that are applied where an activity is present.
        activity_rules = {}
        activity_nodes = yield from home.query(home.path_to_query(path) + ' > activity').run()
        for activity_path, activity_node in activity_nodes.items():
            activity_rules[activity_node.name] = yield from cls.get_scene_rules_at(home, activity_path)

        return cls(base_rules, activity_rules, rooms)

    @classmethod
    @asyncio.coroutine
    def get_scene_rules_at(cls, home: Home, path: str) -> 'RuleList':
        query = home.path_to_query(path)
        rule_nodes = yield from home.query(query + ' > rule').run()
        rules = [cls.SceneRule(node.name, node.text, node.attrs['style']) for path, node in rule_nodes.items()]
        rules = sorted(rules, key=lambda rule: rule.name)
        return rules

    def apply(self, group: QueryGroup):
        for rule in self.base_rules_:
            log.debug("query '{}' and set style to {}".format(rule.lookup, rule.style))
            group.query(rule.lookup).attr('style', rule.style)

        for room in self.rooms_.values():
            if room.activity not in self.activity_rules_:
                continue
            for rule in self.activity_rules_[room.activity]:
                query = 'room[name={}] '.format(room.name) + rule.lookup
                log.debug("query '{}' and set style to {}".format(query, rule.style))
                group.query(query).attr('style', rule.style)


class HomeProxy:
    def __init__(self, home: Home, path: str, scenes: {str: SceneProxy}, rooms: {str: RoomProxy}):
        self.home_ = home
        self.path_ = path
        self.scenes_ = scenes
        self.rooms_ = rooms
        self.current_scene_ = 'on'

    @classmethod
    @asyncio.coroutine
    def create(cls, home: Home):
        home_path = yield from cls.get_home_path(home)
        rooms = yield from cls.find_rooms(home)
        scenes = yield from cls.find_scenes(home, rooms)
        self = cls(home, home_path, scenes, rooms)
        for room in rooms.values():
            room.set_home_proxy(self)
        yield from home.subscribe(home_path, self.on_home_changed)
        log.info("Created HomeProxy with {} scenes, {} rooms".format(len(scenes), len(rooms)))
        return self

    @staticmethod
    @asyncio.coroutine
    def find_rooms(home: Home) -> {str: RoomProxy}:
        """
        Load the list of rooms to apply scenes to.
        """
        # Load the list of rules to apply scenes to.
        rooms = {}
        room_nodes = yield from home.query('room').run()
        for room_path, room_node in room_nodes.items():
            rooms[room_node.name] = yield from RoomProxy.create(home, room_path, room_node)
        return rooms

    @staticmethod
    @asyncio.coroutine
    def find_scenes(home: Home, rooms: {str: RoomProxy}) -> {str: SceneProxy}:
        """
        Load the toplevel scene list.
        """
        nodes = yield from home.query('home > scene').run()
        scenes = {}
        for path, node in nodes.items():
            scenes[node.name] = yield from SceneProxy.create(home, path, rooms)
        return scenes

    @staticmethod
    @asyncio.coroutine
    def get_home_path(home: Home) -> str:
        nodes = yield from home.query('home').run()
        assert len(nodes) == 1
        return list(nodes.keys())[0]

    @asyncio.coroutine
    def on_home_changed(self, path: str, node: NodeData):
        assert path == self.path_
        new_scene = node.attrs.get('scene', 'unset')
        if new_scene not in self.scenes_:
            log.error("Unrecognized scene selected: {}".format(new_scene))
            log.error("Known scenes are: {}".format(self.scenes_.keys()))
            return
        self.current_scene_ = new_scene
        group = self.home_.group()
        self.scenes_[new_scene].apply(group)
        yield from group.run()

    @asyncio.coroutine
    def on_room_changed(self, room_name: str):
        if self.current_scene_ not in self.scenes_:
            log.error("Room scene changed with no valid scene set.")
            return
        group = self.home_.group()
        self.scenes_[self.current_scene_].apply(group)
        yield from group.run()


@asyncio.coroutine
def main():
    args = parse_default_args('Map scene changes to light changes.')
    enable_logging(args.log_target, args.log_level)
    home = yield from Home.connect((args.home_address, args.home_port))
    home_proxy = yield from HomeProxy.create(home)


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
