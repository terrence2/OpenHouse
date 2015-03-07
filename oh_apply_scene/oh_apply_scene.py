#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import argparse
import asyncio
import logging

from collections import namedtuple

import shared.util as util
import shared.aiohome as aiohome

log = logging.getLogger('oh_apply_scene')


'''


class RoomScene:
    def __init__(self, path: str, data: dict, home: Home, owner: 'HomeScene'):
        self.path_ = path
        self.home_ = home
        self.owner_ = owner

        # We only want to apply any particular scene if the room scene allows it. We obviously
        # can't query it on-the-fly however, because that will recurse, so store the scene we
        # set locally whenever we get onchanged. Any re-application of the same scene will still
        # refresh but we will keep local modifications if applying a different scene globally.
        self.cached_scene_ = 'auto';

        self.room_name_ = data['attrs']['name']
        self.room_filter_ = 'room[name={}]'.format(self.room_name_)

        # Listen for updates to the scene property.
        self.home_.subscribe(self.path_, self.on_changed)

        # Acquire scenes defined on this room.
        nodes = self.home_.query('home > room[name={}] > scene'.format(self.room_name_)).run()
        self.scenes_ = {data['attrs']['name']: Scene(scene_path, home) for scene_path, data in nodes.items()}

    def on_changed(self, path: str, data: dict):
        # The scene name may be defined on the home, so apply there to this room only.
        new_scene = data['attrs'].get('scene', 'unset')
        self.cached_scene_ = new_scene
        self.owner_.apply_scene([self], new_scene)

    def apply_scene(self, scene_name: str, global_scene: Scene):
        # When re-applying the "auto" scene we want to update the scene to be what is set on the home.
        if scene_name == 'auto':
            return self.owner_.apply_scene([self], self.owner_.cached_scene_)

        # If our local scene has been overridden, do not apply the change. Note that any local change sets
        # the cached scene to the currently applied scene before calling apply, so cached_scene should be
        # equal to new_scene in that case.
        if self.cached_scene_ != 'auto' and self.cached_scene_ != scene_name:
            log.debug("skipping scene update for room {} since the local scene is {}"
                      .format(self.room_name_, scene_name))
            return

        # Lookup any local overrides to the scene.
        local_scene = self.scenes_.get(scene_name, None)

        # Apply the global and any local scene overrides, limited to this room.
        group = self.home_.group()
        if global_scene:
            global_scene.apply(group, self.room_filter_)
        if local_scene:
            local_scene.refresh_cache(self.home_)
            local_scene.apply(group, self.room_filter_)
        group.run()


class HomeScene:
    def __init__(self, path: str, home: Home):
        self.path_ = path
        self.home_ = home

        # When the state 'auto' gets applied to a room, automatically inherit the last value set on the home.
        self.cached_scene_ = 'unset';

        # Listen for updates to the scene property.
        self.home_.subscribe(self.path_, self.on_changed)

        # Acquire scenes defined on this home.
        nodes = self.home_.query('home > scene').run()
        self.scenes_ = {data['attrs']['name']: Scene(scene_path, home) for scene_path, data in nodes.items()}

        # A list of rooms to which we need to apply scenes at this level.
        self.rooms_ = []

    def add_room(self, room: RoomScene):
        self.rooms_.append(room)

    def on_changed(self, path: str, data: dict):
        # Always update when we poke the scene, even if the scene name is the same. This will flush
        # out any local changes to light state that have been made since the last application of
        # the scene.
        new_scene = data['attrs'].get('scene', 'unset')
        self.cached_scene_ = new_scene
        self.apply_scene(self.rooms_, new_scene)

    def apply_scene(self, rooms: [RoomScene], scene_name: str):
        log.info("Applying scene '{}' to {} rooms".format(scene_name, len(rooms)))
        scene = self.scenes_.get(scene_name, None)
        if scene:
            scene.refresh_cache(self.home_)

        for room in rooms:
            room.apply_scene(scene_name, scene)
'''


class Room:
    def __init__(self, name: str, path: str, node: aiohome.NodeData):
        self.name = name
        self.path_ = path
        self.last_activity_ = node.attrs.get('activity', 'unknown')
        self.owner_ = None

    @property
    def activity(self):
        return self.last_activity_

    @classmethod
    @asyncio.coroutine
    def create(cls, home: aiohome.Home, path: str, node: aiohome.NodeData) -> 'Room':
        self = cls(node.name, path, node)
        yield from home.subscribe(path, self.on_room_changed)
        return self

    @asyncio.coroutine
    def on_room_changed(self, path: str, node: aiohome.NodeData):
        assert path == self.path_
        new_activity = node.attrs.get('activity', 'unknown')
        log.info("room state changed from {} to {}".format(self.last_activity_, new_activity))
        if new_activity == self.last_activity_:
            return
        self.last_activity_ = new_activity
        if self.owner_:
            yield from self.owner_.on_room_changed(node.name)

    def set_home(self, owner: 'Home'):
        self.owner_ = owner


class Scene:
    SceneRule = namedtuple('SceneRule', ['name', 'lookup', 'style'])

    def __init__(self, base_rules: ['Scene.SceneRule'], activity_rules: {str: ['Scene.SceneRule']}, rooms: {str: str}):
        self.base_rules_ = base_rules
        self.activity_rules_ = activity_rules
        self.rooms_ = rooms

    @classmethod
    @asyncio.coroutine
    def create(cls, home: aiohome.Home, path: str, rooms: {str: Room}):
        # Load base rules that are always applied.
        base_rules = yield from cls.get_scene_rules_at(home, path)

        # Get per-activity rules that are applied where an activity is present.
        activity_rules = {}
        activity_nodes = yield from home(home.path_to_query(path) + ' > activity').run()
        for activity_path, activity_node in activity_nodes.items():
            activity_rules[activity_node.name] = yield from cls.get_scene_rules_at(home, activity_path)

        return cls(base_rules, activity_rules, rooms)

    @classmethod
    @asyncio.coroutine
    def get_scene_rules_at(cls, home: aiohome.Home, path: str) -> ['Scene.SceneRule']:
        query = home.path_to_query(path)
        rule_nodes = yield from home(query + ' > rule').run()
        rules = [cls.SceneRule(node.name, node.text, node.attrs['style']) for path, node in rule_nodes.items()]
        rules = sorted(rules, key=lambda rule: rule.name)
        return rules

    def apply(self, group: aiohome.QueryGroup):
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


class Home:
    def __init__(self, home: aiohome.Home, path: str, scenes: {str: Scene}, rooms: {str: Room}):
        self.home_ = home
        self.path_ = path
        self.scenes_ = scenes
        self.rooms_ = rooms
        self.current_scene_ = 'on'

    @classmethod
    @asyncio.coroutine
    def create(cls, home: aiohome.Home):
        home_path = yield from cls.get_home_path(home)
        rooms = yield from cls.find_rooms(home)
        scenes = yield from cls.find_scenes(home, rooms)
        self = cls(home, home_path, scenes, rooms)
        for room in rooms.values():
            room.set_home(self)
        yield from home.subscribe(home_path, self.on_home_changed)
        log.info("Created Home with {} scenes, {} rooms".format(len(scenes), len(rooms)))
        return self

    @staticmethod
    @asyncio.coroutine
    def find_rooms(home: aiohome.Home) -> {str: Room}:
        # Load the list of rules to apply scenes to.
        rooms = {}
        room_nodes = yield from home('room').run()
        for room_path, room_node in room_nodes.items():
            rooms[room_node.name] = yield from Room.create(home, room_path, room_node)
        return rooms

    @staticmethod
    @asyncio.coroutine
    def find_scenes(home: aiohome.Home, rooms: {str: Room}) -> {str: Scene}:
        nodes = yield from home('home > scene').run()
        scenes = {}
        for path, node in nodes.items():
            scenes[node.name] = yield from Scene.create(home, path, rooms)
        return scenes

    @staticmethod
    @asyncio.coroutine
    def get_home_path(home: aiohome.Home) -> str:
        nodes = yield from home('home').run()
        assert len(nodes) == 1
        return list(nodes.keys())[0]

    @asyncio.coroutine
    def on_home_changed(self, path: str, node: aiohome.NodeData):
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
    parser = argparse.ArgumentParser(description='Map scene changes to light changes.')
    util.add_common_args(parser)
    args = parser.parse_args()

    util.enable_logging(args.log_target, args.log_level)
    home = yield from aiohome.connect(('localhost', 8080))

    home_proxy = yield from Home.create(home)


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
