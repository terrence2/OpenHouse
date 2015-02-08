#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import argparse
import logging

from collections import namedtuple
from threading import RLock

import shared.util as util
from shared.home import Home, QueryGroup


log = logging.getLogger('oh_scene')


class Scene:
    SceneRule = namedtuple('SceneRule', ['lookup', 'style'])

    def __init__(self, path: str, home: Home):
        self.path_ = path
        self.rules_ = []

        self.refresh_cache(home)

    def refresh_cache(self, home: Home):
        # FIXME: listen for changes to the scene and refresh on the fly.
        q = Home.path_to_query(self.path_) + ' > rule'
        children = home.query(q).run()
        self.rules_ = [self.SceneRule(children[path]['text'], children[path]['attrs']['style'])
                       for path in sorted(children.keys())]

    def apply(self, group: QueryGroup, filter: str):
        for rule in self.rules_:
            log.debug("query '{}' and set style to {}".format(filter + " " + rule.lookup, rule.style))
            group.query(filter + " " + rule.lookup).attr('style', rule.style)


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


def main():
    parser = argparse.ArgumentParser(description='Map scene changes to light changes.')
    util.add_common_args(parser)
    args = parser.parse_args()

    util.enable_logging(args.log_target, args.log_level)
    with util.run_thread(Home((3, 0), RLock())) as home:
        home_scene = HomeScene(home.get_home_path(), home)

        # Accumulate
        nodes = home.query('room').run()
        for path, data in nodes.items():
            home_scene.add_room(RoomScene(path, data, home, home_scene))

        util.wait_for_exit(args.daemonize, globals(), locals())


if __name__ == '__main__':
    main()
