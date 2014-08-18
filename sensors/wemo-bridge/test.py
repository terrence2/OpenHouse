#!/usr/bin/env python3
from collections import namedtuple
from datetime import datetime, timedelta
from http.server import HTTPServer, BaseHTTPRequestHandler
from lxml import objectify
from queue import Queue
from socketserver import ThreadingUDPServer, BaseRequestHandler, TCPServer
from threading import Thread, Lock
from urllib.parse import urlparse, urljoin, urlunparse

import http.client
import logging
import select
import socket
import time

from pprint import pprint


def enable_logging(level):
    # File logger captures everything.
    fh = logging.FileHandler('bridge.log')
    fh.setLevel(logging.DEBUG)

    # Console output level is configurable.
    ch = logging.StreamHandler()
    ch.setLevel(getattr(logging, level))

    # Set an output format.
    formatter = logging.Formatter('%(asctime)s:%(levelname)s:%(name)s:%(message)s')
    ch.setFormatter(formatter)
    fh.setFormatter(formatter)

    # Add handlers to root.
    root = logging.getLogger('')
    root.setLevel(logging.DEBUG)
    root.addHandler(ch)
    root.addHandler(fh)
enable_logging('DEBUG')

log = logging.getLogger('wemo')
gil = Lock()

"""
Part 0
======
Acquire devices by broadcasting every 60 seconds or so and processing return calls.
Devices which have responded "recently" are present, devices that have not are missing.

Grab services file.
Then make a request with a callback address to the right "service".
Need an http server I guess.
"""


def get_own_external_ip_slow():
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect(('8.8.8.8', 80))
        return s.getsockname()[0]
    except socket.error:
        return None
    finally:
        # Don't wait around for the GC.
        s.close()
        del s


class FormatException(Exception):
    pass


class FakeHandler(BaseRequestHandler):
    def handle(self):
        pass


def parse_http_to_headers(raw_request: bytes) -> {}:
    """
    Parse some request text and pull out the headers.
    """
    request = raw_request.decode('UTF-8')
    lines = [line.strip() for line in request.split('\n')]
    status_line, headers = lines[0], lines[1:]

    if '200 OK' not in status_line:
        raise FormatException("Expected 200 OK, got: " + status_line)

    out = {}
    for line in headers:
        name, _, value = line.partition(':')
        if name:
            out[name.strip().lower()] = value.strip()

    return out


def map_hostname_to_local_ip(hostname: str) -> str:
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect((hostname, 80))
        return s.getpeername()[0]
    except socket.error:
        return None
    finally:
        # Don't wait around for the GC.
        s.close()
        del s


class WeMoDeviceAction:
    Argument = namedtuple('Argument', ('name', 'direction', 'state_variable'))

    def __init__(self, xml):
        self.name_ = str(xml.name)
        self.arguments_ = {}
        if hasattr(xml, 'argumentList') and hasattr(xml.argumentList, 'argument'):
            for node in xml.argumentList.argument:
                if hasattr(node, 'name'):
                    name = str(node.name)
                    direction = str(node.direction)
                    state_variable = str(node.relatedStateVariable)
                    self.arguments_[name] = self.Argument(name, direction, state_variable)

    def __str__(self):
        args = ["{}:{}".format(arg.name, arg.direction) for arg in self.arguments_.values()]
        return "{}({})".format(self.name_, ', '.join(args))


class WeMoDeviceStateVariable:
    def __init__(self, xml):
        self.name_ = str(xml.name)
        self.data_type_ = str(xml.dataType)
        self.default_value_ = str(xml.defaultValue)

    def __str__(self):
        return "{0.name_}:{0.data_type_}={0.default_value_}".format(self)


class WeMoDeviceService:
    def __init__(self, device, serviceType: str, serviceId: str, eventSubURL: str, SCPDURL: str):
        self.device_ = device

        # Data from setup.xml.
        self.service_type = serviceType
        self.service_id = serviceId
        self.event_url = urlparse(urljoin(device.http_location, eventSubURL))
        self.scpd_url = urlparse(urljoin(device.http_location, SCPDURL))

        """
        # Data from scpd xml.
        self.spec_version = (0, 0)
        self.actions = {}
        self.state_vars = {}

    def periodic_update(self, updater):
        log.info("Fetching {} for {} at {}".format(self.service_id, self.device_.friendly_name, urlunparse(self.scpd_url)))
        conn = http.client.HTTPConnection(self.scpd_url.hostname, self.scpd_url.port)
        conn.request("GET", self.scpd_url.path)
        res = conn.getresponse()
        data = res.read()
        xml = objectify.fromstring(data.decode('UTF-8'))

        self.spec_version = (int(xml.specVersion.major), int(xml.specVersion.minor))

        for node in xml.actionList.action:
            name = str(node.name)
            if name not in self.actions:
                self.actions[name] = WeMoDeviceAction(node)

        for node in xml.serviceStateTable.stateVariable:
            name = str(node.name)
            if name not in self.state_vars:
                self.state_vars[name] = WeMoDeviceStateVariable(node)
        """

    def subscribe(self):
        log.info("Sending SUBSCRIBE to {}".format(urlunparse(self.event_url)))
        conn = http.client.HTTPConnection(self.event_url.hostname, self.event_url.port)
        callback = "<http://{}:{}>".format(get_own_external_ip_slow(), 8989)
        conn.request("SUBSCRIBE", self.event_url.path, headers={'NT': 'upnp:event', 'CALLBACK': callback})
        res = conn.getresponse()
        data = res.read()
        if res.status != 200:
            log.error("SUBSCRIBE to {} FAILED: status {}".format(self.device_.friendly_name, res.status))
            return

        raw_timeout = res.getheader('TIMEOUT')
        if not raw_timeout.startswith('Second-'):
            log.error("SUBSCRIBE to {}: unexpected TIMEOUT, does not start with Second-: {}".format(
                self.device_.friendly_name, raw_timeout))
            return

        timeout = timedelta(seconds=int(raw_timeout[len('Second-'):]))
        sid = res.getheader('SID')

        print("subscription result:\n\ttimeout: {}\n\tsid: {}".format(timeout, sid))


class WeMoDeviceState:
    NS = "{urn:schemas-upnp-org:event-1-0}"

    def __init__(self, hostname: str):
        self.hostname = hostname
        self.hostip = map_hostname_to_local_ip(hostname)

        # UPnP properties.
        self.upnp_last_update = datetime.now() - timedelta(weeks=52)
        self.upnp_usn = ''
        self.http_location = ''
        self.http_location_url = None
        self.max_age = timedelta(seconds=0)

        # Device properties from setup.xml.
        self.setup_last_update = datetime.now() - timedelta(weeks=52)
        self.spec_version = (0, 0)
        self.friendly_name = ''
        self.manufacturer = ''
        self.model_description = ''
        self.model_name = ''
        self.model_number = ''
        self.model_url = ''
        self.serial_number = ''
        self.udn = ''
        self.upc = ''
        self.mac_address = ''
        self.firmware_version = ''

        # The services list.
        self.services = {}

    def update_upnp_info(self, headers: {str: str}):
        now = datetime.now()
        since_update = now - self.upnp_last_update
        if since_update < self.max_age:
            log.debug("Skipping upnp update for {} because {} < max-age({})".format(
                self.hostname, since_update, self.max_age))
            return

        self.upnp_last_update = now
        self.upnp_usn = headers['usn']
        self.http_location = headers['location']
        self.http_location_url = urlparse(self.http_location)
        max_age, _, max_age_value = headers['cache-control'].partition('=')
        assert max_age.strip() == 'max-age'
        self.max_age = timedelta(seconds=int(max_age_value.strip()))

    def periodic_update(self, updater):
        assert self.max_age != 0  # Ensure we've gotten at least one upnp update.

        now = datetime.now()
        since_update = now - self.setup_last_update
        if since_update < self.max_age:
            log.debug("Skipping setup.xml update for {} because {} < max-age({})".format(
                self.friendly_name, since_update, self.max_age))
            return

        # Grab the services file.
        log.info("Fetching services.xml for {} at {}".format(self.hostname, self.hostip))
        conn = http.client.HTTPConnection(self.http_location_url.hostname, self.http_location_url.port)
        conn.request("GET", self.http_location_url.path)
        res = conn.getresponse()
        data = res.read()
        xml = objectify.fromstring(data.decode('UTF-8'))

        self.setup_last_update = now
        self.spec_version = (int(xml.specVersion.major), int(xml.specVersion.minor))
        self.friendly_name = str(xml.device.friendlyName)
        self.manufacturer = str(xml.device.manufacturer)
        self.model_description = str(xml.device.modelDescription)
        self.model_name = str(xml.device.modelName)
        self.model_number = str(xml.device.modelNumber)
        self.model_url = str(xml.device.modelURL)
        self.serial_number = str(xml.device.serialNumber)
        self.udn = str(xml.device.UDN)
        self.upc = str(xml.device.UPC)
        self.mac_address = str(xml.device.macAddress)
        self.firmware_version = str(xml.device.firmwareVersion)
        for node in xml.device.serviceList.service:
            key = str(node.serviceId).split(':')[-1]
            if key not in self.services:
                self.services[key] = WeMoDeviceService(self,
                                                       str(node.serviceType), str(node.serviceId),
                                                       str(node.eventSubURL), str(node.SCPDURL))
            #updater.background_update(self.services[key])

        # FIXME: new thread for this, or is it fine to subscribe immediately.
        self.get_service('basicevent1').subscribe()

    def get_service(self, name: str) -> WeMoDeviceService:
        return self.services[name]


class WeMoBackgroundUpdate(Thread):
    def __init__(self, lock: Lock):
        super().__init__()
        self.setDaemon(True)

        # The global consistency lock.
        self.lock_ = lock

        # The input queue.
        self.queue_ = Queue()

    def background_update(self, to_update):
        """
        The object to_update must have a |periodic_update| function.
        """
        self.queue_.put(to_update)

    def run(self):
        while True:
            to_update = self.queue_.get(True)
            if to_update is None:
                return
            with self.lock_:
                to_update.periodic_update(self)


class WeMoManager(Thread):
    """
    Uses UPnP and service descriptors to keep our list internal list of devices up-to-date.
    """
    def __init__(self, own_intranet_ip: str, lock: Lock):
        super().__init__()
        self.setDaemon(True)

        # The global interlock that keeps everyone seeing coherenet data.
        self.lock_ = lock

        # The set of tracked devices.
        self.devices_ = {}  # {host: WeMoDeviceState}

        # The UPnP update infrastructure.
        self.upnp_server_ = ThreadingUDPServer((own_intranet_ip, 54322), self.handle_upnp_response)
        self.upnp_server_thread_ = Thread(target=self.upnp_server_.serve_forever)
        self.upnp_server_thread_.daemon = True

        # The background update runner.
        self.updater_thread_ = WeMoBackgroundUpdate(self.lock_)

    def add_device(self, hostname: str):
        state = WeMoDeviceState(hostname)
        self.devices_[state.hostip] = state

    def device_by_hostname(self, hostname: str) -> WeMoDeviceState:
        for state in self.devices_.values():
            if state.hostname == hostname:
                return state
        return None

    def run(self):
        self.upnp_server_thread_.start()
        self.updater_thread_.start()

        while True:
            mcast_addr = ('239.255.255.250', 1900)
            request = '\r\n'.join(("M-SEARCH * HTTP/1.1",
                                   "HOST:{}:{}",
                                   "ST:upnp:rootdevice",
                                   "MX:2",
                                   'MAN:"ssdp:discover"',
                                   "", "")).format(*mcast_addr)
            log.info("SENDING UPnP M-SEARCH")
            self.upnp_server_.socket.sendto(request.encode('UTF-8'), mcast_addr)
            time.sleep(60)

    def handle_upnp_response(self, request, client_address, server):
        raw_data = request[0]
        try:
            headers = parse_http_to_headers(raw_data)
        except FormatException as ex:
            log.exception(ex)
            return

        if headers.get('x-user-agent', None) != 'redsonic':
            log.debug("Found non-wemo device at: {}".format(client_address))
            return

        with self.lock_:
            state = self.devices_.get(client_address[0], None)
            if state is None:
                log.warning("found unrecognized wemo at {}".format(client_address))
                return
            state.update_upnp_info(headers)

        # Enqueue us for updating.
        self.updater_thread_.background_update(state)

        # The handler method is called from init, so the handler is totally extraneous.
        return FakeHandler(request, client_address, server)


class WeMoNotifyServer(Thread):
    """
    I hesitate to call this an http server as it doesn't even remotely attempt to be one; rather, it is a server that
    does as much HTTP as necessary to trick a WeMo into talking to it and makes no other concessions to portability.

    Ideally, the builtin http.server would "just work", but the WeMo's NOTIFY client has a few odd quirks that this
    particular implementation does not quite follow to the WeMo's satisfaction. In particular, it keeps HTTP/1.1
    connections alive and does not offer any mechanism to close them, causing the WeMo to RST the TCP connection and
    give up the subscription. Obviously HTTP/1.0 will close the connection, but the WeMo also gives a TCP RST here,
    presumably because HTTP/1.0.

    Whatevs, a trivial, single-purpose http server really is trivial. In theory doing it this way to should also give
    lower latency and lower overhead as all the WeMo really wants is a 200 OK -- most servers insist on sending
    advertising headers and other unnecessary pomp with every response.
    """
    def __init__(self, address: (str, int), lock: Lock):
        super().__init__()
        self.setDaemon(True)

        self.lock_ = lock

        self.sock_ = socket.socket()
        self.sock_.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock_.bind(address)

    def run(self):
        self.sock_.listen(128)
        while True:
            peer, peer_addr = self.sock_.accept()
            # FIXME: in theory all we need is the processing under the gil, so we could do all of the http work out of
            # the gil and queue up results elsewhere. We still need the queue though, so we might as well use the network
            # buffers as the queue until we have throughput issues.
            with self.lock_:
                result = self.process_one_connection(peer, peer_addr)
                peer.close()

                if result is not None:
                    self.handle_result(*result)

    def process_one_connection(self, peer: socket.socket, peer_addr: (str, int)):
        log.debug("Receiving connection from: {}".format(peer_addr))

        # Decode as UTF-8, as we know it will be.
        data = peer.recv(1024)
        data = data.decode('UTF-8')
        lines = data.split('\r\n')

        # Verify status line is sane.
        status = lines.pop(0)
        if status != 'NOTIFY / HTTP/1.1':
            log.warning("Unrecognized status line ({}) from {}".format(status, peer_addr))
            return

        # Extract headers.
        headers = {}
        line = lines.pop(0).strip()
        while line:
            key, _, value = line.partition(':')
            headers[key.strip()] = value.strip()
            line = lines.pop(0).strip()

        # Verify headers.
        for key in ['NT', 'NTS', 'SEQ', 'SID', 'CONTENT-LENGTH', 'CONTENT-TYPE']:
            if key not in headers:
                log.warning('Missing {} header from {}'.format(key, peer_addr))
                return
        for key, expect in [('NT', 'upnp:event'), ('NTS', 'upnp:propchange'), ('CONTENT-TYPE', 'text/xml; charset="utf-8"')]:
            if headers[key] != expect:
                log.warning('Unexpected {} header: got {}, expected {}'.format(key, headers[key], expect))
                return

        # The wemo sends the headers in a separate write from the body, so we might get the body after the headers.
        # I don't think these can be split further than headers/body, but I've handled the generic case here to be
        # relatively safe.
        content_length = int(headers['CONTENT-LENGTH'])
        body = '\r\n'.join(lines).encode('UTF-8')

        # Now that we've spent a few cycles processing, eagerly try another read if needed.
        if len(body) < content_length:
            log.debug("Got short initial read, optimistic retry with {} of {} bytes".format(len(body), content_length))
            body += peer.recv(1024)

        # If we still don't have what we need, wait for more: the upnp timeout is 30 seconds.
        # FIXME: if we hit this often, we'll need to start a thread for this.
        if len(body) < content_length:
            log.debug("Got short reads, using select to retry with {} of {} bytes".format(len(body), content_length))
            ready, _, _ = select.select([peer], [], [], 30)
            if len(ready) == 0:
                log.critical("Short read after long wait on {}: FIXME thread this if we see many of these".format(peer_addr))
                return
            body += peer.recv(1024)

        # NOTE!!: the WeMo is wrong about the content-length is sends (too short by 1 byte),
        #         so we use < instead of != in this check.
        # If we still don't have a full read at this point, wtf?
        if len(body) < content_length:
            log.critical("Short read after select.select: have {}, want {}".format(len(body), content_length))
            return

        # Let the WeMo know we have what we need -- the caller will close the connection.
        peer.send(b'HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n')

        # Return successfully.
        sid = headers['SID']
        seq = headers['SEQ']
        body = body.decode('UTF-8').strip()
        return sid, seq, body

    def handle_result(self, sid, seq, body):
        log.info("Got result: {}, {}, {}".format(sid, seq, body))


class MyHTTPRequestHandler(BaseHTTPRequestHandler):
    def do_NOTIFY(self):
        assert self.command == 'NOTIFY'
        self.protocol_version = 'HTTP/1.1'

        if self.path != '/':
            log.warning("Received non-root request path from {}: {}".format(self.client_address, self.path))
            return

        data = self.rfile.read()
        print("Request version: {}".format(self.request_version))
        print("Response version: {}".format(self.protocol_version))
        pprint(data.decode('UTF-8'))

        out = b'<html><body><h1>200 OK</h1></body></html>'
        self.send_response(200)
        self.send_header('Connection', 'close')
        self.send_header('Content-Type', 'text/html')
        self.send_header('Content-Length', str(len(out)))
        self.end_headers()
        self.wfile.write(out)


def main():
    """
    http_server_ = HTTPServer(('', 8989), MyHTTPRequestHandler)
    http_server_thread_ = Thread(target=http_server_.serve_forever)
    http_server_thread_.daemon = True
    http_server_thread_.start()
    """
    http = WeMoNotifyServer(('', 8989), gil)
    http.start()

    manager = WeMoManager(get_own_external_ip_slow(), gil)
    manager.add_device('wemomotion-bedroom-desk')
    """
    manager.add_device('wemomotion-bedroom-south')
    manager.add_device('wemomotion-kitchen-sink')
    manager.add_device('wemomotion-kitchen-west')
    manager.add_device('wemomotion-livingroom-north')
    manager.add_device('wemomotion-office-desk')
    manager.add_device('wemomotion-office-east')
    manager.add_device('wemomotion-office-west')
    manager.add_device('wemomotion-utility-north')
    manager.add_device('wemoswitch-office-fountain')
    """
    manager.start()

    """
    state = manager.device_by_hostname('wemomotion-bedroom-desk')
    service = state.get_service('basicevent1')
    service.subscribe('BinaryState')

    class MyTCPHandler(BaseRequestHandler):
        def handle(self):
            self.data = self.request.recv(4096).strip()
            print("{} wrote:".format(self.client_address[0]))
            print(self.data)
            body = "<html><body><h1>200 OK</h1></body></html>"
            response = (
                '200 OK',
                'Connection: close',
                'Content-Type: text/html',
                'Content-Length: {}'.format(len(body)),
                '',
                body
            )
            self.request.send('\r\n'.join(response).encode('UTF-8'))
            self.request.close()

    server = TCPServer(('', 8989), MyTCPHandler)
    server.serve_forever()
    """

    http.join()








if __name__ == "__main__":
    main()
