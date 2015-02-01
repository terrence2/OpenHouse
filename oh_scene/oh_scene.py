#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import argparse
import logging

from pprint import pprint
from threading import RLock

import shared.util as util
from shared.home import Home


log = logging.getLogger('oh_hue')


class Scene:
    def __init__(self, path: str, home: Home):
        self.path_ = path
        self.home_ = home

    def apply(self):
        # FIXME: listen for and re-apply scene kind changes on the fly.
        # FIXME: for now re-lookup the scene data every time we apply it to get a similar effect.
        q = self.home_.path_to_query(self.path_) + ' > div'
        children = self.home_.query(q).run()

        # Note that we need to traverse children in sorted order.
        paths = sorted(children.keys())
        for path in paths:
            data = children[path]
            log.info("query {} and set style to {}".format(data['text'], data['attrs']['style']))
            (self.home_.query(data['text'])
                       .attr('style', data['attrs']['style'])
                       .run())


class Root:
    def __init__(self, path: str, data: dict, home: Home):
        self.path_ = path
        self.home_ = home
        self.scene_ = data['attrs'].get('scene', 'unset')

        # Listen for updates to the scene property.
        self.home_.subscribe(self.path_, self.on_changed)

        # Acquire scenes defined on this home.
        nodes = self.home_.query(self.home_.path_to_query(self.path_) + ' [kind=scene]').run()
        self.scenes_ = {data['attrs']['name']: Scene(path, home) for path, data in nodes.items()}

    def on_changed(self, path: str, data: dict):
        # Always update when we poke the scene, even if the scene name is the same. This will flush
        # out any local changes to light state that have been made since the last application of
        # the scene.
        new_scene = data['attrs'].get('scene', 'unset')
        if new_scene not in self.scenes_:
            log.warning("cannot apply unknown scene named: {}".format(new_scene))
            return

        self.scenes_[new_scene].apply()


def main():
    parser = argparse.ArgumentParser(description='Map scene changes to light changes.')
    util.add_common_args(parser)
    args = parser.parse_args()

    util.enable_logging(args.log_target, args.log_level)
    with util.run_thread(Home((3, 0), RLock())) as home:
        nodes = home.query('[kind="home"]').run()
        roots = [Root(path, data, home) for path, data in nodes.items()]

        util.wait_for_exit(args.daemonize, globals(), locals())


if __name__ == '__main__':
    main()
