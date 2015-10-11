// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
var home = require('./home');

window.do_daytime = function(e) {set_scene("daytime")};
window.do_evening = function(e) {set_scene("evening")};
window.do_sleep = function(e) {
    console.log("GOT HERE");
    set_scene("sleep");
};

function set_scene(scene_name)
{
    home.connect(HOME_ADDRESS)
        .then(conn => {
            conn.query('home').attr('scene', scene_name).run();
        });
}

module.exports = { set_scene: set_scene };

