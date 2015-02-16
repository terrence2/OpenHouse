/*
 * This Source Code Form is subject to the terms of the GNU General Public
 * License, version 3. If a copy of the GPL was not distributed with this file,
 * You can obtain one at https://www.gnu.org/licenses/gpl.txt.
 */
/// <reference path="interfaces/node.d.ts" />
/// <reference path="interfaces/bunyan.d.ts" />
/// <reference path="interfaces/jsdom.d.ts" />
/// <reference path="interfaces/primus.d.ts" />
/// <reference path="interfaces/zmq.d.ts" />
import fs = require('fs');

import bunyan = require('bunyan');
import jsdom = require('jsdom');
import primus = require('primus');
import zmq = require('zmq');


var log = bunyan.createLogger({
  name: 'HOMe',
  streams: [
    { level: 'debug', stream: process.stdout },
    { level: 'debug', path: 'debug.log' }
  ]
});


// Protocol constants.
var query_major_version: number = 3;
var query_minor_version: number = 0;
// 0MQ constants.
var query_address: string = "ipc:///var/run/openhouse/home/query";
var event_address: string = "ipc:///var/run/openhouse/home/events";
// WebSocket constants.
var websocket_ipv4: string = "192.168.0.16";
var websocket_port: number = 8080;
var websocket_address: string = "ws://" + websocket_ipv4 + ":" + websocket_port + "/primus";
var websocket_client_code: string = "http://" + websocket_ipv4 + ":" + websocket_port + "/primus/primus.js";
// Configuration constants.
var home_html: string = "home.xhtml";
var autosave_interval: number = 5 * 60 * 1000;


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


interface Subscriptions {
    [index: string]: Array<any>;
}
class Context {
    $: any; // I'm not even going to try to type jquery.
    window: any; // or jsdom.
    pub_sock: zmq.Socket;
    ws_listeners: Array<any>;
    ws_subscriptions: Subscriptions;

    constructor(window, pub_sock: zmq.Socket) {
        this.window = window;
        this.$ = window.$;
        this.pub_sock = pub_sock;
        this.ws_listeners = [];
        this.ws_subscriptions = {};
    }
}


interface Message {
    type: string;
}
interface Response {
    error?: string;
}
function handle_message(ctx: Context, data): Response {
    var type: string = data.type;
    try {
        if (type == "ping")
            return handle_ping(data);
        else if (type == "html")
            return handle_html(ctx);
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
interface Version {
    major: number;
    minor: number;
}
interface PingResponse extends Response {
    pong: string;
    version: Version;
}
function handle_ping(data: PingMessage): PingResponse {
    log.info("handling ping");
    var version: Version = { major: query_major_version, minor: query_minor_version };
    var result: PingResponse = {
        pong: data.ping,
        version: version,
        zmq: {
            query_address: query_address,
            event_address: event_address
        },
        websocket: {
            address: websocket_address,
            client_code: websocket_client_code
        }
    };
    return result;
}


interface HtmlResponse extends Response {
    html: string;
}
function handle_html(ctx: Context): HtmlResponse {
    log.info("returning html serialization");
    return {html: jsdom.serializeDocument(ctx.window.document)};
}


interface Transform {
    method: string;
    args: string[];
}
interface Query {
    query: string;
    transforms: Transform[];
}
interface QueryMessage extends Message {
    query_group: Query[];
}
interface Attributes {
    [index: string]: string;
}
function attrs($, node): Attributes {
    var attrs: Attributes = {};
    $(node.attributes).each(function() { attrs[this.nodeName] = this.nodeValue; });
    return attrs;
}
interface QueryResult {
    text: string;
    attrs: Attributes;
}
function to_result($, node): QueryResult {
    // Text, with text of children stripped out.
    var text = $(node).clone().children().remove().end().text().trim();
    return { attrs: attrs($, node), text: text };
}
interface QueryResponse {
    [index: string]: QueryResult;
}
function handle_query(ctx: Context, data: QueryMessage): QueryResponse {
    log.info("handling query group");

   // Keep a list of all touched and changed nodes for output and subscription updates.
    var touched: QueryResponse = {};
    var changed: QueryResponse = {};

    // Handle equy query in order.
    for (var i in data.query_group)
        handle_one_query(ctx, data.query_group[i], touched, changed);

    // Publish touched nodes for subscribers to snoop on.
    for (var path in changed) {
        if (path === '')
            continue;

        ctx.pub_sock.send(path + " " + JSON.stringify(changed[path]));

        if (ctx.ws_subscriptions[path] !== undefined) {
            for (var i in ctx.ws_subscriptions[path])
                ctx.ws_subscriptions[path][i].write({path: path, message: changed[path]});
        }
        for (var i in ctx.ws_listeners)
            ctx.ws_listeners[i].write({path: path, message: changed[path]});
    }

    delete touched[''];
    return touched;
}
function handle_one_query(ctx: Context, query: Query, touched: QueryResponse, changed: QueryResponse) {
    // Perform the base query.
    var nodes = ctx.$(query.query);
    nodes.each(function(i, node) { touched[pathof(ctx.$, node)] = to_result(ctx.$, node); });

    // Apply each transform to the initial query.
    for (var i in query.transforms) {
        var method_name = query.transforms[i].method;
        var args = query.transforms[i].args;

        nodes = nodes[method_name].apply(nodes, args);
        nodes.each(function(i, node) {
            var map = to_result(ctx.$, node);
            touched[pathof(ctx.$, node)] = map;
            changed[pathof(ctx.$, node)] = map;
        });
    }
}


interface SubscribeMessage extends Message {
    target: string;
}
interface SubscribeResponse extends Response {
}
function handle_subscribe(ctx: Context, data: SubscribeMessage, spark): SubscribeResponse {
    if (ctx.ws_subscriptions[data.target] === undefined)
        ctx.ws_subscriptions[data.target] = [];
    ctx.ws_subscriptions[data.target].push(spark);
    return {};
}


function save_jsdom(window)
{
    log.info("saving snapshot");
    fs.writeFile('snapshot.html', jsdom.serializeDocument(window.document), function (err) {
        if (err)
            console.error("Failed to save snapshot.");
    });
}

function loaded_jsdom(errors, window)
{
    if (errors) {
        log.fatal("failed to load %s: %s", home_html, errors.toString());
        process.exit(1);
    }
    log.info("Loaded home: " + home_html);

    // Setup occasional autosave of the current state for inspection.
    setInterval(function() {save_jsdom(window);}, autosave_interval);

    // Setup broadcast sockets and listen for connections via ZMQ.
    var pub_sock = zmq.socket("pub");
    pub_sock.bind(event_address, function(error) {
        if (error) {
            log.fatal("Failed to bind event socket: " + error);
            process.exit(1);
        }

        var ctx = new Context(window, pub_sock);

        // Listen for zmq connections.
        var query_sock = zmq.socket("rep");
        query_sock.bind(query_address, function(error) {
            if (error) {
                log.fatal("Failed to bind query socket: " + error);
                process.exit(1);
            }

            query_sock.on('message', function(msg) {
                var output = handle_message(ctx, JSON.parse(msg.toString()));
                query_sock.send(JSON.stringify(output));
            });
        });

        // Listen for websocket connections.
        var primus_server = primus.createServer({
            port: 8080,
            protocol: "JSON"
        });
        primus_server.on('connection', function (spark) {
            log.info({address: spark.address}, 'new primus connection');

            spark.on('data', function (data) {
                var token = data.token;
                var message = data.message;
                log.info({data:data, token:token, message:message}, 'websocket data');
                var output;
                if (message.type == 'subscribe')
                    output = handle_subscribe(ctx, message, spark);
                else
                    output = handle_message(ctx, message);
                spark.write({token: token, message: output});
            });

            ctx.ws_listeners.push(spark);
        });
        primus_server.on('disconnection', function(spark) {
            ctx.ws_listeners = ctx.ws_listeners.filter(
                                                function(val, i, arr) { return val !== spark; });
            for (var key in ctx.ws_subscriptions) {
                ctx.ws_subscriptions[key] = ctx.ws_subscriptions[key].filter(
                                                function(val, i, arr) { return val !== spark; });
                if (ctx.ws_subscriptions[key].length === 0) {
                    delete ctx.ws_subscriptions[key];
                }
            }
        });
    });
}


fs.readFile(home_html, function(error, data) {
    if (error) {
        log.fatal("Failed to to load home '" + home_html + "': " + error);
        process.exit(1);
    }

    jsdom.env(
        data.toString(),
        ["../node_modules/jquery/dist/jquery.min.js",
         "../node_modules/jquery-color/jquery.color.js",
        ],
        loaded_jsdom
    );
});
