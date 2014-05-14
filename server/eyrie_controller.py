# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
__author__ = 'terrence'

import mcp.network as network
from mcp.abode import Abode
from mcp.devices import DeviceSet
from mcp.filesystem import FileSystem, Directory, File

from datetime import datetime
import logging

from apscheduler.scheduler import Scheduler

log = logging.getLogger('manager')


class EyrieState:
    Manual = 0

    WakingUp = 1
    Daytime = 2
    Bedtime = 3
    Sleep = 4

    @staticmethod
    def to_string(state: int) -> str:
        return ['manual', 'wakeup', 'daytime', 'bedtime', 'sleep'][state]


class EyrieController:
    """
    The state machine:

    WakingUp @ WakeupAlarm -> +30m
      Simple replay of a sunrise until it ends or we manually move the state.

    Daytime
      Motion -> Lights
      Light level set by time of day relative to sunset/sunrise.

    Bedtime @ -30m -> SleepAlarm
      Simple replay of sunset until it ends or we manually move the state.

    Sleep
      Lights fixed, no movement triggers (yet).

    Manual
      All other states can jump here and the lights will be set as requested.
      When done, will cycle back to the appropriate state and update actuators.
    """

    DefaultWakeupTime = 60 * 10 + 0  # min
    DefaultSleepTime = 60 * 23 + 29  # min
    WakeupAlarmInterval = 30  # min
    SleepAlarmInterval = 30  # min

    SleepColor = (0, 47000, 255)  # hsv
    DaylightHue = 34495
    DaylightV = 232

    @classmethod
    def daylight(cls, brightness: float) -> (int, int, int):
        """Return an HSV tuple for pleasant light at the given relative brightness."""
        assert brightness >= 0
        assert brightness <= 1
        return int(255 * brightness), cls.DaylightHue, cls.DaylightV

    @classmethod
    def daylight_with_ambient(cls):
        """
        Return an HSV tuple for pleasant light, dimming the light when it is light outside, unless it is overcast.
        """


    def __init__(self):
        self.abode = None
        self.devices = None
        self.filesystem = None
        self.network = None
        self.scheduler = None

        self.state_ = EyrieState.Manual

    def init(self, abode: Abode, devices: DeviceSet, filesystem: FileSystem, bus: network.Bus, scheduler: Scheduler):
        self.abode = abode
        self.devices = devices
        self.filesystem = filesystem
        self.network = bus
        self.scheduler = scheduler

        self.init_presets(devices, filesystem)
        self.init_alarms()

        self.abode.lookup('/eyrie/bedroom').listen('motion', 'propertyChanged', self.on_motion)
        self.abode.lookup('/eyrie/office').listen('motion', 'propertyChanged', self.on_motion)
        self.abode.lookup('/eyrie/livingroom').listen('motion', 'propertyChanged', self.on_motion)

        self.restore_automatic_control()

    def on_motion(self, event):
        if self.state_ != EyrieState.Daytime:
            return

        #color = int(event.property_value) * self.adjustment_for_ambient()
        #properties = event.target.set('hsv', self.daylight())

        devices = self.devices.select('$hue').select('@' + event.target.name)
        if event.property_value:
            devices.set('on', True).set('hsv', self.daylight(1))
        else:
            devices.set('on', True).set('hsv', self.daylight(0))

    ### State Machine ###
    @property
    def state(self):
        return self.state_

    def leave_automatic_control(self):
        self.state_ = EyrieState.Manual

    def restore_automatic_control(self):
        # Get our current minutes offset.
        now = datetime.now()
        current_minutes = now.hour * 60 + now.minute

        # Find current day of week.
        weekday = datetime.today().weekday()
        weekday_name = ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday'][weekday]

        # Get alarm start and end times for that day.
        wakeup_job = self.find_scheduler_job(self.get_alarm('wakeup', weekday_name))
        wakeup_minutes = self.alarm_to_minutes(wakeup_job, now) if wakeup_job else self.DefaultWakeupTime
        sleep_job = self.find_scheduler_job(self.get_alarm('sleep', weekday_name))
        sleep_minutes = self.alarm_to_minutes(sleep_job, now) if sleep_job else self.DefaultSleepTime

        # Find our state based on the time.
        if current_minutes < wakeup_minutes or current_minutes > sleep_minutes:
            self.state_ = EyrieState.Sleep
        elif current_minutes < (wakeup_minutes + self.WakeupAlarmInterval):
            self.state_ = EyrieState.WakingUp
        elif current_minutes > (sleep_minutes + self.SleepAlarmInterval):
            self.state_ = EyrieState.Bedtime
        else:
            self.state_ = EyrieState.Daytime

        # Do our best to set up auto mode correctly.
        self.reboot_state_from_nothing()

    def reboot_state_from_nothing(self):
        """
        This is for stateless restart into auto mode. Most state is edge triggered, so this is a bit of a hack and
        may or may not line up perfectly with what we'd get by just running the machine normally.
        """
        assert self.state_ != EyrieState.Manual

        if self.state_ == EyrieState.Sleep:
            self.enter_sleep_state()

        elif self.state_ == EyrieState.Daytime:
            # FIXME: currently we turn on everything and let normal events stabilize us later.
            #        Instead we should be snooping the motion state directly.
            self.devices.select('$hue').set('on', True).set('hsv', self.daylight(1))

        elif self.state_ == EyrieState.WakingUp:
            # FIXME: build the animation in such a way that we can re-enter it from the middle.
            self.devices.select('$hue').set('on', True).set('hsv', self.daylight(1))

        elif self.state_ == EyrieState.Bedtime:
            # FIXME: build the animation in such a way that we can re-enter it from the middle.
            self.enter_sleep_state()

    def enter_sleep_state(self):
        assert self.state_ == EyrieState.Sleep
        bed_light = self.devices.select('$hue').select('@bedroom').select('#bed')
        bed_light.set('on', False)
        (self.devices.select('$hue') - bed_light).set('on', True).set('hsv', self.SleepColor)


    ### ALARMS ###
    @staticmethod
    def alarm_name(name, day):
        return 'alarm_{}_{}'.format(name, day)

    @staticmethod
    def get_alarm(name, day):
        return globals()[EyrieController.alarm_name(name, day)]

    @staticmethod
    def set_alarm(name, day, alarm_func):
        global_name = EyrieController.alarm_name(name, day)
        alarm_func.__name__ = global_name
        globals()[global_name] = alarm_func

    @staticmethod
    def alarm_to_minutes(job, dateval):
        total = 0
        for field in job.trigger.fields:
            if field.name == 'hour':
                value = field.get_next_value(dateval)
                if value:
                    total += 60 * int(value)
            if field.name == 'minute':
                value = field.get_next_value(dateval)
                if value:
                    total += int(value)
        return total

    @staticmethod
    def map_filesystem_to_scheduler_day(day):
        return {'monday': 'mon',
                'tuesday': 'tue',
                'wednesday': 'wed',
                'thursday': 'thu',
                'friday': 'fri',
                'saturday': 'sat',
                'sunday': 'sun'}[day]

    def find_scheduler_job(self, alarm_func):
        jobs = self.scheduler.get_jobs()
        for job in jobs:
            if job.func == alarm_func:
                return job
        return None

    def build_alarms(self):
        """
        Build alarm functions and put them on the global. This is separate from normal init
        because it has to be done very early in init, before initializing apscheduler.
        """
        for name_ in ['wakeup', 'sleep']:
            for day_ in ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday']:
                def make_alarm(controller, name, day):
                    def alarm():
                        controller.trigger_alarm(name, day)
                    controller.set_alarm(name, day, alarm)
                make_alarm(self, name_, day_)

    def init_alarms(self):
        """
        Binds the callable alarm functions we made earlier into the system. Apscheduler should
        have picked up any existing schedules automatically.
        """
        def alarms_help() -> str:
            return "Values are: 'off' or |24-hour-time|.\nExample: '7:30' or '16:42'.\n"

        # Now that we've initialized the rest of the system, install our alarms on the filesystem.
        alarms_dir = self.filesystem.root().add_entry("alarms", Directory())
        alarms_dir.add_entry("help", File(alarms_help, None))
        for name_ in ['wakeup', 'sleep']:
            alarm_dir = alarms_dir.add_entry(name_, Directory())
            for day_ in ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday']:
                def make_alarm_file(controller, name, day):
                    """Closure to capture the right values for loop vars name and day."""
                    def read_alarm() -> str:
                        alarm_func = controller.get_alarm(name, day)
                        job = controller.find_scheduler_job(alarm_func)
                        value = 'off'
                        if job is not None:
                            value = str(job.trigger)
                        return "Alarm {} for {}: {}\n".format(name, day, value)

                    def write_alarm(data: str):
                        data = data.strip()
                        alarm_func = controller.get_alarm(name, day)
                        existing_job = controller.find_scheduler_job(alarm_func)

                        if data == 'off':
                            if existing_job:
                                controller.scheduler.unschedule_job(existing_job)
                            return

                        hour, _, minute = data.strip().partition(':')
                        hour = min(23, max(0, int(hour)))
                        minute = min(59, max(0, int(minute)))
                        day_of_week = controller.map_filesystem_to_scheduler_day(day)
                        if existing_job:
                            controller.scheduler.unschedule_job(existing_job)
                        controller.scheduler.add_cron_job(alarm_func, day_of_week=day_of_week, hour=hour, minute=minute)

                    return File(read_alarm, write_alarm)
                alarm_dir.add_entry(day_, make_alarm_file(self, name_, day_))

    def trigger_alarm(self, name, day):
        """
        Alarm trampoline into the controller.
        """
        if name == 'wakeup':
            return self.on_alarm_wakeup()
        return self.on_alarm_sleep()

    def on_alarm_wakeup(self):
        for device in self.devices.select("$hue"):
            device.on = True
            device.hsv = (255, 34495, 232)

    def on_alarm_sleep(self):
        for device in self.devices.select("$hue"):
            device.on = True
            device.hsv = (0, 34495, 232)

    ### PRESETS ###
    def apply_preset(self, name: str, match: str):
        if name == 'auto':
            self.restore_automatic_control()
            return
        self.leave_automatic_control()

        devices = self.devices.select(match)
        if name == 'off':
            devices.set('on', False)
        elif name == 'on':
            devices.set('on', True).set('hsv', (255, 34495, 232))
        elif name == 'low':
            devices.set('on', True).set('hsv', (0, 34495, 232))
        elif name == 'read':
            bed = devices.select('#bed')
            bed.set('on', True).set('hsv', (255, 34495, 232))
            (devices - bed).set('on', True).set('hsv', (0, 34495, 232))
        elif name == 'sleep':
            bed = devices.select('#bed')
            bed.set('on', False).set('hsv', (0, 34495, 232))
            (devices - bed).set('on', True).set('hsv', (0, 47000, 255))

    def init_presets(self, devices: DeviceSet, filesystem: FileSystem):
        preset_state = {
            '@bedroom': 'unset',
            '@office': 'unset',
            '@livingroom': 'unset'
        }

        def make_preset_file(controller, match: str):
            def read_lighting_preset() -> str:
                name = preset_state[match]

                # Presets are only useful in Manual mode -- the controller state superceeds the state here.
                if controller.state != EyrieState.Manual:
                    name = 'auto:{}'.format(EyrieState.to_string(controller.state))

                return "Current Value is: {} -- Possible Values are: on, off, sleep, read, low, auto\n".format(name)

            def write_lighting_preset(data: str):
                data = data.strip()
                controller.apply_preset(data, match)
                preset_state[match] = data

            return File(read_lighting_preset, write_lighting_preset)

        presets = filesystem.root().add_entry("presets", Directory())
        for key in preset_state.keys():
            dir_node = presets.add_entry(key.strip('@'), Directory())
            dir_node.add_entry("lighting", make_preset_file(self, key))

