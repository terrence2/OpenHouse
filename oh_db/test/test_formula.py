# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from util import run_server, make_connection
import oh_shared.db as db
import asyncio
import pytest


@pytest.mark.asyncio
async def test_basic_formula_get():
    with run_server():
        async with make_connection() as tree:
            await tree.create_formula("/", "result", {}, '"Hello, World!"')
            data = await tree.get_file("/result")
            assert data == 'Hello, World!'


@pytest.mark.asyncio
async def test_basic_formula_stmt():
    with run_server():
        async with make_connection() as tree:
            await tree.create_formula("/", "result", {}, '(format "~s" 42)')
            data = await tree.get_file("/result")
            assert data == '42'


@pytest.mark.asyncio
async def test_basic_formula_types():
    with run_server():
        async with make_connection() as tree:
            await tree.create_formula("/", "result", {}, '42')
            with pytest.raises(db.FormulaTypeError):
                await tree.get_file("/result")


@pytest.mark.asyncio
async def test_basic_formula_missing_input():
    with run_server():
        async with make_connection() as tree:
            await tree.create_formula("/", "result", {'a0': '/a0'}, 'a0')
            with pytest.raises(db.FormulaInputNotFound):
                await tree.get_file("/result")


@pytest.mark.asyncio
async def test_basic_formula_no_assign():
    with run_server():
        async with make_connection() as tree:
            await tree.create_formula("/", "result", {}, '"foo"')
            with pytest.raises(db.NotFile):
                await tree.set_file("/result", "anything")


@pytest.mark.asyncio
async def test_basic_formula_input():
    with run_server():
        async with make_connection() as tree:
            await tree.create_file("/", "a0")
            await tree.set_file("/a0", "Hello, World!")
            await tree.create_formula("/", "result", {'a0': '/a0'}, 'a0')
            data = await tree.get_file("/result")
            assert data == "Hello, World!"


@pytest.mark.asyncio
async def test_formula_subscription_result():
    with run_server():
        async with make_connection() as tree:
            await tree.create_file("/", "a0")
            await tree.set_file("/a0", "Hello, World!")
            await tree.create_formula("/", "result", {'a0': '/a0'}, 'a0')

            count = 0
            expect = None

            async def on_result_changed(changes):
                nonlocal count
                assert expect == (count, changes)
                count += 1
            await tree.watch_matching_files("/result", on_result_changed)

            expect = (0, {"foobar": ["/result"]})
            await tree.set_file("/a0", "foobar")
            assert count == 1


@pytest.mark.asyncio
async def test_formula_subscription_all():
    with run_server():
        async with make_connection() as tree:
            await tree.create_file("/", "a0")
            await tree.set_file("/a0", "Hello, World!")
            await tree.create_formula("/", "result", {'a0': '/a0'}, 'a0')

            count = 0
            expect = None

            async def on_result_changed(changes):
                nonlocal count
                assert expect == (count, changes)
                count += 1
            await tree.watch_matching_files("/*", on_result_changed)

            expect = (0, {"foobar": ["/a0", "/result"]})
            await tree.set_file("/a0", "foobar")
            assert count == 1


@pytest.mark.asyncio
async def test_formula_multi_input():
    """
    Use a formula to ensure that the basic use works.
    """
    with run_server():
        async with make_connection() as tree:
            expect = None
            count = 0

            async def on_result_changed(changes):
                nonlocal count
                assert expect == (count, changes)
                count += 1

            await tree.create_formula("/", "result", {'a0': "/arg0", 'a1': "/arg1"},
                                      '(join "" a0 a1)')
            with pytest.raises(db.FormulaInputNotFound):
                await tree.get_file("/result")

            await tree.watch_matching_files("/*", on_result_changed)

            await tree.create_file("/", "arg0")
            with pytest.raises(db.FormulaInputNotFound):
                await tree.get_file("/result")

            await tree.create_file("/", "arg1")
            await tree.get_file("/result")

            expect = (0, {"foo": ["/arg0", "/result"]})
            await tree.set_file("/arg0", "foo")
            assert await tree.get_file("/result") == "foo"

            expect = (1, {"bar": ["/arg1"], "foobar": ["/result"]})
            await tree.set_file("/arg1", "bar")
            assert await tree.get_file("/result") == "foobar"

            expect = (2, {"baz": ["/arg0"], "bazbar": ["/result"]})
            await tree.set_file("/arg0", "baz")
            assert await tree.get_file("/result") == "bazbar"


@pytest.mark.asyncio
async def test_formula_nested():
    with run_server():
        async with make_connection() as tree:
            expect = None
            count = 0

            async def on_result_changed(changes):
                nonlocal count
                assert expect == (count, changes)
                count += 1

            await tree.create_file("/", "a")
            await tree.create_formula("/", "b", {"a": "/a"}, "a")
            await tree.create_formula("/", "c", {"b": "/b"}, "b")
            await tree.watch_matching_files("/{a,c}", on_result_changed)

            expect = (0, {"foobar": ["/a", "/c"]})
            await tree.set_file("/a", "foobar")
            assert count == 1
            assert await tree.get_file("/c") == "foobar"
