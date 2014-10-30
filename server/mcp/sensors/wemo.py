# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import contextlib
import errno
import http.client
import logging
import re
import requests
import requests.exceptions
import select
import socket

from collections import namedtuple
from datetime import datetime, timedelta
from lxml import objectify
from queue import Queue, Empty
from socketserver import ThreadingUDPServer, BaseRequestHandler
from threading import Thread, Lock
from urllib.parse import urlparse, urljoin, urlunparse

from mcp.network import Bus as NetworkBus
from mcp.scheduler import Scheduler
from mcp.sensors import Sensor, MotionEvent, DefunctEvent


log = logging.getLogger('wemo-sensor')

'''
class WeMoSensorBridge:
    """
    Interfaces with the zmq bridge to get the insane UPnP, Python2 world the wemo lives in.
    """
    ReplyPort = 31978
    BroadcastPort = 31979

    def __init__(self, address_without_port: str):
        self.address_without_port = address_without_port

        # When we need to get a device's state, we want to do so synchronously.
        # Make a separate connection for our own sync use in addition to the
        # one made by network.Bus.
        self.ctx = zmq.Context()
        self.request_socket = self.ctx.socket(zmq.REQ)
        req_address = "tcp://{}:{}".format(self.address_without_port, self.ReplyPort)
        log.debug("connecting for sync reply at: {}".format(req_address))
        self.request_socket.connect(req_address)

        # All sensor messages arrive via the bridge, so we need to process the messages
        # on the bridge and dispatch from there. Otherwise, each WeMoDevice has to get
        # every message from the network and do its own filtering. Instead we just attach
        # the bridge and have it do dispatch.
        self.devices = {}  # {name: WeMoMotion}
        self.name = self.address_without_port
        self.address = (self.address_without_port, self.BroadcastPort)
        self.remote = network.Sensor(self)

    def on_message(self, wrapper: object):
        source = wrapper['source']
        message = wrapper['message']
        log.debug("got message for {}: {}".format(source, message))
        try:
            self.devices[source].on_message(message)
        except KeyError:
            log.warning("unknown device: {}".format(source))

    def add_device(self, device: Sensor) -> Sensor:
        self.devices[device.name] = device
        return device

    def get_state(self, name: str) -> bool:
        log.debug("getting state for {}".format(name))
        self.request_socket.send_json({'target': name, 'type': 'get_state'})
        data = self.request_socket.recv_json()
        log.debug("state of {} is {}".format(name, data['state']))
        return bool(data['state'])


class WeMoMotion(Sensor):
    def __init__(self, hostname: str, bridge: WeMoSensorBridge):
        super().__init__(hostname)
        self.bridge_ = bridge
        self.motion_listener_ = self.default_motion_listener_

    def default_motion_listener_(self, event: MotionEvent):
        log.warning('ignoring motion from {}: {}'.format(self.name, event.value))

    def get_state(self) -> bool:
        return self.bridge_.get_state(self.name)

    def listen_motion(self, listener: callable):
        self.motion_listener_ = listener

    def on_message(self, message: object):
        self.motion_listener_(MotionEvent(bool(message['state'])))


class WeMoSwitch(Sensor):
    """
    The sensor half of the switch. Receives updates on the switch's state.
    """
    def __init__(self, hostname: str, bridge: WeMoSensorBridge):
        super().__init__(hostname)
        self.bridge_ = bridge
        self.state_listener_ = self.default_switch_listener_

    def default_switch_listener_(self, event: SwitchEvent):
        log.warning('ignoring motion from {}: {}'.format(self.name, event.value))

    def get_state(self) -> bool:
        return self.bridge_.get_state(self.name)

    def listen_switch_state(self, listener: callable):
        self.state_listener_ = listener

    def on_message(self, message: object):
        self.state_listener_(SwitchEvent(bool(message['state'])))
'''


class FormatException(Exception):
    pass


class _FakeHandler(BaseRequestHandler):
    def handle(self):
        pass


class _WeMoDeviceAction:
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


class _WeMoDeviceStateVariable:
    def __init__(self, xml):
        self.name_ = str(xml.name)
        self.data_type_ = str(xml.dataType)
        self.default_value_ = str(xml.defaultValue)

    def __str__(self):
        return "{0.name_}:{0.data_type_}={0.default_value_}".format(self)


@contextlib.contextmanager
def auto_unlock(lock: Lock):
    lock.release()
    try:
        yield
    finally:
        lock.acquire()


class _WeMoDeviceService:
    SUBSCRIBE_TIMEOUT = 20  # seconds
    TIMEOUT_RETRY_INTERVAL = timedelta(seconds=2 * 60)
    ERROR_RETRY_INTERVAL = timedelta(seconds=5 * 60)

    def __init__(self, device, service_type: str, service_id: str, event_suburl: str, scpd_url: str):
        self.device_ = device

        # Data from setup.xml.
        self.service_type = service_type
        self.service_id = service_id
        self.event_url = urlparse(urljoin(device.http_location, event_suburl))
        self.scpd_url = urlparse(urljoin(device.http_location, scpd_url))

        # Data from scpd xml.
        # Note: we don't really need any of the spec data. It appears to be of questionable accuracy in any case.
        self.spec_version = (0, 0)
        self.actions = {}
        self.state_vars = {}

        # Subscription state.
        self.is_subscribed = False

    def load_spec_data(self):
        """
        Yes, this is unused at the moment, but we might as well keep working code in case we want to do more with this
        at some later time.
        """
        self.spec_version = (0, 0)
        self.actions = {}
        self.state_vars = {}

        log.info("Fetching {} for {} at {}".format(self.service_id, self.device_.friendly_name,
                                                   urlunparse(self.scpd_url)))
        conn = http.client.HTTPConnection(self.scpd_url.hostname, self.scpd_url.port)
        conn.request("GET", self.scpd_url.path)
        res = conn.getresponse()
        data = res.read()
        xml = objectify.fromstring(data.decode('UTF-8'))

        self.spec_version = (int(xml.specVersion.major), int(xml.specVersion.minor))

        for node in xml.actionList.action:
            name = str(node.name)
            if name not in self.actions:
                self.actions[name] = _WeMoDeviceAction(node)

        for node in xml.serviceStateTable.stateVariable:
            name = str(node.name)
            if name not in self.state_vars:
                self.state_vars[name] = _WeMoDeviceStateVariable(node)

    @staticmethod
    def make_subscribe_closure(self_inner, scheduler_inner):
        # Don't close over our nested call chain.
        def callback():
            self_inner.subscribe(scheduler_inner)
        return callback

    @staticmethod
    def make_resubscribe_closure(self_inner, scheduler_inner, sid_inner):
        # Don't close over our nested call chain.
        def callback():
            self_inner.resubscribe(scheduler_inner, sid_inner)
        return callback

    def subscribe(self, scheduler: Scheduler):
        """
        Note that in UPnP, SUBSCRIBE subscribes to /everything/ all the time, so there is no point not just doing it
        automatically.
        """
        log.info("Sending SUBSCRIBE to {}".format(urlunparse(self.event_url)))
        assert not self.is_subscribed
        try:
            with auto_unlock(self.device_.manager.lock_):
                res = requests.request('SUBSCRIBE', urlunparse(self.event_url), timeout=self.SUBSCRIBE_TIMEOUT,
                                       stream=False, allow_redirects=False,
                                       headers={'NT': 'upnp:event', 'CALLBACK': self.device_.manager.callback_address})
        except requests.exceptions.Timeout as ex:
            log.error("timed out trying to SUBSCRIBE")
            log.exception(ex)
            return self._handle_subscribe_failure(scheduler, self.TIMEOUT_RETRY_INTERVAL)
        except requests.exceptions.ConnectionError as ex:
            log.error("failed to connect to {} for SUBSCRIBE".format(urlunparse(self.event_url)))
            log.exception(ex)
            return self._handle_subscribe_failure(scheduler, self.ERROR_RETRY_INTERVAL)
        if res.status_code != 200:
            log.error("SUBSCRIBE to {} FAILED: status {}".format(self.device_.friendly_name, res.status_code))
            return self._handle_subscribe_failure(scheduler, self.ERROR_RETRY_INTERVAL)

        if self.device_.is_defunct:
            self.device_.set_defunct(False)
        self.is_subscribed = True

        self._setup_automatic_resubscribe(scheduler, res)

    def resubscribe(self, scheduler: Scheduler, sid: str):
        log.info("Sending RESUBSCRIBE to {} for {}".format(urlunparse(self.event_url), sid))
        try:
            with auto_unlock(self.device_.manager.lock_):
                res = requests.request('SUBSCRIBE', urlunparse(self.event_url), timeout=self.SUBSCRIBE_TIMEOUT,
                                       stream=False, allow_redirects=False,
                                       headers={'SID': sid})
        except requests.exceptions.Timeout as ex:
            log.error("timed out trying to re-SUBSCRIBE")
            log.exception(ex)
            return self._handle_subscribe_failure(scheduler, self.TIMEOUT_RETRY_INTERVAL)
        except requests.exceptions.ConnectionError as ex:
            log.error("failed to connect to {} for re-SUBSCRIBE".format(urlunparse(self.event_url)))
            log.exception(ex)
            return self._handle_subscribe_failure(scheduler, self.ERROR_RETRY_INTERVAL)

        # Code 412 is for invalid sid. Flush and start over.
        if res.status_code == 412:
            log.error("RESUBSCRIBE got 412 (invalid sid) back: restarting from SUBSCRIBE")
            if self.unsubscribe(sid, scheduler):
                self.subscribe(scheduler)
            return

        if res.status_code != 200:
            log.error("SUBSCRIBE to {} FAILED: status {}".format(self.device_.friendly_name, res.status_code))
            return

        if self.device_.is_defunct:
            self.device_.set_defunct(False)
        self.is_subscribed = True

        self._setup_automatic_resubscribe(scheduler, res)

    def _handle_subscribe_failure(self, scheduler: Scheduler, retry_time: timedelta):
        # FIXME: retry a few times before setting the device as defunct.
        self.device_.set_defunct(True)
        scheduler.set_timeout(retry_time, self.make_subscribe_closure(self, scheduler))

        """
        self.timeout_state_.timed_out()
        if self.timeout_state_.defunct():
            # send defunct notice on device.
            # maybe send present notice to device as well, so that we don't shut the lights off inadvertently?
            # although defuct could handle that.
        else:
            # try to resubscribe in a few seconds.
            scheduler.set_timeout(5, self.make_subscribe_closure(self, scheduler))
        """

    def _setup_automatic_resubscribe(self, scheduler: Scheduler, res: requests.Response):
        raw_timeout = res.headers['timeout']
        if not raw_timeout.startswith('Second-'):
            log.error("SUBSCRIBE to {}: unexpected TIMEOUT, does not start with Second-: {}".format(
                self.device_.friendly_name, raw_timeout))
            return

        timeout = timedelta(seconds=int(raw_timeout[len('Second-'):]))
        sid = res.headers['sid']

        log.info("subscription result: timeout: {}; sid: {}".format(timeout, sid))
        # TODO: Test this thoroughly. We need to go at least |timeout - 30 * numDevices| before timeout in order to be
        # TODO:    guaranteed to hit our resubscribe interval. In practice we're probably fine with just using 5 min
        # TODO:    or something and not plumbing that number all the way down here.
        #time_to_resubscribe = timedelta(seconds=60)
        time_to_resubscribe = timeout - timedelta(seconds=30 * 10)
        scheduler.set_timeout(time_to_resubscribe, self.make_resubscribe_closure(self, scheduler, sid))

    def unsubscribe(self, sid: str, scheduler: Scheduler) -> bool:
        log.info("Sending UNSUBSCRIBE to {} for {}".format(urlunparse(self.event_url), sid))
        try:
            with auto_unlock(self.device_.manager.lock_):
                res = requests.request('UNSUBSCRIBE', urlunparse(self.event_url), timeout=self.SUBSCRIBE_TIMEOUT,
                                       stream=False, allow_redirects=False,
                                       headers={'SID': sid})
        except requests.exceptions.Timeout as ex:
            log.error('UNSUBSCRIBE timed out, giving up.')
            log.exception(ex)
            self._handle_subscribe_failure(scheduler, self.TIMEOUT_RETRY_INTERVAL)
            return False
        except requests.exceptions.ConnectionError as ex:
            log.error("failed to connect to {} for UNSUBSCRIBE".format(urlunparse(self.event_url)))
            log.exception(ex)
            self._handle_subscribe_failure(scheduler, self.ERROR_RETRY_INTERVAL)
            return False

        self.is_subscribed = False

        if res.status_code != 200:
            log.warning("UNSUBSCRIBE unsuccessful, result is {}".format(res.status_code))

        return True


class WeMoSensor(Sensor):
    NS = "{urn:schemas-upnp-org:event-1-0}"

    def __init__(self, hostname: str, manager):
        super().__init__(hostname)
        self.manager = manager

        self.hostname = hostname
        self.hostip = self.map_hostname_to_local_ip(hostname)

        self.is_defunct_ = False

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

        # Who to notify on motion events.
        self.motion_listeners_ = []
        self.defunct_listeners_ = []

    @staticmethod
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

    def _fetch_service_xml(self, timeout):
        conn = http.client.HTTPConnection(self.http_location_url.hostname, self.http_location_url.port, timeout)
        conn.request("GET", self.http_location_url.path)
        conn.sock.settimeout(timeout)
        try:
            res = conn.getresponse()
            data = res.read()
            res.close()
        except socket.error as ex:
            if ex.errno == errno.ETIMEDOUT:
                raise TimeoutError(ex)
            raise ex
        return data

    def periodic_update(self):
        assert self.max_age != 0  # Ensure we've gotten at least one upnp update.

        now = datetime.now()
        since_update = now - self.setup_last_update
        if since_update < self.max_age:
            log.debug("Skipping setup.xml update for {} because {} < max-age({})".format(
                self.friendly_name, since_update, self.max_age))
            return

        # Grab the services file.
        try:
            log.info("Fetching services.xml for {} at {}".format(self.hostname, self.hostip))
            data = self._fetch_service_xml(timeout=5.0)
        except TimeoutError as ex:
            log.error("Timed out fetching services.xml for {}")
            log.exception(ex)
            self.set_defunct(True)
            return

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
                self.services[key] = _WeMoDeviceService(self,
                                                        str(node.serviceType), str(node.serviceId),
                                                        str(node.eventSubURL), str(node.SCPDURL))
                #updater.background_update(self.services[key])

        # If we just started up,
        if not self.get_service('basicevent1').is_subscribed:
            self.get_service('basicevent1').subscribe(self.manager.scheduler)

    def get_service(self, name: str) -> _WeMoDeviceService:
        return self.services[name]

    def listen_motion(self, callback: callable):
        self.motion_listeners_.append(callback)

    def listen_defunct(self, callback: callable):
        self.defunct_listeners_.append(callback)

    def motion_event(self, current_state: bool):
        event = MotionEvent(current_state)
        for listener in self.motion_listeners_:
            listener(event)

    @property
    def is_defunct(self):
        return self.is_defunct_

    def set_defunct(self, current_state: bool):
        self.is_defunct_ = current_state
        event = DefunctEvent(current_state)
        for listener in self.defunct_listeners_:
            listener(event)


class _WeMoNotifyServer(Thread):
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
    def __init__(self, address: (str, int), lock: Lock, manager):
        super().__init__()
        self.setDaemon(True)

        self.lock_ = lock

        self.manager = manager

        self.address_ = address

        self.sock_ = socket.socket()
        self.sock_.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock_.bind(address)

        self.want_exit_ = False

    def exit(self):
        self.want_exit_ = True
        sock = socket.socket()
        sock.connect(self.address_)
        sock.close()

    def run(self):
        self.sock_.listen(128)
        while True:
            peer, peer_addr = self.sock_.accept()
            result = self.process_one_connection(peer, peer_addr)
            peer.close()

            with self.lock_:
                if self.want_exit_:
                    return

                if result is not None:
                    self.handle_result(peer_addr, result[0], result[1], result[2])

    @staticmethod
    def process_one_connection(peer: socket.socket, peer_addr: (str, int)):
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
        # FIXME: this select may timeout if the device falls over and we won't get messages for 30 seconds. If this
        # FIXME:    happens often in practice, we'll need to start a thread for this.
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

    def handle_result(self, peer_addr, sid, seq, body):
        match = re.search(r'<BinaryState>(\d)</BinaryState>', body)
        if match is None:
            log.debug("got notification from {} not BinaryState for {}".format(peer_addr, sid))
            return

        current_state = bool(int(match.group(1)))
        log.info("got notification from {}: BinaryState: {}".format(peer_addr, current_state))

        device = self.manager.devices_[peer_addr[0]]
        device.motion_event(current_state)


class WeMoManager(Thread):
    UPNP_MSEARCH_BEACON_INTERVAL = 10 * 60  # seconds

    """
    Uses UPnP and service descriptors to keep our list internal list of devices up-to-date.
    """
    def __init__(self, own_intranet_ip: str, scheduler: Scheduler, network: NetworkBus, lock: Lock):
        super().__init__()

        # Compute the callback address once, since it's slow to do so.
        self.callback_address = "<http://{}:{}>".format(network.internal_address, 8989)

        # The set_timeout service.
        self.scheduler = scheduler

        # The global interlock that keeps everyone seeing coherent data.
        self.lock_ = lock

        # The set of tracked devices.
        self.devices_ = {}  # {host: WeMoSensor}

        # The i/o queue.
        self.queue_ = Queue()

        # The UPnP update infrastructure.
        self.upnp_server_ = ThreadingUDPServer((own_intranet_ip, 54322), self.handle_upnp_response)
        self.upnp_server_thread_ = Thread(target=self.upnp_server_.serve_forever)

        # The httpish server that receives updates from subscribed devices.
        self.event_receiver_ = _WeMoNotifyServer(('', 8989), self.lock_, self)
        self.event_receiver_.start()

    def add_device(self, device: WeMoSensor) -> WeMoSensor:
        assert device.manager == self
        assert device.hostip is not None
        self.devices_[device.hostip] = device
        return device

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
                message = self.queue_.get(block=True, timeout=self.UPNP_MSEARCH_BEACON_INTERVAL)
            except Empty:
                continue

            assert message is None
            self.upnp_server_.shutdown()
            self.event_receiver_.exit()

            self.upnp_server_thread_.join()
            self.event_receiver_.join()
            return

    def handle_upnp_response(self, request, client_address, server):
        log.debug("Received reply to UPnP request from {}".format(client_address))
        raw_data = request[0]
        try:
            headers = self.parse_http_to_headers(raw_data)
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
        self.scheduler.set_timeout(timedelta(seconds=0), state.periodic_update)

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
            raise FormatException("Expected 200 OK, got: " + status_line)

        out = {}
        for line in headers:
            name, _, value = line.partition(':')
            if name:
                out[name.strip().lower()] = value.strip()

        return out
