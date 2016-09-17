# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import argparse
from .ip import get_own_internal_ip_slow


def add_common_args(parser: argparse.ArgumentParser):
    internal_ip = get_own_internal_ip_slow()
    group = parser.add_argument_group('OpenHouse Common Arguments')
    group.add_argument('--log-level', '-l', default='DEBUG',
                       help="The logging level. (default DEBUG)")
    group.add_argument('--log-target', '-L', default='events.log',
                       help="The logging target. (default events.log)")
    group.add_argument('--db-address', '-A', default=internal_ip, type=str,
                       help="The HOMe daemon's ipv4 address. (default {}".format(internal_ip))
    group.add_argument('--db-port', '-P', default=8887, type=int,
                       help="The HOMe daemon's ipv4 port.")
    group.add_argument('--ca-chain', '-C', type=str,
                       help="The private key of this daemon.")
    group.add_argument('--certificate', '-c', type=str,
                       help="The certificate of this daemon.")
    group.add_argument('--private-key', '-k', type=str,
                       help="The private key of this daemon.")


def parse_default_args(description: str) -> object:
    parser = argparse.ArgumentParser(description=description)
    add_common_args(parser)
    return parser.parse_args()


def make_parser(description: str) -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=description)
    add_common_args(parser)
    return parser
