#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from html.parser import HTMLParser
from oh_shared.args import add_common_args
from oh_shared.db import Connection, Tree
from oh_shared.log import enable_logging
from pathlib import Path
import argparse
import asyncio
import logging
import yaml
from pprint import pprint


log = logging.getLogger("oh_populate")


class ConfigParser(HTMLParser):
    def __init__(self, tree: Tree):
        super().__init__()
        self.loop = asyncio.get_event_loop()
        self.tree = tree
        self.path = Path('/')
        self.tasks = []

    def handle_starttag(self, tag, attrs):
        if tag == 'html':
            assert self.path == Path('/'), "at path {}".format(self.path)
            return

        attrs = dict(attrs)
        name = tag
        if 'name' in attrs:
            name = attrs['name']
            del attrs['name']

        log.info("creating directory: {}".format(self.path))
        self.tasks.append(self.loop.create_task(self.tree.create_directory(str(self.path), name)))
        self.path /= name

        for attr, value in attrs.items():
            log.info("creating file: {} => {}".format(self.path / attr, value))
            # HTTP allows for flag attributes. Coerce these to bool.
            if value is None:
                value = "1"
            self.tasks.append(self.loop.create_task(self.tree.create_file(str(self.path), attr)))
            self.tasks.append(self.loop.create_task(self.tree.set_file_content(str(self.path / attr), value)))


    def handle_endtag(self, tag):
        if tag == 'html' or tag == '':
            assert self.path == Path('/'), "at path {}".format(self.path)
            return
        self.path = self.path.parent
        #print("Now at: {}".format(self.path))

    def handle_data(self, data):
        #print("found data: {}".format(data))
        pass


async def slurp_config(tree: Tree, parent_path: str, config: dict):
    for key, value in config.items():
        assert '/' not in key, "invalid path component"
        assert '*' not in key, "invalid path component"
        assert '?' not in key, "invalid path component"
        path = str(Path(parent_path) / key)
        if isinstance(value, dict):
            await tree.create_directory(parent_path, key)
            await slurp_config(tree, path, value)
        else:
            await tree.create_file(parent_path, key)
            await tree.set_file_content(path, str(value))


async def main():
    parser = argparse.ArgumentParser(description="Inject configuration into a pristine database.")
    add_common_args(parser)
    parser.add_argument("--config", type=str, metavar="FILE",
                        help="The configuration to load.")
    args = parser.parse_args()

    if not args.config:
        raise Exception("A configuration file is required!")

    enable_logging(args.log_target, args.log_level)

    async with Connection((args.home_address, args.home_port),
                          args.ca_chain, args.certificate, args.private_key) as tree:
        assert await tree.list_directory("/") == [], "tree must be empty when starting"

        with open(args.config, "r", encoding="utf-8") as fp:
            if args.config.endswith("html"):
                config_parser = ConfigParser(tree)
                config_parser.feed(fp.read())
                await asyncio.gather(*config_parser.tasks)
            else:
                assert args.config.endswith("yaml")
                config = yaml.load(fp)
                await slurp_config(tree, "/", config)



if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
