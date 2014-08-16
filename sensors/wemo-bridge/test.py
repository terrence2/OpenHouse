#!/usr/bin/env python3
from collections import namedtuple
from copy import deepcopy
from datetime import datetime, timedelta
from lxml import objectify
from queue import Queue
from socketserver import ThreadingUDPServer, BaseRequestHandler
from threading import Thread, Lock
from urllib.parse import urlparse, urljoin, urlunparse

import http.client
import logging
import socket
import time

from pprint import pprint


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
        self.event_suburl = eventSubURL
        self.scpd_url = urlparse(urljoin(device.http_location, SCPDURL))

        # Data from scpd xml.
        self.spec_version = (0, 0)
        self.actions = {}
        self.state_vars = {}

    def periodic_update(self, updater):
        log.info("Fetching {} for {} at {}".format(self.service_id, self.device_.friendly_name, urlunparse(self.scpd_url)))
        #print("Fetching {} for {} at {}".format(self.service_id, self.device_.friendly_name, urlunparse(self.scpd_url)))
        conn = http.client.HTTPConnection(self.scpd_url.hostname, self.scpd_url.port)
        conn.request("GET", self.scpd_url.path)
        res = conn.getresponse()
        data = res.read()
        xml = objectify.fromstring(data.decode('UTF-8'))

        if self.service_id == 'urn:Belkin:serviceId:basicevent1' and self.device_.friendly_name == 'wemomotion-bedroom-desk':
            print(data.decode('UTF-8'))

        self.spec_version = (int(xml.specVersion.major), int(xml.specVersion.minor))
        for node in xml.actionList.action:
            name = str(node.name)
            if name not in self.actions:
                self.actions[name] = WeMoDeviceAction(node)

        for node in xml.serviceStateTable.stateVariable:
            name = str(node.name)
            if name not in self.state_vars:
                self.state_vars[name] = WeMoDeviceStateVariable(node)


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
            updater.background_update(self.services[key])


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


def main():
    manager = WeMoManager(get_own_external_ip_slow(), gil)
    manager.add_device('wemomotion-bedroom-desk')
    manager.add_device('wemomotion-bedroom-south')
    manager.add_device('wemomotion-kitchen-sink')
    manager.add_device('wemomotion-kitchen-west')
    manager.add_device('wemomotion-livingroom-north')
    manager.add_device('wemomotion-office-desk')
    manager.add_device('wemomotion-office-east')
    manager.add_device('wemomotion-office-west')
    manager.add_device('wemomotion-utility-north')
    manager.add_device('wemoswitch-office-fountain')
    manager.start()

    time.sleep(2)

    """
    state = manager.device_by_hostname('wemomotion-bedroom-desk')
    print("bedroom-desk info is at: {}".format(state.http_location))
    """


if __name__ == "__main__":
    main()
