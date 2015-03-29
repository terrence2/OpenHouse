// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
var esprima = require('esprima');
var $ = require('jquery');
var R = require('ramda');
var jss = require('jss');
var home = require('./home');
var util = require('./util');

var gHistory = {position: 0, commands: []};


function parse_query(command)
{
    var call_list = [];

    var program = esprima.parse(command);
    var call = program.body[0].expression;
    util.assert(call.type == 'CallExpression', "query must end with a call");
    while (call.callee.type == 'MemberExpression') {
        // Verify and add the current expression to call_list.
        util.assert(call.callee.property.type == 'Identifier',
                    "called properties must be identifiers");
        var name = call.callee.property.name;
        var args = [];
        for (var arg of call.arguments) {
            util.assert(arg.type == 'Literal', "arguments must be literals");
            args.push(arg.value);
        }
        call_list.push({name: name, args: args});

        // Go to the next nested callee.
        util.assert(call.callee.computed === false,
                    "computed member expressions not allowed.");
        util.assert(call.callee.object.type == 'CallExpression',
                    "query expression must be a chain of calls");
        call = call.callee.object;
    }
    util.assert(call.callee.type == 'Identifier',
                "query's first call must be to an identifier");
    util.assert(call.callee.name == '$', "query must be a call to '$'");
    util.assert(call.arguments.length === 1, "call to $ must have one argument");
    util.assert(call.arguments[0].type == 'Literal', "call to $ must have one literal arg");

    return [call.arguments[0].value, call_list];
}

function build_result_html(msg)
{
    var out = '';
    for (var path in msg) {
        out += `<div>${path}: ${msg[path].tagName}</div>`;
        for (var attr_name in msg[path].attrs)
            out += `<div class="query_out_attr">{ ${attr_name} : ${msg[path].attrs[attr_name]} }</div>`;
        if (msg[path].text.length > 0)
            out += `<div class="query_out_text">&lt;text&gt;: ${msg[path].text}</div>`;
    }
    return out;
}

function on_query_entered(conn, event)
{
    var elem = $(event.target);
    var command = elem.val();
    elem.val('');

    $('#query_response').removeClass('query_error').html('');
    try {
        var [query, calls] = parse_query(command);
    } catch (e) {
        $('#query_response').addClass('query_error').html(e.toString());
        return;
    }

    // Print to the debug console so we can easily verify that we parsed correctly.
    var msg = '$("' + query + '")';
    for (var call of calls)
        msg += '.' + call.name + '(' + call.args.join(', ') + ')';
    console.log(msg);

    // Build and run the query and show the result.
    var q = conn.query(query);
    for (var call of calls) {
        q[call.name].apply(q, call.args);
    }
    q.run()
     .then(msg => $('#query_response').html(build_result_html(msg)));

    // Add the thing to history and reset our backlog.
    gHistory.commands.unshift(command);
    gHistory.position = 0;
}

function on_keyup(event)
{
    if (event.keyCode == 38) {  // arrow-up
        if (gHistory.position + 1 > gHistory.commands.length)
            return;
        var last = gHistory.commands[gHistory.position];
        gHistory.position += 1;
        $(event.target).val(last);
    } else if (event.keyCode == 40) {  // arrow-down
        if (gHistory.position - 1 < 0) {
            $(event.target).val('');
            return;
        }
        gHistory.position -= 1;
        $(event.target).val(gHistory.commands[gHistory.position]);
    }
}

function attach(conn, elem)
{
    var styles = jss.createStyleSheet({
        '#query_console': {
            'width': '600px',
        },
        '.query_out_attr': {
            'padding-left': '25px',
            'font-size': '11pt',
        },
        '.query_out_text': {
            'padding-left': '25px',
            'font-size': '11pt',
        },
        '.query_error': {
            'color': 'red',
            'font-weight': 'bold',
        },
    });
    styles.attach();

    $(elem).append('<input id="query_console" type="text" width="120"></input>' +
                   '<div id="query_response"></div>');
    $('#query_console')
        .change(R.partial(on_query_entered, conn))
        .keyup(R.partial(on_keyup));

}

module.exports = {
    attach: attach
};
