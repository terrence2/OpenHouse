// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
var $ = require('jquery');
var R = require('ramda');
var jss = require('jss');
var home = require('./home');


function attach(conn, elem)
{
    var styles = jss.createStyleSheet({});
    styles.attach();

    $(elem).append(`<div class="enable-alarms">Enable Alarms:
                      <input type="checkbox" id="alarm_enable"/></div>`);
    var gAlarmEnable = $("#alarm_enable");

    conn.query('alarm').run()
        .then(msg => console.log(msg));
}

module.exports = {
    attach: attach
};
