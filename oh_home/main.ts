/*
 * This Source Code Form is subject to the terms of the GNU General Public
 * License, version 3. If a copy of the GPL was not distributed with this file,
 * You can obtain one at https://www.gnu.org/licenses/gpl.txt.
 */
/// <reference path="interfaces/node.d.ts" />
/// <reference path="interfaces/bunyan.d.ts" />
/// <reference path="interfaces/jsdom.d.ts" />
/// <reference path="interfaces/zmq.d.ts" />
import fs = require('fs');

import bunyan = require('bunyan');
import jsdom = require('jsdom');
import zmq = require('zmq');


var log = bunyan.createLogger({
  name: 'HOMe',
  streams: [
    { level: 'debug', stream: process.stdout },
    { level: 'debug', path: 'debug.log' }
  ]
});


var query_address: string = "ipc:///var/run/openhouse/home/query";
var event_address: string = "ipc:///var/run/openhouse/home/events";
var home_html: string = "home.html";


function check_zmq_version(): boolean {
    var parts = zmq.version.split(".");
    return Number(parts[0]) > 2;
}
if (!check_zmq_version()) {
    log.fatal("ZMQ version must be >=3.0.0... stopping");
    process.exit(1);
}


function print_usage(message) {
    console.log("Error: " + message);
    console.log("");
    console.log("Usage: node oh_home.js <home.html>");
}
if (process.argv.length < 3) {
    print_usage("No home.html provided.");
    process.exit(1);
}
home_html = process.argv[2];


function pathof($, node): string {
    var path = [];
    var current = $(node);
    while (current.attr("name")) {
        path.unshift(current.attr("name"));
        current = $(current.parent()[0]);
    }
    if (!path.length)
        return "";
    return "/" + path.join("/");
}


class Context {
    $: any; // I'm not even going to try to type jquery.
    pub_sock: zmq.Socket;

    constructor(doc, pub_sock: zmq.Socket) {
        this.$ = doc;
        this.pub_sock = pub_sock
    }
}


interface Message {
    type: string;
}
interface Response {
    error?: string;
}
function handle_message(ctx: Context, msg: ArrayBuffer): Response {
    var data = JSON.parse(msg.toString());

    var type: string = data.type;
    try {
        if (type == "ping")
            return handle_ping(data);
        else if (type == "query")
            return handle_query(ctx, data);
    } catch(e) {
        log.error({exception: e}, "Failed to handle message");
        return {error: "Unexpected exception", exception: e};
    }

    log.warn({message_type: type}, "unrecognized message type");
    return {error: "Unrecognized message type: " + type}
}


interface PingMessage extends Message {
    ping: string;
}
interface PingResponse extends Response {
    pong: string;
}
function handle_ping(data: PingMessage): PingResponse {
    log.info({data: data.ping}, "handling ping");
    return {pong: data.ping};
}


interface Transform {
    method: string;
    args: string[];
}
interface QueryMessage extends Message {
    query: string;
    transforms: Transform[];
}
interface Attributes {
    [index: string]: string;
}
function attrs($, node): Attributes {
    var attrs: Attributes = {};
    $(node.attributes).each(function() { attrs[this.nodeName] = this.nodeValue; });
    return attrs;
}
interface QueryResponse {
    [index: string]: Attributes;
}
function handle_query(ctx: Context, data: QueryMessage): QueryResponse {
    log.info({query: data.query}, "handling query");

    // Keep a list of all touched nodes with the most recent values.
    var output: QueryResponse = {};

    // Perform the query.
    var nodes = ctx.$(data.query);
    nodes.each(function(i, node) {
        output[pathof(ctx.$, node)] = attrs(ctx.$, node);
    });

    // Apply each transform to the initial query.
    for (var i in data.transforms) {
        var method_name = data.transforms[i].method;
        var args = data.transforms[i].args;

        nodes = nodes[method_name].apply(nodes, args);
        nodes.each(function(i, node) {
            output[pathof(ctx.$, node)] = attrs(ctx.$, node);
        });
    }

    // Publish touched nodes for subscribers to snoop on.
    for (var path in output)
        ctx.pub_sock.send(path + " " + JSON.stringify(output[path]));

    return output;
}


fs.readFile(home_html, function(error, data) {
    if (error) {
        log.fatal("Failed to to load home '" + home_html + "': " + error);
        process.exit(1);
    }

    jsdom.env(
        data.toString(),
        ["../node_modules/jquery/dist/jquery.min.js"],
        function (errors, window) {
            if (errors) {
                log.fatal("failed to load %s: %s", home_html, errors.toString());
                process.exit(1);
            }
            var $ = window.$;
            log.info("Loaded home: " + home_html);

            var pub_sock = zmq.socket("pub");
            pub_sock.bind(event_address, function(error) {
                if (error) {
                    log.fatal("Failed to bind event socket: " + error);
                    process.exit(1);
                }

                var ctx = new Context($, pub_sock);

                var query_sock = zmq.socket("rep");
                query_sock.bind(query_address, function(error) {
                    if (error) {
                        log.fatal("Failed to bind query socket: " + error);
                        process.exit(1);
                    }

                    query_sock.on('message', function(msg) {
                        var output = handle_message(ctx, msg);
                        query_sock.send(JSON.stringify(output));
                    });
                });
            });

            var saveInterval = setInterval(function() {
                log.info("saving snapshot");
                fs.writeFile('snapshot.html', jsdom.serializeDocument(window.document), function (err) {
                    if (err)
                        console.error("Failed to save snapshot.");
                });
            }, 60 * 1000);
        }
    );
});
