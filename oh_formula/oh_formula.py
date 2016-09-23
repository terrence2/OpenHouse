#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from oh_shared.args import make_parser
from oh_shared.db import Connection, Tree
from oh_shared.log import enable_logging
from pathlib import Path
import ast
import asyncio
import logging
from collections import OrderedDict
from pathlib import PurePosixPath as Path


log = logging.getLogger("oh_formula")


class Formula:
    """
    Respond to changes to inputs by running code and setting the adjacent
    value to the result of the output.
    """
    class Input:
        def __init__(self, name: str, path: Path, value: str, sid: int):
            self.name = name
            self.path = path
            self.value = value
            self.sid = sid

    def __init__(self, target_path: Path, tree: Tree):
        self.target_path = target_path
        self.tree = tree
        self.inputs = OrderedDict()
        self.formula = None

    @classmethod
    async def create(cls, target_path: Path, raw_inputs: {str: Path}, code: str, tree: Tree):
        # Query initial values for every input.
        instance = cls(target_path, tree)
        for name, path in raw_inputs.items():
            value = await tree.get_file(str(path))
            sid = await tree.watch_matching_files(str(path), instance.on_input_changed)
            assert path not in instance.inputs,\
                "Duplicate input detected for {}: {}".format(target_path, path)
            instance.inputs[path] = cls.Input(name, path, value, sid)

        # Parse the code: this will become the body of the function we define below.
        body_module = ast.parse(code)

        # Create the input argument list.
        input_arg_list = ",".join(inp.name for inp in instance.inputs.values())

        # Parse an empty function declaration with the right arguments.
        function_module = ast.parse("def formula_func({}): pass".format(input_arg_list))

        # Replace the body of the function with the code from the formula.
        function_module.body[0].body = body_module.body

        # Compile the new AST into bytecode and evaluate it to produce a callable function.
        bytecode = compile(function_module, str(target_path), "exec")
        env = {}
        exec(bytecode, {}, env)

        # Store the function into our Formula instance for later calls.
        instance.formula = env['formula_func']

    async def on_input_changed(self, changes: {str: [str]}):
        for context, changed_paths in changes.items():
            # First, update our cached values.
            for changed_path in changed_paths:
                assert Path(changed_path) in self.inputs, "non-subscribed path in event"
                self.inputs[Path(changed_path)].value = context
                log.debug("Observed value of {} change to {}".format(changed_path, context))

            # Next, apply the formula with the new values.
            inputs = [inp.value for inp in self.inputs.values()]
            result = self.formula(*inputs)
            log.info("Formula {} input changed; new value is: {}".format(self.target_path, result))

            # And update the tree with the new value.
            await self.tree.set_file(str(self.target_path), result)


async def main():
    parser = make_parser("Process formulas and apply their results.")
    args = parser.parse_args()

    enable_logging(args.log_target, args.log_level)

    async with Connection.from_args(args) as tree:
        formula_matches = await tree.get_matching_files('/**/formula/*')
        formulas = []
        for path, value in formula_matches.items():
            target_path = Path(path).parent.parent / "value"
            kind = Path(path).name
            if kind == 'compute':
                inputs_glob = Path(path).parent / "inputs" / "*"
                inputs = await tree.get_matching_files(str(inputs_glob))
                inputs = {Path(k).name: Path(v) for k, v in inputs.items()}
                formulas.append(await Formula.create(target_path, inputs, value, tree))
            elif kind == 'same-as':
                inputs = {'_foo_': Path(value)}
                code = "return _foo_"
                formulas.append(await Formula.create(target_path, inputs, code, tree))
            else:
                log.warning("Unhandled formula kind: {}".format(kind))

        while True:
            try:
                await asyncio.sleep(500)
            except KeyboardInterrupt:
                return


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
