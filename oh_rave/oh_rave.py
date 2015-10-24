#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import argparse
import asyncio
import json
import logging
import math
import random
from oh_shared.args import add_common_args
from oh_shared.log import enable_logging
from oh_shared.home import Home, NodeData

log = logging.getLogger('oh_rave')

COLOR_RED_BIAS = 0.1
COLOR_SIGMA = 0.2
COLOR_MAX_BLUE = 32


class Distribution:
    def __init__(self, name, min_, max_, **params):
        self.fn = getattr(random, name)
        self.min = min_
        self.max = max_
        self.params = params

    def sample(self):
        return max(self.min, min(self.max, self.fn(**self.params)))

    @classmethod
    def from_dict(cls, d: dict) -> "Distribution":
        return cls(d['name'], d['min'], d['max'], **d['parameters'])


class Config:
    def __init__(self, config: dict):
        self.transition_time = Distribution.from_dict(config['transition'])
        self.red = Distribution.from_dict(config['color']['red'])
        self.green = Distribution.from_dict(config['color']['green'])
        self.blue = Distribution.from_dict(config['color']['blue'])

    @classmethod
    def from_file(cls, filename: str) -> "Config":
        with open(filename, 'r') as fp:
            raw = json.load(fp)
        return cls(raw)


@asyncio.coroutine
def light_handler(home, path, config):
    tt = config.transition_time.sample()
    red = int(config.red.sample() * 255)
    green = int(config.green.sample() * 255)
    blue = int(config.blue.sample() * 255)
    print("LIGHT: {}, TT: {}; RGB({}, {}, {})".format(path, tt, red, green, blue))

    yield from home.query(home.path_to_query(path))\
        .css('animation-duration', str(math.floor(tt)))\
        .css('visibility', 'visible')\
        .css('color', 'RGB({},{},{})'.format(red, green, blue))\
        .run()

    yield from asyncio.sleep(tt)
    asyncio.async(light_handler(home, path, config))


@asyncio.coroutine
def main():
    parser = argparse.ArgumentParser(description="Party Time!")
    add_common_args(parser)
    parser.add_argument("-c", "--config", type=str, metavar="FILE",
                        help="JSON file describing party scenario.")
    args = parser.parse_args()

    if not args.config:
        raise Exception("A configuration file is required to start the party!")

    config = Config.from_file(args.config)

    enable_logging(args.log_target, args.log_level)
    home = yield from Home.connect((args.home_address, args.home_port))

    lights = yield from home.query('light').run()
    for path in lights.keys():
        asyncio.async(light_handler(home, path, config))


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
