#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from prompt_toolkit.interface import CommandLineInterface
from prompt_toolkit.shortcuts import create_prompt_application, create_asyncio_eventloop, prompt_async
import argparse
import asyncio
import contextlib
import os
import sys
import subprocess
from pathlib import Path

loop = asyncio.get_event_loop()

Daemons = ['hue', 'color', 'button', 'zwave']
DebugDaemons = ['rest']


@contextlib.contextmanager
def chdir(path: Path):
    pwd = os.getcwd()
    os.chdir(str(path))
    try:
        yield
    finally:
        os.chdir(pwd)


class GenericProcess:
    """
    A process that is tangentially related to running OH. This requires more arguments
    because we can infer less about the environment. These processes are blocking.
    Typically these are used for build tasks and other setup before OH begins running.
    """
    def __init__(self, name: str, command: str, pwd: Path):
        self.name = name
        self.command = command.split()
        self.pwd = pwd

    async def start(self):
        with chdir(self.pwd):
            rv = subprocess.run(self.command)
        assert rv.returncode == 0,\
            "GenericProcess({}) failed to run: {}".format(self.name, self.command)


class OpenHouseDatabase:
    """
    The main database process. This is split out because it has slightly different args
    because of immaterial difference with the standard python environment. Oh well.
    """
    def __init__(self, args):
        self.name = 'oh_db'
        self.command = [
            './oh_db/target/release/oh_db',
            '-l', 'debug' if args.verbosity >= 3 else 'info',
            '-L', args.logdir + '/oh_db.log',
            '-a', '127.0.0.1', '-p', str(args.db_port),
            '-C', 'CA/intermediate/certs/chain.cert.pem',
            '-c', 'CA/intermediate/certs/oh_db.cert.pem',
            '-k', 'CA/intermediate/private/oh_db.key.pem',
        ]

    async def start(self) -> asyncio.subprocess.Process:
        return await asyncio.create_subprocess_exec(*self.command)


class OpenHouseDaemon:
    """
    An OH daemon. Fully parameterized by the name. Runs in parallel with the database
    and other daemon processes.
    """
    def __init__(self, name: str, args):
        self.name = name
        self.command = [
            '{0}/{0}.py'.format(self.name),
            '-l', 'DEBUG' if args.verbosity >= 3 else 'INFO',
            '-L', args.logdir + '/{}.log'.format(self.name),
            '-A', '127.0.0.1', '-P', str(args.db_port),
            '-C', 'CA/intermediate/certs/chain.cert.pem',
            '-c', 'CA/intermediate/certs/{}.cert.pem'.format(self.name),
            '-k', 'CA/intermediate/private/{}.key.pem'.format(self.name),
        ]

    async def start(self) -> asyncio.subprocess.Process:
        return await asyncio.create_subprocess_exec(*self.command)


class OpenHouseProcess:
    """
    A process runs in the foreground and blocks until completion.
    """
    def __init__(self, name: str, extra: [str], args):
        self.daemon = OpenHouseDaemon(name, args)
        self.daemon.command += extra

    async def start(self):
        proc = await self.daemon.start()
        await proc.wait()
        return None


async def spawn(processes):
    """Create monitored subprocess tasks and return the handles."""
    managed = []
    for process in processes:
        task = await process.start()
        if task is not None:
            managed.append(task)
    return managed


async def print_counter():
    """
    Coroutine that prints counters.
    """
    i = 0
    while True:
        print('Counter: %i' % i)
        i += 1
        await asyncio.sleep(3)


async def interactive_shell():
    """
    Like `interactive_shell`, but doing things manual.
    """
    # Create an asyncio `EventLoop` object. This is a wrapper around the
    # asyncio loop that can be passed into prompt_toolkit.
    eventloop = create_asyncio_eventloop()

    # Create interface.
    cli = CommandLineInterface(
        application=create_prompt_application('Say something inside the event loop: '),
        eventloop=eventloop)

    # Patch stdout in something that will always print *above* the prompt when
    # something is written to stdout.
    sys.stdout = cli.stdout_proxy()

    # Run echo loop. Read text from stdin, and reply it back.
    while True:
        try:
            result = await cli.run_async()
            print('You said: "{0}"'.format(result.text))
        except (EOFError, KeyboardInterrupt):
            return


def main():
    parser = argparse.ArgumentParser(description="The OpenHouse Supervisor")
    parser.add_argument('--config', '-c', type=str,
                        help="The configuration to load.")
    parser.add_argument('--db-port', '-p', type=int, default=28184,
                        help="The port to start the database on.")
    parser.add_argument('--debug', '-d', type=bool, default=False,
                        help="Run extra debugging daemons.")
    parser.add_argument('--verbosity', '-v', type=int, default=1,
                        help="Run with verbosity level.")
    parser.add_argument('--logdir', '-L', type=str, default='log',
                        help="Choose where to put logs.")
    args = parser.parse_args()

    if not args.config:
        print("A configuration file is required!")
        return 1

    if args.debug:
        global Daemons
        Daemons += DebugDaemons

    # Launch the shell on a sub-loop.
    shell_task = loop.create_task(interactive_shell())

    # Build the supervisor tree.
    processes = [
        GenericProcess('compile-oh_db', 'cargo build --release', Path('./oh_db')),
        OpenHouseDatabase(args),
        OpenHouseProcess('oh_populate', ['--config', args.config], args),
    ] + [OpenHouseDaemon('oh_' + name, args) for name in Daemons]

    managed = loop.run_until_complete(spawn(processes))
    tasks = [p.wait() for p in managed]
    background_task = asyncio.gather(*tasks, return_exceptions=True)

    loop.run_until_complete(shell_task)
    for proc in managed:
        proc.terminate()
    loop.run_until_complete(background_task)
    print('Quiting event loop. Bye.')
    loop.close()

    return 0


if __name__ == '__main__':
    sys.exit(main())
