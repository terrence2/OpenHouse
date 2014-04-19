__author__ = 'terrence'

import mcp.network as network
from mcp.abode import Abode
from mcp.filesystem import FileSystem, Directory, File

import logging
from apscheduler.scheduler import Scheduler

log = logging.getLogger('manager')


class EyrieController:
    def __init__(self):
        self.abode = None
        self.devices = None
        self.filesystem = None
        self.network = None
        self.scheduler = None

    def init(self, abode: Abode, devices: [object], filesystem: FileSystem, bus: network.Bus, scheduler: Scheduler):
        self.abode = abode
        self.devices = devices
        self.filesystem = filesystem
        self.network = bus
        self.scheduler = scheduler

        self.init_presets(devices, filesystem)
        self.init_alarms()

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
    def map_filesystem_to_scheduler_day(day):
        return {'monday': 'mon',
                'tuesday': 'tue',
                'wednesday': 'wed',
                'thursday': 'thu',
                'friday': 'fri',
                'saturday': 'sat',
                'sunday': 'sun'}[day]

    def find_job(self, alarm_func):
        jobs = self.scheduler.get_jobs()
        for job in jobs:
            if job.func == alarm_func:
                return job
        return None

    def build_alarms(self):
        # Install custom alarm callbacks on the global.
        for name_ in ['wakeup', 'sleep']:
            for day_ in ['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday']:
                def make_alarm(controller, name, day):
                    def alarm():
                        controller.trigger_alarm(name, day)
                    controller.set_alarm(name, day, alarm)
                make_alarm(self, name_, day_)

    def init_alarms(self):
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
                        job = controller.find_job(alarm_func)
                        value = 'off'
                        if job is not None:
                            value = str(job.trigger)
                        return "Alarm {} for {}: {}\n".format(name, day, value)

                    def write_alarm(data: str):
                        data = data.strip()
                        alarm_func = controller.get_alarm(name, day)
                        existing_job = controller.find_job(alarm_func)

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
        if name == 'wakeup':
            return self.on_alarm_wakeup()
        return self.on_alarm_sleep()

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

    def apply_preset(self, name: str):
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
                 'hue-bedroom-dresser': {'on': True, 'hsv': (0, 47000, 255)}},
            'low':
                {'hue-bedroom-bed': {'on': True, 'hsv': (0, 34495, 232)},
                 'hue-bedroom-desk': {'on': True, 'hsv': (0, 34495, 232)},
                 'hue-bedroom-dresser': {'on': True, 'hsv': (0, 34495, 232)}},
            }
        if name not in states:
            return False
        state = states[name]
        for device_name, presets in state.items():
            device = self.devices[device_name]
            for prop, value in presets.items():
                setattr(device, prop, value)
        return True

    def init_presets(self, devices, filesystem: FileSystem):
        bedroom_lighting_preset = "unset"

        def read_lighting_preset() -> str:
            return "Current Value is: {} -- Possible Values are: on, off, sleep, read, low\n".format(
                bedroom_lighting_preset)

        def make_preset_writer(controller):
            def write_lighting_preset(data: str):
                nonlocal bedroom_lighting_preset
                data = data.strip()
                if not controller.apply_preset(data):
                    return
                bedroom_lighting_preset = data
            return write_lighting_preset

        presets = filesystem.root().add_entry("presets", Directory())
        bedroom = presets.add_entry("bedroom", Directory())
        bedroom.add_entry("lighting", File(read_lighting_preset, make_preset_writer(self)))

