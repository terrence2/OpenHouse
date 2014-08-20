# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import heapq

from collections import namedtuple
from datetime import datetime, timedelta
from queue import Queue, Empty
from threading import Thread, Lock


class _Event(namedtuple('_Event', ('time', 'action'))):
    def __eq__(s, o): return s.time == o.time
    #def __ne__(s, o): return s.time != o.time
    def __lt__(s, o): return s.time <  o.time
    #def __le__(s, o): return s.time <= o.time
    #def __gt__(s, o): return s.time >  o.time
    #def __ge__(s, o): return s.time >= o.time


class Scheduler(Thread):
    def __init__(self, lock: Lock):
        super().__init__()
        self.lock_ = lock

        self.queue_ = Queue()
        self.heap_ = []

    def set_timeout(self, delay: timedelta, callback: callable):
        self.queue_.put((datetime.now() + delay, callback))

    def quit(self):
        self.queue_.put(None)

    def _compute_next_delay(self):
        if self.heap_:
            return (self.heap_[0].time - datetime.now()).total_seconds()
        return None

    def run(self):
        while True:
            try:
                # Block until we get an event if the heap is empty, or block
                # until it is time to fire the next event.
                event = self.queue_.get(block=True, timeout=self._compute_next_delay())

                # A None event is the cue to exit.
                if not event:
                    return
            except Empty:
                # Not an external request, so it must be time to fire the next request.
                top = heapq.heappop(self.heap_)
                assert top.time < datetime.now(), "verify that enough time has passed waiting"
                with self.lock_:
                    top.action()
                continue

            # Insert the new event into our heap.
            assert event, "not an empty queue assertion, so we must have a new event to schedule"
            heapq.heappush(self.heap_, _Event(*event))

