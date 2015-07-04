#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import logging
from collections import namedtuple
from oh_shared.args import parse_default_args
from oh_shared.log import enable_logging
from oh_shared.home import Home, NodeData

log = logging.getLogger('oh_apply_scene')

"""
Applies scene and activity changes to light states.
"""

class SceneProxy:
    """
    Holds information about a <scene> node somewhere in the tree.
    """
    SceneRule = namedtuple('SceneRule', ['name', 'lookup', 'style', 'class_'])
    RuleList = ['SceneProxy.SceneRule']

    def __init__(self, home: Home, path: str, default_activity: str, activity_rules: {str: 'RuleList'}):
        self.home_ = home
        self.path_ = path
        self.default_activity_ = default_activity
        self.activity_rules_ = activity_rules

    @classmethod
    @asyncio.coroutine
    def create(cls, home: Home, path: str, node: NodeData):
        # Get the default activity.
        default_activity = node.attrs.get('default-activity', 'yes')

        # Get per-activity rules that are applied where an activity is present.
        activity_rules = {}
        activity_nodes = yield from home.query(home.path_to_query(path) + ' > activity').run()
        for activity_path, activity_node in activity_nodes.items():
            activity_rules[activity_node.name] = yield from cls.get_scene_rules_at(home, activity_path)

        log.debug("Creating scene for {} with {} activities (default {}):".format(
            path, len(activity_rules), default_activity))
        for name, rules in activity_rules.items():
            log.debug("\tActivity {} with {} rules".format(name, len(rules)))
        return cls(home, path, default_activity, activity_rules)

    @classmethod
    @asyncio.coroutine
    def get_scene_rules_at(cls, home: Home, path: str) -> 'RuleList':
        query = home.path_to_query(path)
        rule_nodes = yield from home.query(query + ' > rule').run()
        rules = []
        for path, node in rule_nodes.items():
            rule = cls.SceneRule(node.name, node.text, node.attrs.get('style', ''), node.attrs.get('class', ''))
            rules.append(rule)
        rules = sorted(rules, key=lambda r: r.name)
        return rules

    @asyncio.coroutine
    def apply(self, context: str, activity: str) -> {str: 'SceneRule'}:
        """
        Query each rule in order to get the set of applicable paths that need to change. Each query may contain the
        same paths, so overlaying like this will only take the last path. This prevents many subsequent changes to
        the same path from stacking up.

        Note: we re-do the rule queries at each application so that such queries can make use of dynamic information.
        """
        using_default = False
        if activity is None or activity not in self.activity_rules_:
            activity = self.default_activity_
            using_default = True

        if activity not in self.activity_rules_:
            if using_default:
                log.error("No rules for scene {} for default activity {}".format(self.path_, activity))
            else:
                log.warning("No rules for scene {} for non-default activity {}".format(self.path_, activity))
            return {}

        log.debug("applying rules for {} in context {} for activity {}".format(self.path_, context, activity))
        change_map = {}
        for rule in self.activity_rules_[activity]:
            log.debug("\tquery '{}' and set style: {}, class: {}".format(rule.lookup, rule.style, rule.class_))
            results = yield from self.home_.query(rule.lookup).run()
            changes = {path: rule for path in results if path.startswith(context)}
            change_map.update(changes)
        return change_map


class RoomProxy:
    def __init__(self, name: str, path: str, node: NodeData, scenes: {str: SceneProxy}):
        self.name = name
        self.path = path
        self.scenes_ = scenes
        self.last_activity_ = node.attrs.get('activity', 'unknown')

    @classmethod
    @asyncio.coroutine
    def create(cls, home: Home, path: str, node: NodeData) -> 'RoomProxy':
        scenes = yield from cls.build_scenes(home, node.name)
        self = cls(node.name, path, node, scenes)
        return self

    @classmethod
    @asyncio.coroutine
    def build_scenes(cls, home: Home, room_name: str) -> {str: SceneProxy}:
        """
        Load the toplevel scene list.
        """
        nodes = yield from home.query('room[name={}] > scene'.format(room_name)).run()
        scenes = {}
        for path, node in nodes.items():
            scenes[node.name] = yield from SceneProxy.create(home, path, node)
        return scenes

    @asyncio.coroutine
    def apply_scene(self, global_scene: SceneProxy, new_scene: str):
        # Apply the global scene to this room with the current activity.
        global_changes = yield from global_scene.apply(self.path, self.last_activity_)

        # If we are overlaying scene rules for this scene, overlay them now.
        if new_scene not in self.scenes_:
            return global_changes
        own_changes = yield from self.scenes_[new_scene].apply(self.path, self.last_activity_)

        global_changes.update(own_changes)
        return global_changes

    def update_activity(self, node: NodeData):
        """
        Update the room's cached activity. Return true if the cached activity changed.
        """
        new_activity = node.attrs.get('activity', 'unknown')
        if new_activity == self.last_activity_:
            return False
        log.info("room state changed from {} to {}".format(self.last_activity_, new_activity))
        self.last_activity_ = new_activity
        return True


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
        rooms = yield from cls.build_rooms(home)
        scenes = yield from cls.build_scenes(home)
        self = cls(home, home_path, scenes, rooms)
        for room in rooms.values():
            yield from home.subscribe(room.path, self.on_room_changed)
        yield from home.subscribe(home_path, self.on_home_changed)
        log.info("Created HomeProxy with {} scenes, {} rooms".format(len(scenes), len(rooms)))
        return self

    @staticmethod
    @asyncio.coroutine
    def build_rooms(home: Home) -> {str: RoomProxy}:
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
    def build_scenes(home: Home) -> {str: SceneProxy}:
        """
        Load the toplevel scene list.
        """
        nodes = yield from home.query('home > scene').run()
        scenes = {}
        for path, node in nodes.items():
            scenes[node.name] = yield from SceneProxy.create(home, path, node)
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

        # Change the scene.
        new_scene_name = node.attrs.get('scene', 'unset')
        if new_scene_name not in self.scenes_:
            log.error("Unrecognized scene selected: {}".format(new_scene_name))
            log.error("Known scenes are: {}".format(self.scenes_.keys()))
            return
        self.current_scene_ = new_scene_name
        global_scene = self.scenes_[new_scene_name]

        # Apply the new global scene to the top level, then in each room.
        change_map = yield from global_scene.apply(self.path_, None)  # Always use default activity at root.
        for room in self.rooms_.values():
            room_updates = yield from room.apply_scene(global_scene, new_scene_name)
            change_map.update(room_updates)

        self._print_change_information(new_scene_name, change_map)
        yield from self._apply_changes(change_map)

    @asyncio.coroutine
    def on_room_changed(self, path: str, node: NodeData):
        assert node.name in self.rooms_
        room = self.rooms_[node.name]

        assert self.current_scene_ in self.scenes_
        global_scene = self.scenes_[self.current_scene_]

        # Update the room's activity property before re-applying the scene.
        if not room.update_activity(node):
            return

        # Apply the new information to the one room.
        change_map = yield from room.apply_scene(global_scene, self.current_scene_)

        self._print_change_information(self.current_scene_, change_map)
        yield from self._apply_changes(change_map)

    @staticmethod
    def _print_change_information(new_scene: str, change_map: {str: SceneProxy.SceneRule}):
        log.debug('applied scene {}; changing {} nodes:'.format(new_scene, len(change_map)))
        for path in sorted(change_map.keys()):
            log.debug("\t{}: {}".format(path, change_map[path]))

    @asyncio.coroutine
    def _apply_changes(self, change_map):
        group = self.home_.group()
        for path, rule in change_map.items():
            group.query_path(path).attr('style', rule.style).attr('class', rule.class_)
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
