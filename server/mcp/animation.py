# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from datetime import datetime, timedelta
from threading import Thread
import time


class Ticker(Thread):
    """
    The dumbest possible interval scheduler. It just sleeps for interval and calls the callback in a loop. No
    provisions are made for drift... or for anything else.
    """
    def __init__(self, callback, interval, lock):
        super().__init__()
        self.daemon = True

        self.callback_ = callback
        self.interval_ = interval
        self.lock_ = lock
        self.want_exit_ = False

    def exit(self):
        with self.lock_:
            self.want_exit_ = True

    def run(self):
        while True:
            time.sleep(self.interval_)

            with self.lock_:
                if self.want_exit_:
                    return

                self.callback_()


class Animation:
    """
    Represents an animation state.
    """
    def __init__(self, duration: float, initial, terminal):
        self.duration_ = timedelta(seconds=duration)
        self.starttime_ = datetime.now()
        self.endtime_ = self.starttime_ + self.duration_
        self.initial_ = initial
        self.terminal_ = terminal

    def initial(self):
        return self.initial_

    def is_over(self):
        return datetime.now() > self.endtime_

    def interpolate(self, fraction: float):
        """Knows how to handle numbers. Needs to be subclassed to handle other types."""
        return self.initial_ + ((self.terminal_ - self.initial_) * fraction)

    def current(self):
        now = min(self.endtime_, datetime.now())
        elapsed = now - self.starttime_
        fraction = elapsed.total_seconds() / self.duration_.total_seconds()
        return self.interpolate(fraction)


class VectorAnimation(Animation):
    def interpolate(self, fraction: float):
        return [a + ((b - a) * fraction) for a, b in zip(self.initial_, self.terminal_)]

class IntVectorAnimation(Animation):
    def interpolate(self, fraction: float):
        return [int(a + ((b - a) * fraction)) for a, b in zip(self.initial_, self.terminal_)]
