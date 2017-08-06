# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import argparse


def add_common_args(parser: argparse.ArgumentParser):
    group = parser.add_argument_group('OpenHouse Common Arguments')
    group.add_argument('--log-level', '-l', default='DEBUG',
                       help="The logging level. (default DEBUG)")
    group.add_argument('--log-target', '-L', default='events.log',
                       help="The logging target. (default events.log)")
    group.add_argument('--db-port', '-p', default=8182, type=int,
                       help="The db daemon's ipv4 port. (default: 8182)")


def parse_default_args(description: str) -> object:
    parser = argparse.ArgumentParser(description=description)
    add_common_args(parser)
    return parser.parse_args()


def make_parser(description: str) -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=description)
    add_common_args(parser)
    return parser
