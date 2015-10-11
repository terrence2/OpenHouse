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

# Describes a Rule used by an <design> to define a lighting configuration.
Rule = namedtuple('Rule', ['name', 'lookup', 'style', 'class_'])


class DesignProxy:
    """
    Holds information about a <design> node somewhere in the tree.
    """
    def __init__(self, home: Home, path: str, rules: [Rule]):
        self.home_ = home
        self.path_ = path
        self.rules_ = rules

    @classmethod
    @asyncio.coroutine
    def create(cls, home: Home, design_path: str) -> "DesignProxy":
        """
        Generate an DesignProxy from a <design>.
        """
        rule_nodes = yield from home.query(home.path_to_query(design_path) + ' > rule').run()
        rules = []
        for path, node in rule_nodes.items():
            rules.append(Rule(node.name, node.text, node.attrs.get('style', ''), node.attrs.get('class', '')))
        rules = sorted(rules, key=lambda r: r.name)
        return cls(home, design_path, rules)

    @asyncio.coroutine
    def apply_in_context(self, context: str):
        change_map = {}
        for rule in self.rules_:
            log.debug("\tquery '{}' and set style: {}, class: {}".format(rule.lookup, rule.style, rule.class_))
            results = yield from self.home_.query(rule.lookup).run()
            changes = {path: rule for path in results if path.startswith(context)}
            change_map.update(changes)
        return change_map

    def __str__(self):
        return "\tActivity at {} with {} rules".format(self.path_, len(self.rules_))


class SceneProxy:
    """
    Holds information about a <scene> node somewhere in the tree.
    """
    SceneRule = namedtuple('SceneRule', ['name', 'lookup', 'style', 'class_'])
    RuleList = ['SceneProxy.SceneRule']

    def __init__(self, home: Home, path: str, node: NodeData, default_design: str, activities: {str: str}):
        self.name = node.name
        self.home_ = home
        self.path_ = path
        self.activities_ = activities
        self.default_design = default_design

    @classmethod
    @asyncio.coroutine
    def create(cls, home: Home, path: str, node: NodeData) -> "SceneProxy":
        """
        Generate a SceneProxy from a <scene>. The same scene name may be defined at the toplevel and provide
        per-room overrides, so we need to be careful to only load nodes under path context, not just the name.
        """
        default_design = node.attrs.get('default-design', '__unknown__')

        # Get defined activity => design map for this scene.
        activities = {}
        activity_nodes = yield from home.query(home.path_to_query(path) + ' > activity').run()
        for activity_path, activity_node in activity_nodes.items():
            activities[activity_node.name] = activity_node.attrs.get('design', '__unknown__')

        log.debug("Creating scene for {} with {} defined activities (default design {}):".format(
            path, len(activities), default_design))
        for name, design in activities.items():
            log.debug("\tactivity '{}' => {}".format(name, design))
        return cls(home, path, node, default_design, activities)


class RoomProxy:
    def __init__(self, name: str, path: str, node: NodeData, designs: {str: DesignProxy}, scenes: {str: SceneProxy}):
        self.name = name
        self.path = path
        self.design_overrides_ = designs
        self.scene_overrides_ = scenes
        self.last_activity_ = node.attrs.get('activity', 'unknown')
        self.last_design_ = node.attrs.get('design', '__unset__')

    @classmethod
    @asyncio.coroutine
    def create(cls, home: Home, path: str, node: NodeData) -> 'RoomProxy':
        """
        Create a RoomProxy from a <room> node.
        """
        room_designs = yield from cls.find_room_designs(home, node.name)
        room_scenes = yield from cls.find_room_scenes(home, node.name)
        self = cls(node.name, path, node, room_designs, room_scenes)
        return self

    @classmethod
    @asyncio.coroutine
    def find_room_designs(cls, home: Home, room_name: str) -> {str: SceneProxy}:
        """
        Load all <design>s being over-ridden in this room.
        """
        nodes = yield from home.query('room[name={}] > design'.format(room_name)).run()
        designs = {}
        for path, node in nodes.items():
            designs[node.name] = yield from DesignProxy.create(home, path)
        return designs

    @classmethod
    @asyncio.coroutine
    def find_room_scenes(cls, home: Home, room_name: str) -> {str: SceneProxy}:
        """
        Load all <scene>s being over-ridden in this room.
        """
        nodes = yield from home.query('room[name={}] > scene'.format(room_name)).run()
        scenes = {}
        for path, node in nodes.items():
            scenes[node.name] = yield from SceneProxy.create(home, path, node)
        return scenes

    def change_activity(self, node: NodeData) -> bool:
        """Apply a new activity from node. Return True if the activity was changed."""
        new_activity = node.attrs.get('activity', 'unknown')
        if new_activity == self.last_activity_:
            return False
        self.last_activity_ = new_activity
        return True

    def change_design(self, node: NodeData) -> bool:
        """Apply a new design from node. Return True if the design was changed."""
        new_design = node.attrs.get('design', '__unset__')
        if new_design == self.last_design_:
            return False
        self.last_design_ = new_design
        return True

    @asyncio.coroutine
    def apply_overrides_to_room(self, global_scene: SceneProxy, global_designs: {str: DesignProxy}):
        """
        The global scene has already been applied to the room with the default activity and added to the change map.
        The room may have an activity applied, so we need to query that to know what design to apply. Also, the
        room may override the scene to change what an activity does, so we need to take that into account as well.
        Further, the room may override what a design means locally, and that also needs to get accounted for.
        """
        # Constants over this computation.
        scene_name = global_scene.name
        activity_name = self.last_activity_

        # Get design for the current activity from the global.
        best_design = global_scene.activities_.get(activity_name, global_scene.default_design)

        # If we have a scene override for the current scene, and the override contains an override for the current
        # activity, then take the design that is specified there.
        if scene_name in self.scene_overrides_:
            if activity_name in self.scene_overrides_[scene_name].activities_:
                best_design = self.scene_overrides_[scene_name].activities_[activity_name]

        # The user may also have set an exact design to use on the room node. If so, obey that.
        if self.last_design_ != '__unset__':
            if self.last_design_ in global_designs or self.last_design_ in self.design_overrides_:
                best_design = self.last_design_

        # If the room overrides the names of designs and the design we chose is one of them, use the different design.
        if best_design in self.design_overrides_:
            design = self.design_overrides_[best_design]
        elif best_design in global_designs:
            design = global_designs[best_design]
        else:
            log.warning("in room {} for scene {} at activity {}: no design found: {}".format(self.name, scene_name,
                                                                                             activity_name,
                                                                                             best_design))
            return

        change_map = yield from design.apply_in_context(self.path)
        return change_map


class HomeProxy:
    def __init__(self, home: Home, path: str, designs: {str: DesignProxy}, scenes: {str: SceneProxy},
                 rooms: {str: RoomProxy}):
        self.home_ = home
        self.path_ = path
        self.designs_ = designs
        self.scenes_ = scenes
        self.rooms_ = rooms
        self.current_scene_ = '__unset__'

    @classmethod
    @asyncio.coroutine
    def create(cls, home: Home):
        home_path = yield from cls.get_home_path(home)
        global_designs = yield from cls.find_global_designs(home)
        global_scenes = yield from cls.find_global_scenes(home)
        rooms = yield from cls.find_rooms(home)
        self = cls(home, home_path, global_designs, global_scenes, rooms)
        for room in rooms.values():
            yield from home.subscribe(room.path, self.on_room_changed)
        yield from home.subscribe(home_path, self.on_home_changed)
        log.info("Created HomeProxy with {} global designs, {} global scenes, {} rooms".format(
            len(global_designs), len(global_scenes), len({})))
        return self

    @staticmethod
    @asyncio.coroutine
    def find_global_designs(home: Home) -> {str: DesignProxy}:
        """
        Load the set of toplevel designs that are available to all scenes in all rooms.
        """
        nodes = yield from home.query('home > design').run()
        designs = {}
        for path, node in nodes.items():
            designs[node.name] = yield from DesignProxy.create(home, path)
        return designs

    @staticmethod
    @asyncio.coroutine
    def find_global_scenes(home: Home) -> {str: SceneProxy}:
        """
        Load the set of toplevel scenes.
        """
        nodes = yield from home.query('home > scene').run()
        scenes = {}
        for path, node in nodes.items():
            scenes[node.name] = yield from SceneProxy.create(home, path, node)
        return scenes

    @staticmethod
    @asyncio.coroutine
    def find_rooms(home: Home) -> {str: RoomProxy}:
        """
        Load the list of rooms to apply scenes to.
        """
        # Load the list of rules to apply scenes to.
        rooms = {}
        room_nodes = yield from home.query('home > room').run()
        for room_path, room_node in room_nodes.items():
            rooms[room_node.name] = yield from RoomProxy.create(home, room_path, room_node)
        return rooms

    @staticmethod
    @asyncio.coroutine
    def get_home_path(home: Home) -> str:
        nodes = yield from home.query('home').run()
        assert len(nodes) == 1
        return list(nodes.keys())[0]

    @asyncio.coroutine
    def on_home_changed(self, path: str, node: NodeData):
        assert path == self.path_
        new_scene_name = node.attrs.get('scene', 'unset')
        if new_scene_name not in self.scenes_:
            log.error("Unrecognized scene selected: {}".format(new_scene_name))
            log.error("Known scenes are: {}".format(self.scenes_.keys()))
            return
        self.current_scene_ = new_scene_name
        yield from self.apply_scene(self.scenes_[new_scene_name])

    @asyncio.coroutine
    def on_room_changed(self, path: str, node: NodeData):
        assert node.name in self.rooms_
        room = self.rooms_[node.name]

        if self.current_scene_ not in self.scenes_:
            log.warning("Current scene {} is not in scenes_".format(self.current_scene_))
            return

        if not room.change_activity(node) and not room.change_design(node):
            return

        global_scene = self.scenes_[self.current_scene_]
        change_map = yield from room.apply_overrides_to_room(global_scene, self.designs_)
        self._print_change_information(self.current_scene_, change_map)
        yield from self._apply_changes(change_map)

    @asyncio.coroutine
    def apply_scene(self, scene: SceneProxy):
        log.warning("Applying Scene {}".format(scene.name))
        change_map = yield from self.apply_scene_to_home(scene)
        for room in self.rooms_.values():
            room_change_map = yield from room.apply_overrides_to_room(scene, self.designs_)
            change_map.update(room_change_map)
        self._print_change_information(scene.name, change_map)
        yield from self._apply_changes(change_map)

    @asyncio.coroutine
    def apply_scene_to_home(self, scene: SceneProxy):
        """
        At the top level, we don't have activities, so apply the default without any limiting context.
        """
        if scene.default_design not in self.designs_:
            log.warning("Attempting to apply unknown design at the toplevel: {}".format(scene.default_design))
            return
        return self.designs_[scene.default_design].apply_in_context(self.path_)

    @staticmethod
    def _print_change_information(new_scene: str, change_map: {str: "SceneProxy.SceneRule"}):
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
