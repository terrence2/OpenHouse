# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
__author__ = 'terrence'

from datetime import datetime, timedelta, time
import logging
import http.client
import xml.etree.ElementTree as etree

log = logging.getLogger('environment')


class Environment:
    """
    Look up and cache weather information and day start/end times for use in other parts of the system.
    """
    EarthTools = "www.earthtools.org"

    def __init__(self):
        self.latitude_ = 34.45
        self.longitude_ = -119.72

        self.last_request_ = datetime.now() - timedelta(days=2)  # Fudge the last request time so we get new data now.
        self.sunrise_twilight_ = None
        self.sunrise_ = None
        self.sunset_ = None
        self.sunset_twilight_ = None

    @classmethod
    def stamp_to_time(cls, stamp: str) -> time:
        parts = stamp.strip().split(':')
        return time(
            hour=int(parts[0]),
            minute=int(parts[1]),
            second=int(parts[2])
        )

    def request_sunrise_sunset(self):
        now = datetime.now()
        assert now - self.last_request_ >= timedelta(days=1)

        request_url = "/sun/{}/{}/{}/{}/99/0".format(
            self.latitude_,
            self.longitude_,
            now.day,
            now.month
        )
        log.info("Requesting: {}".format(request_url))
        conn = http.client.HTTPConnection(self.EarthTools)
        conn.connect()
        conn.request('GET', request_url)
        response = conn.getresponse()
        body = response.read()
        conn.close()
        self.last_request_ = datetime.now()

        tree = etree.fromstring(body)
        morning = tree.find('morning')
        sunrise = morning.find('sunrise').text
        sunrise_twilight = morning.find('twilight').find('civil').text
        evening = tree.find('evening')
        sunset = evening.find('sunset').text
        sunset_twilight = evening.find('twilight').find('civil').text

        self.sunrise_twilight_ = self.stamp_to_time(sunrise_twilight)
        self.sunrise_ = self.stamp_to_time(sunrise)
        self.sunset_ = self.stamp_to_time(sunset)
        self.sunset_twilight_ = self.stamp_to_time(sunset_twilight)
        log.info("Updated sunrise/sunset times: {}-{} & {}-{}".format(
            self.sunrise_twilight_, self.sunrise_, self.sunset_twilight_, self.sunset_))

    def update_sunrise_sunset(self):
        """Ensures our cached data is up-to-date."""
        if datetime.now() - self.last_request_ > timedelta(days=1):
            self.request_sunrise_sunset()

    @property
    def sunrise_twilight(self) -> time:
        self.update_sunrise_sunset()
        return self.sunrise_twilight_

    @property
    def sunrise(self) -> time:
        self.update_sunrise_sunset()
        return self.sunrise_

    @property
    def sunset(self) -> time:
        self.update_sunrise_sunset()
        return self.sunset_

    @property
    def sunset_twilight(self) -> time:
        self.update_sunrise_sunset()
        return self.sunset_twilight_

if __name__ == '__main__':
    env = Environment()
    print(env.sunrise)
    print(env.sunset)
