__author__ = 'terrence'

import mcp.network as network
from mcp.abode import Abode
from mcp.filesystem import FileSystem, Directory, File

import logging
from apscheduler.scheduler import Scheduler

log = logging.getLogger('manager')


def alarm_wakeup():
    if not alarm_wakeup.active:
        return
    control = alarm_wakeup.control
    control.on_alarm_wakeup()

alarm_wakeup.active = True
alarm_wakeup.control = None


def alarm_sleep():
    if not alarm_sleep.active:
        return
    control = alarm_sleep.control
    control.on_alarm_sleep()

alarm_sleep.active = True
alarm_sleep.control = None


class ManualControl:
    def __init__(self, abode: Abode, devices: [object], filesystem: FileSystem, bus: network.Bus, scheduler: Scheduler):
        self.abode = abode
        self.devices = devices
        self.filesystem = filesystem
        self.network = bus
        self.scheduler = scheduler

        self.init_presets(devices, filesystem)
        self.init_alarms(scheduler, filesystem)

    def init_alarms(self, scheduler: Scheduler, filesystem: FileSystem):
        alarm_wakeup.control = self
        alarm_sleep.control = self

        scheduler.add_cron_job(alarm_wakeup, year='*', month='*', day_of_week='mon,tue,wed,thu,fri', hour=6, minute=50, second=0)
        scheduler.add_cron_job(alarm_sleep, year='*', month='*', day_of_week='mon,tue,wed,thu,fri', hour=22, minute=00, second=0)

        """
        def read_wakeup() -> str:
            return "Values are: 'off' or |24-hour-time|.\nExample: '7:30' or '16:42'.\n\nCurrent Value: {}".format(wakeup_alarm)

        def write_wakeup(data: str):
            pass

        alarm_dir = filesystem.root().add_entry("alarms", Directory())
        wakeup = alarm_dir.add_entry("wakeup", File(read_wakeup, write_wakeup))
        """

    def on_alarm_wakeup(self):
        for name, device in self.devices.items():
            if name.startswith('hue-'):
                device.on = True
                device.hsv = (255, 34495, 232)

    def on_alarm_sleep(self):
        for name, device in self.devices.items():
            if name.startswith('hue-'):
                device.on = True
                device.hsv = (0, 34495, 232)

    @staticmethod
    def init_presets(devices, filesystem: FileSystem):
        bedroom_lighting_preset = "unset"
        def read_lighting_preset() -> str:
            return "Current Value is: {} -- Possible Values are: on, off, sleep, reading".format(bedroom_lighting_preset)

        def write_lighting_preset(data: str):
            data = data.strip()
            states = {
                'off':
                    {'hue-bedroom-bed': {'on': False},
                     'hue-bedroom-desk': {'on': False},
                     'hue-bedroom-dresser': {'on': False}},
                'on':
                    {'hue-bedroom-bed': {'on': True, 'hsv': (255, 34495, 232)},
                     'hue-bedroom-desk': {'on': True, 'hsv': (255, 34495, 232)},
                     'hue-bedroom-dresser': {'on': True, 'hsv': (255, 34495, 232)}},
                'read':
                    {'hue-bedroom-bed': {'on': True, 'hsv': (255, 34495, 232)},
                     'hue-bedroom-desk': {'on': True, 'hsv': (0, 34495, 232)},
                     'hue-bedroom-dresser': {'on': True, 'hsv': (0, 34495, 232)}},
                'sleep':
                    {'hue-bedroom-bed': {'on': False},
                     'hue-bedroom-desk': {'on': True, 'hsv': (0, 47000, 255)},
                     'hue-bedroom-dresser': {'on': True, 'hsv': (0, 47000, 255)}}
            }
            if data not in states:
                return
            state = states[data]
            for device_name, presets in state.items():
                device = devices[device_name]
                for prop, value in presets.items():
                    setattr(device, prop, value)

            nonlocal bedroom_lighting_preset
            bedroom_lighting_preset = data

        presets = filesystem.root().add_entry("presets", Directory())
        bedroom = presets.add_entry("bedroom", Directory())
        bedroom.add_entry("lighting", File(read_lighting_preset, write_lighting_preset))

