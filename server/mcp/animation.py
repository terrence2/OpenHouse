# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from datetime import datetime, timedelta
from threading import Thread
import os
import select


class Animation:
    def __init__(self):
        super().__init__()
        self.is_over = False

    def animate(self):
        raise NotImplementedError("Animations must override animate.")


class NullAnimation(Animation):
    def animate(self):
        pass


class CallbackAnimation(Animation):
    def __init__(self, callback: callable):
        super().__init__()
        self.callback_ = callback

    def animate(self):
        self.is_over = self.callback_() is False


class LinearAnimation(Animation):
    def __init__(self, start, end, duration: float, tick_callback: callable, finish_callback: callable):
        """
        |start| and |end| must support __sub__ and __mul__ for interpolation.
        """
        super().__init__()
        self.start_ = start
        self.end_ = end
        self.extent_ = self.end_ - self.start_
        self.tick_callback_ = tick_callback
        self.finish_callback_ = finish_callback

        self.duration_ = timedelta(seconds=duration)
        self.start_time_ = datetime.now()
        self.end_time_ = self.start_time_ + self.duration_

    def animate(self):
        now = datetime.now()

        if now >= self.end_time_:
            self.tick_callback_(self.end_)
            self.is_over = True
            self.finish_callback_()
            return

        elapsed = now - self.start_time_
        fraction = elapsed.total_seconds() / self.duration_.total_seconds()
        value = self.start_ + (self.extent_ * fraction)
        self.tick_callback_(value)


class OldAnimation:
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


class AnimationController(Thread):
    """
    A simple interval scheduler.
    """
    def __init__(self, interval, lock):
        super().__init__()
        self.daemon = True

        self.read_fd_, self.write_fd_ = os.pipe()
        self.interval_ = interval
        self.lock_ = lock
        self.want_exit_ = False
        self.state_ = NullAnimation()

    def exit(self):
        with self.lock_:
            self.want_exit_ = True
            os.write(self.write_fd_, b"\0")

    def run(self):
        while True:
            readable, _, _ = select.select([self.read_fd_], [], [], self.interval_)
            if readable:
                os.read(self.read_fd_, 4096)

            with self.lock_:
                if self.want_exit_:
                    return

                self._apply_animation()

    def _apply_animation(self):
        self.state_.animate()

        if self.state_.is_over:
            self.state_ = NullAnimation()

    def animate(self, animation: Animation):
        """
        Must be called with lock held.
        """
        self.state_ = animation
        os.write(self.write_fd_, b"\0")

    def cancel_ongoing_animation(self):
        """
        Must be called with lock held.
        """
        self.state_ = NullAnimation()
        os.write(self.write_fd_, b"\0")

