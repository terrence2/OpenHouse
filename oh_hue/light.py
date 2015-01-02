# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from color import parse_css_color, RGB, BHS, Mired
from home import Home
from bridge import Bridge

import logging

log = logging.getLogger('oh_hue.light')


def parse_css(style: str) -> dict:
    out = {}
    parts = style.strip(';').split(';')
    for part in parts:
        key, _, value = part.strip().partition(':')
        out[key.strip()] = value.strip()
    return out


class Light:
    def __init__(self, id_: str, path: str, name: str, bridge: Bridge, home: Home):
        self.id_ = id_
        self.name_ = name
        self.path_ = path
        self.bridge_ = bridge

        # Bind to light updates.
        home.subscribe(self.path_, self.on_change)

    def on_change(self, target: str, data: dict):
        assert target == self.path_

        try:
            attrs = data['attrs']
            style = attrs['style']
        except KeyError as ex:
            log.warn("No style set on light.")
            return

        # FIXME: use DOM to getComputedStyle so that we can use CSS, etc.
        log.info("light state updated: {}".format(style))
        css = parse_css(data['attrs']['style'])
        try:
            raw_color = parse_css_color(css.get('color', 'rgb(255,255,255)'))
            light_vis = css.get('visibility', 'visible')
            trans_time = int(float(css.get('animation-duration', 1)) * 10)
        except Exception as ex:
            log.error("Failed to parse light style")
            log.exception(ex)
            return

        # Color must be BHS or Mired.
        if isinstance(raw_color, RGB):
            raw_color = BHS.from_rgb(raw_color)

        props = {
            'on': light_vis == 'visible',
            'transitiontime': trans_time,
        }
        if isinstance(raw_color, BHS):
            props.update({
                'bri': raw_color.b,
                'hue': raw_color.h,
                'sat': raw_color.s,
            })
        else:
            assert isinstance(raw_color, Mired)
            props.update({'ct': raw_color.ct})
        self.bridge_.set_light_state(self.id_, props)

