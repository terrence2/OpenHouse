# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from util import run_server, make_connection
import oh_shared.db as db
import asyncio
import pytest


@pytest.mark.asyncio
async def test_basic_formula_correctness():
    """
    Use a formula to ensure that the basic use works.
    """
    with run_server():
        async with make_connection() as tree:
            expect = None

            def on_result_changed(path, _, context):
                assert expect == (path, context)

            await tree.create_formula("/", "result", {'a0': "/arg0", 'a1': "/arg1"},
                                "(+ a0 a1)")
            with pytest.raises(db.FormulaInputNotFound):
                await tree.get_file("/result")

            await tree.subscribe("/result", on_result_changed)

            await tree.create_file("/", "arg0")
            with pytest.raises(db.FormulaInputNotFound):
                tree.get_data("/result")

            await tree.create_file("/", "arg1")
            expect = ("/result", "")
            await tree.get_data("/result")

            expect = ("/result", "foo")
            await tree.set_file("/arg0", "foo")
            assert await tree.get_file("/result") == "foo"

            expect = ("/result", "foobar")
            await tree.set_file("/arg1", "bar")
            assert await tree.get_file("/result") == "foobar"

            expect = ("/result", "bazbar")
            await tree.set_file("/arg0", "baz")
            assert await tree.get_file("/result") == "bazbar"
