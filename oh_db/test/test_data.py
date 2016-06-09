# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from util import run_server, make_connection
import asyncio
import db
import pytest


@pytest.mark.asyncio
async def test_data_sync():
    with run_server():
        async with make_connection() as tree:
            await tree.create_key("/", "a", "flinfniffle")
            data = await tree.get_key("/", "a")
            assert data == "flinfniffle"
            await tree.remove_key("/", "a")


@pytest.mark.asyncio
async def test_data_async():
    with run_server():
        async with make_connection() as tree:
            ops = []
            for i in range(10):
                ops.append(await tree.create_key_async("/", str(i)))
            await asyncio.gather(*ops)

            keys_future = await tree.list_keys_async("/")
            response = await keys_future
            assert "".join(sorted(response['keys'])) == "0123456789"

            ops = []
            for i in range(10):
                ops.append(await tree.remove_key_async("/", "a"))
            await asyncio.gather(*ops)


@pytest.mark.asyncio
async def test_subscribe_keys():
    with run_server():
        async with make_connection() as tree:
            count = 0
            future = asyncio.Future()

            async def on_change(path: str, event: str, name: str):
                nonlocal count, future
                assert path == "/"
                assert (event == "Create" and count == 0) or (event == "Remove" and count == 1)
                assert name = "a"
                count += 1
                future.set_result(...)

            tree.subscribe_keys("/", on_change)

            future = asyncio.Future()
            await tree.create_key("/", "a", "foo")
            await future
            assert count == 1

            future = asyncio.Future()
            await tree.remove_key("/", "a")
            await future
            assert count == 2


@pytest.mark.asyncio
async def test_data_errors():
    with run_server():
        async with make_connection() as tree:
            with pytest.raises(db.NoSuchKey):
                await tree.get_key("/", "a")


