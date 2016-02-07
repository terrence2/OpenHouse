#!/usr/bin/env python3
"""
Simple example of a syntax-highlighted HTML input line.
"""
import asyncio

from oh_shared.args import parse_default_args
from oh_shared.log import enable_logging
from oh_shared.home import Home

#from prompt_toolkit.contrib.repl import embed
#from bs4 import BeautifulSoup as bs


@asyncio.coroutine
def main():
    args = parse_default_args('Respond to sunrise and sunset states with dramatic fades.')
    enable_logging(args.log_target, args.log_level)
    home = yield from Home.connect((args.home_address, args.home_port))

    while True:
        query = input("> ")
        foo = yield from home.query(query).run()
        print(foo)

    #embed(globals(), locals(), vi_mode=True, history_filename=os.path.expanduser("~/.oh_history.txt"))


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
