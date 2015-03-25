# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import socket


def get_own_internal_ip_slow() -> str:
    """
    Discovering the active internal interface that new connections will get spawned on -- e.g. that local peers can
    (in typical networks) call back on -- is actually quite hard. We spawn a connection to an external resource and
    derive the internal network from that. A rather inelegant hack, but it gets the job done.
    """
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect(('8.8.8.8', 80))
        return s.getsockname()[0]
    except socket.error:
        return None
    finally:
        s.close()
        del s
