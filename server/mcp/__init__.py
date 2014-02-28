__author__ = 'terrence'

import logging


def enable_logging(level):
    log_level = getattr(logging, level)
    logging.basicConfig(
        format='%(asctime)s:%(levelname)s:%(name)s:%(message)s',
        filename='mcp-eventlog.log', level=log_level)
    log = logging.getLogger('home')
    return log

