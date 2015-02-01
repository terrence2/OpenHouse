#!/usr/bin/env python3
"""
Simple example of a syntax-highlighted HTML input line.
"""
import os.path

from threading import Lock

from shared.home import Home

from prompt_toolkit.contrib.repl import embed
from bs4 import BeautifulSoup as bs


def main():
    home = Home((3, 0), Lock())
    home.start()
    embed(globals(), locals(), vi_mode=True, history_filename=os.path.expanduser("~/.oh_history.txt"))
    home.exit()
    home.join()


if __name__ == '__main__':
    main()
