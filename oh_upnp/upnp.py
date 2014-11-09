# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

from queue import Queue, Empty
from socketserver import UDPServer, BaseRequestHandler
from threading import Thread, Lock


log = logging.getLogger("UPnP")


class _FormatException(Exception):
    pass


class _FakeHandler(BaseRequestHandler):
    def handle(self):
        pass


class UPnP(Thread):
    def __init__(self, listen_addr: (str, int), callback: callable, beacon_interval: int, lock: Lock):
        super().__init__()

        # Callback to call when we discover a UPnP device.
        self.callback_ = callback

        # Time to wait between M-SEARCH broadcasts.
        self.beacon_interval_ = beacon_interval

        # The global interlock that keeps everyone seeing coherent data.
        self.lock_ = lock

        # The i/o queue.
        self.queue_ = Queue()

        # The UPnP update infrastructure.
        self.upnp_server_ = UDPServer(listen_addr, self.handle_upnp_response)
        self.upnp_server_thread_ = Thread(target=self.upnp_server_.serve_forever)

    def exit(self):
        self.queue_.put(None)

    def run(self):
        self.upnp_server_thread_.start()

        while True:
            mcast_addr = ('239.255.255.250', 1900)
            request = '\r\n'.join(("M-SEARCH * HTTP/1.1",
                                   "HOST:{}:{}",
                                   "ST:upnp:rootdevice",
                                   "MX:2",
                                   'MAN:"ssdp:discover"',
                                   "", "")).format(*mcast_addr)
            log.info("Sending UPnP M-SEARCH broadcast")
            self.upnp_server_.socket.sendto(request.encode('UTF-8'), mcast_addr)
            try:
                message = self.queue_.get(block=True, timeout=self.beacon_interval_)
            except Empty:
                continue

            assert message is None
            self.upnp_server_.shutdown()
            self.upnp_server_thread_.join()
            return

    def handle_upnp_response(self, request, client_address, server):
        log.debug("Received reply to UPnP request from {}".format(client_address))
        raw_data = request[0]
        try:
            headers = self.parse_http_to_headers(raw_data)
        except _FormatException as ex:
            log.exception(ex)
            return

        with self.lock_:
            self.callback_(client_address, headers)

        # The handler method is called from init, so the handler is totally extraneous.
        return _FakeHandler(request, client_address, server)

    @staticmethod
    def parse_http_to_headers(raw_request: bytes) -> {}:
        """
        Parse some request text and pull out the headers.
        """
        request = raw_request.decode('UTF-8')
        lines = [line.strip() for line in request.split('\n')]
        status_line, headers = lines[0], lines[1:]

        if '200 OK' not in status_line:
            raise _FormatException("Expected 200 OK, got: " + status_line)

        out = {}
        for line in headers:
            name, _, value = line.partition(':')
            if name:
                out[name.strip().lower()] = value.strip()

        return out
