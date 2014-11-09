#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

from functools import partial
from pprint import pprint
from threading import RLock

import util

from home import Home

from prompt_toolkit.contrib.repl import embed


log = logging.getLogger("oh_controller")


class SceneQuery:
    """
    A query to map a scene transition to the DOM.
    """
    def __init__(self, for_path: str, query: str, updates: {str: str}):
        # Scene queries are executed from least to most specific. The owning path allows us to sort them correctly.
        self.for_path = for_path

        # The query that will apply this scene to the DOM.
        self.query = query
        self.updates = updates

    def __lt__(self, other):
        return self.for_path > other.for_path

    def __str__(self):
        return "Query({} <= {})".format(self.query.strip(), self.updates)


class Scene:
    """
    A series of queries to run to transition the DOM to a new scene.
    """
    def __init__(self, name: str):
        self.name = name
        self.queries = []

    def __str__(self):
        return "Scene('{}' with {} queries)".format(self.name, len(self.queries))


def on_home_control_changed(home: Home, target: str, message: dict):
    attributes = message['attrs']
    if 'control' not in attributes:
        log.warning("No control attribute on home: {}".format(target))
        return

    # Select the relevant set of lights that we care about.


def on_room_control_changed(home: Home, scenes: {str: Scene}, target: str, message: dict):
    attributes = message['attrs']
    if 'control' not in attributes:
        log.warning("No control attribute on room: {}".format(target))
        return
    new_control = attributes['control']

    # Get all affected devices.
    affected = home.query(home.path_to_query(target) + " [kind='hue']").run()

    group = home.group()

    if new_control in scenes:
        log.warning("Setting the lights in {} to scene: {}".format(target, new_control))
        for scene_query in scenes[new_control].queries:
            # FIXME: filter the queries we are doing by for_path.startswith(target).
            query = group.query(scene_query.query)
            for key, value in scene_query.updates.items():
                query = query.attr(key, value)
            log.info("Adding query '{}' <= {}".format(scene_query.query.strip(), scene_query.updates))

    elif new_control == 'auto':
        log.error("Skipping auto request for {}".format(target))

    else:
        log.warning("Unrecongized control request: {}".format(new_control))

    group.run()


def establish_watches(home: Home, scenes: {str: Scene}):
    """
    Each home and each room in the home gets a "control" attribute. These attributes nest such that the most specific
    node has direct control.

    The possible modes for a |room| type node are:
        Specific manual instructions:
            'on', 'off', <identifier>
        Or deferral to the next higher node:
            'defer'

    The possible modes for a whole |home| type node are:
        Specific manual instructions:
            'on', 'off', <identifier>
        Or an automatic program:
            'auto'
    """
    res = home.query("[kind='home'], [kind='room']").run()
    for key in res.keys():
        if 'control' not in res[key]:
            home.query("[kind='home']").attr('control', 'auto').run()
        log.debug("Subscribing to messages from {}".format(key))
        home.subscribe(key, partial(on_room_control_changed, home, scenes))


def read_scenes(home: Home):
    log.debug("reading scenes")
    res = home.query("[kind='scene']").run()

    scenes = {}
    for path, result in res.items():
        # Ensure the scene exists.
        attrs = result['attrs']
        name = attrs['name']
        if name not in scenes:
            scenes[name] = Scene(name)
        scene = scenes[name]

        # Grab all child nodes of the scene node and add them as queries.
        children = home.query(home.path_to_query(path)).children().run()
        pprint(children)
        del children[path]  # We traversed it, so it will get included.
        for _, command in children.items():
            query = SceneQuery(path, command['text'], command['attrs'])
            scene.queries.append(query)

        # Sort the queries so they get applied in the right order.
        scene.queries = list(sorted(scene.queries))

    log.info("Read {} scenes:".format(len(scenes)))
    for scene in scenes.values():
        log.info("\t{}".format(str(scene)))
        for query in scene.queries:
            log.info("\t\t{}".format(str(query)))
    return scenes




def main():
    util.enable_logging('events.log', 'DEBUG')

    with util.run_thread(Home((3, 0), RLock())) as home:
        scenes = read_scenes(home)
        establish_watches(home, scenes)
        embed(globals(), locals(), vi_mode=True)


if __name__ == '__main__':
    main()
