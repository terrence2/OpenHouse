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

    conn.query('[kind=alarm]').run()
        .then(msg => console.log(msg));
}

module.exports = {
    attach: attach
};
