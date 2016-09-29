#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from oh_shared.db import Connection, Tree
from oh_shared.log import enable_logging
from oh_shared.args import make_parser
from pathlib import Path
import asyncio
import logging
import yaml


log = logging.getLogger("oh_populate")


async def slurp_config(tree: Tree, parent_path: str, config: dict):
    formulas = []
    for key, value in config.items():
        assert isinstance(key, str)
        assert '/' not in key, "invalid path component"
        assert '*' not in key, "invalid path component"
        assert '?' not in key, "invalid path component"
        path = str(Path(parent_path) / key)
        if isinstance(value, dict):
            if 'formula' in value and 'where' in value:
                formulas.append((parent_path, key, value['where'], value['formula']))
            else:
                await tree.create_directory(parent_path, key)
                formulas.extend(await slurp_config(tree, path, value))
        else:
            await tree.create_file(parent_path, key)
            await tree.set_file(path, str(value))
    return formulas


async def create_formulas(tree: Tree, formulas):
    """We have to create formulas after all normal nodes so that formula inputs are already present."""
    for (parent_path, key, inputs, formula) in formulas:
        await tree.create_formula(parent_path, key, inputs, formula)


async def main():
    parser = make_parser("Inject configuration into a pristine database.")
    parser.add_argument("--config", type=str, metavar="FILE",
                        help="The configuration to load.")
    args = parser.parse_args()

    if not args.config:
        raise Exception("A configuration file is required!")

    enable_logging(args.log_target, args.log_level)

    async with Connection.from_args(args) as tree:
        assert await tree.list_directory("/") == [], "tree must be empty when starting"

        with open(args.config, "r", encoding="utf-8") as fp:
            assert args.config.endswith("yaml")
            config = yaml.load(fp)
            formulas = await slurp_config(tree, "/", config)
            await create_formulas(tree, formulas)


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
