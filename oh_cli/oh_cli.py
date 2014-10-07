#!/usr/bin/env python3
"""
Simple example of a syntax-highlighted HTML input line.
"""
from home import Home

from threading import Lock

from prompt_toolkit.contrib.repl import embed


def main():
    home = Home(Lock())
    home.start()
    embed(globals(), locals(), vi_mode=False)
    home.exit()
    home.join()


if __name__ == '__main__':
    main()
