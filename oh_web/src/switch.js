var $ = require('jquery');
var R = require('ramda');
var jss = require('jss');
var home = require('./home');


function attach(conn, elem)
{
    var styles = jss.createStyleSheet({});
    styles.attach();

    $(elem).append('<div class="scene_select">Scene Select: '+
                   '<select id="global_switch"></select></div>');
    var gswitch = $('#global_switch');

    var gMonitoredPath = undefined;  // Path of scene we're observing.
    function broadcast_handler(path, msg) {
        if (path === gMonitoredPath)
            $(gswitch).val(msg.attrs.scene);
    }

    conn.query('scene').run()
        .then(R.values)
        .then(R.map((data) => data.attrs.name))
        .then(R.map((name) => `<option value="${name}">${name}</option>`))
        .then(R.map((opt) => $(gswitch).append(opt)))
        .then((_) => {
            conn.query('home').run()
                .then(msg => {
                    var path = R.last(R.keys(msg));
                    var data = R.last(R.values(msg));
                    gMonitoredPath = path;
                    $(gswitch).val(data.attrs.scene);
                });
        });

    gswitch.on('change', (e) => {
        var switchValue = e.target.options[e.target.selectedIndex].value;
        console.log("new switch val: " + switchValue);
        conn.query('home').attr('scene', switchValue).run();
    });

    return broadcast_handler;
}

module.exports = {
    attach: attach
};
